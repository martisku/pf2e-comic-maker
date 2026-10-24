use comic_model::{ORIGINAL_PROJECT_ID, ProjectCreate, ProjectSnapshot, ProjectSummary};
use dioxus::prelude::*;
use gloo_net::http::{Request, Response};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Open(String),
    List,
    New,
    Invalid,
}

impl Target {
    fn url(&self) -> String {
        match self {
            Self::Open(id) => format!("?project={id}"),
            Self::List => "?view=projects".into(),
            Self::New => "?view=new".into(),
            Self::Invalid => "?view=invalid".into(),
        }
    }
}

fn location_target() -> Target {
    let Some(window) = web_sys::window() else {
        return Target::Invalid;
    };
    let Ok(search) = window.location().search() else {
        return Target::Invalid;
    };
    let Ok(params) = web_sys::UrlSearchParams::new_with_str(&search) else {
        return Target::Invalid;
    };
    if let Some(project) = params.get("project") {
        return match uuid::Uuid::parse_str(&project) {
            Ok(id) => Target::Open(id.to_string()),
            Err(_) => Target::Invalid,
        };
    }
    match params.get("view").as_deref() {
        Some("projects") => Target::List,
        Some("new") => Target::New,
        None => Target::Open(ORIGINAL_PROJECT_ID.into()),
        _ => Target::Invalid,
    }
}

thread_local! {
    static HISTORY_INDEX: Cell<i32> = const { Cell::new(0) };
    static RESTORE_NAV: RefCell<Option<(Target, i32)>> = const { RefCell::new(None) };
    static SUPPRESS_POP: Cell<bool> = const { Cell::new(false) };
}

fn write_url(target: &Target, push: bool) {
    if let Some(window) = web_sys::window()
        && let Ok(history) = window.history()
    {
        let index = HISTORY_INDEX.with(|cursor| {
            let next = cursor.get() + i32::from(push);
            cursor.set(next);
            next
        });
        let _ = if push {
            history.push_state_with_url(
                &JsValue::from_f64(f64::from(index)),
                "",
                Some(&target.url()),
            )
        } else {
            history.replace_state_with_url(
                &JsValue::from_f64(f64::from(index)),
                "",
                Some(&target.url()),
            )
        };
    }
}

async fn decode<T: serde::de::DeserializeOwned>(response: Response) -> Result<T, String> {
    if !response.ok() {
        return Err(response
            .text()
            .await
            .unwrap_or_else(|_| "Request failed.".into()));
    }
    response
        .json()
        .await
        .map_err(|_| "Invalid server response. Reload saved work before retrying.".into())
}

pub async fn load(project: &str) -> Result<ProjectSnapshot, String> {
    decode(
        Request::get(&format!("/api/projects/{project}"))
            .send()
            .await
            .map_err(|_| "Could not load project. Try again.".to_string())?,
    )
    .await
}

async fn save(snapshot: &ProjectSnapshot) -> Result<ProjectSnapshot, String> {
    decode(
        Request::put(&format!("/api/projects/{}", snapshot.project_id))
            .json(snapshot)
            .map_err(|_| "Could not encode project.".to_string())?
            .send()
            .await
            .map_err(|_| "Saving failed. Your edits remain on screen.".to_string())?,
    )
    .await
}

#[derive(Clone, PartialEq)]
struct Navigation {
    target: Target,
    push: bool,
    history_index: Option<i32>,
}

fn commit_url(navigation: &Navigation) {
    if let Some(index) = navigation.history_index {
        let previous = HISTORY_INDEX.with(|cursor| cursor.replace(index));
        if index != previous {
            SUPPRESS_POP.with(|flag| flag.set(true));
            if let Some(window) = web_sys::window()
                && let Ok(history) = window.history()
            {
                let _ = history.go_with_delta(index - previous);
            }
        }
    } else {
        write_url(&navigation.target, navigation.push);
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Session {
    pub state: Signal<Option<ProjectSnapshot>>,
    pub dirty: Signal<bool>,
    pub busy: Signal<bool>,
    pub status: Signal<String>,
    pub error: Signal<Option<String>>,
    pub uncertain: Signal<bool>,
    loading: Signal<bool>,
    generation: Signal<u64>,
    view: Signal<Target>,
    pending: Signal<Option<Navigation>>,
    flushing: Signal<bool>,
    failed_navigation: Signal<Option<Navigation>>,
    browser_navigation: Signal<Option<Navigation>>,
}

impl Session {
    pub fn dialog_open(self) -> bool {
        self.pending.read().is_some()
    }
    pub fn show_list(self) -> bool {
        matches!(*self.view.read(), Target::List)
    }
    pub fn show_new(self) -> bool {
        matches!(*self.view.read(), Target::New)
    }
    pub fn invalid(self) -> bool {
        matches!(*self.view.read(), Target::Invalid)
    }
    pub fn loading(self) -> bool {
        *self.loading.read()
    }
    pub fn activation(self) -> u64 {
        *self.generation.peek()
    }

    pub fn request(self, target: Target) {
        self.navigate(Navigation {
            target,
            push: true,
            history_index: None,
        });
    }

    fn queue_browser_navigation(mut self, navigation: Navigation) {
        self.browser_navigation.set(Some(navigation));
    }

    fn navigate(mut self, navigation: Navigation) {
        if *self.busy.peek() {
            return;
        }
        if *self.uncertain.peek() {
            let reloading = self
                .state
                .peek()
                .as_ref()
                .is_some_and(|s| navigation.target == Target::Open(s.project_id.clone()));
            if !reloading {
                self.error.set(Some("The upload outcome is unknown. Reload saved work in this project before switching.".into()));
                return;
            }
        }
        if *self.dirty.peek() {
            self.pending.set(Some(navigation));
        } else {
            self.activate(navigation);
        }
    }

    fn activate(mut self, navigation: Navigation) {
        if *self.busy.peek() {
            return;
        }
        self.pending.set(None);
        self.flushing.set(false);
        self.failed_navigation.set(None);
        match &navigation.target {
            Target::Open(id) => {
                let id = id.clone();
                self.busy.set(true);
                self.loading.set(true);
                self.status.set("Loading project…".into());
                let generation = self.activation();
                spawn(async move {
                    let result = load(&id).await;
                    if self.activation() != generation {
                        return;
                    }
                    match result {
                        Ok(snapshot) if snapshot.project_id == id => {
                            self.generation.set(generation + 1);
                            self.state.set(Some(snapshot));
                            self.dirty.set(false);
                            self.error.set(None);
                            self.uncertain.set(false);
                            self.status.set("Saved".into());
                            self.view.set(navigation.target.clone());
                            commit_url(&navigation);
                        }
                        Ok(_) => {
                            self.failed_navigation.set(Some(navigation.clone()));
                            self.error.set(Some("The server returned a different project. Open Projects and try again.".into()));
                            self.status.set("Load failed".into());
                        }
                        Err(message) => {
                            self.failed_navigation.set(Some(navigation.clone()));
                            self.error.set(Some(message));
                            self.status.set("Load failed".into());
                        }
                    }
                    self.loading.set(false);
                    self.busy.set(false);
                });
            }
            Target::Invalid => {
                self.view.set(Target::Invalid);
                self.error.set(Some(
                    "This project link is invalid. Open Projects to choose a saved project.".into(),
                ));
                self.status.set("Load failed".into());
            }
            Target::List | Target::New => {
                self.generation.set(self.activation() + 1);
                self.dirty.set(false);
                self.error.set(None);
                self.view.set(navigation.target.clone());
                self.status.set("Saved".into());
                commit_url(&navigation);
            }
        }
    }

    pub fn current(self) {
        if let Some(snapshot) = self.state.peek().as_ref() {
            self.request(Target::Open(snapshot.project_id.clone()));
        }
    }

    pub fn reload(self) {
        self.current();
    }

    pub fn retry(mut self) {
        let failed = self.failed_navigation.peek().clone();
        if let Some(navigation) = failed {
            self.navigate(navigation);
            return;
        }
        if self
            .error
            .peek()
            .as_ref()
            .is_some_and(|message| message.contains("changed in another session"))
        {
            self.status
                .set("Copy unsaved text before reloading the latest saved work.".into());
            return;
        }
        if *self.uncertain.peek() {
            self.reload();
            return;
        }
        self.error.set(None);
        if self.state.peek().is_none() {
            self.activate(Navigation {
                target: location_target(),
                push: false,
                history_index: None,
            });
        }
    }
}

pub fn use_session() -> Session {
    let session = Session {
        state: use_signal(|| None),
        dirty: use_signal(|| false),
        busy: use_signal(|| false),
        status: use_signal(|| "Loading project…".into()),
        error: use_signal(|| None),
        uncertain: use_signal(|| false),
        loading: use_signal(|| false),
        generation: use_signal(|| 0),
        view: use_signal(location_target),
        pending: use_signal(|| None),
        flushing: use_signal(|| false),
        failed_navigation: use_signal(|| None),
        browser_navigation: use_signal(|| None),
    };
    use_hook(move || {
        let mut session = session;
        if let Some(window) = web_sys::window()
            && let Ok(history) = window.history()
        {
            let index = history
                .state()
                .ok()
                .and_then(|state| state.as_f64())
                .unwrap_or(0.0) as i32;
            HISTORY_INDEX.with(|cursor| cursor.set(index));
            let _ = history.replace_state_with_url(&JsValue::from_f64(f64::from(index)), "", None);
        }
        session.activate(Navigation {
            target: location_target(),
            push: false,
            history_index: None,
        });
        spawn(async move {
            loop {
                gloo_timers::future::TimeoutFuture::new(700).await;
                if !*session.dirty.peek()
                    || *session.busy.peek()
                    || session.error.peek().is_some()
                    || (session.pending.peek().is_some() && !*session.flushing.peek())
                {
                    continue;
                }
                let Some(sent) = session.state.peek().clone() else {
                    continue;
                };
                let activation = session.activation();
                session.busy.set(true);
                session.status.set("Saving…".into());
                let result = save(&sent).await;
                if session.activation() != activation {
                    continue;
                }
                match result {
                    Ok(saved) if saved.project_id == sent.project_id => {
                        let mut clean = false;
                        if let Some(current) = session.state.write().as_mut()
                            && current.project_id == sent.project_id
                        {
                            current.revision = saved.revision;
                            clean = current.workspace == sent.workspace
                                && current.description == sent.description;
                        }
                        session.dirty.set(!clean);
                        session
                            .status
                            .set(if clean { "Saved" } else { "Unsaved changes" }.into());
                        session.busy.set(false);
                        if clean && *session.flushing.peek() {
                            let pending = session.pending.peek().clone();
                            if let Some(navigation) = pending {
                                session.activate(navigation);
                            }
                        }
                    }
                    Ok(_) => {
                        session.error.set(Some(
                            "The server returned a different project. Reload saved work.".into(),
                        ));
                        session.status.set("Saving failed".into());
                        session.busy.set(false);
                    }
                    Err(message) => {
                        session.error.set(Some(message));
                        session.status.set("Saving failed".into());
                        session.flushing.set(false);
                        session.busy.set(false);
                    }
                }
            }
        });
    });
    use_effect(move || {
        let navigation = session.browser_navigation.read().clone();
        if let Some(navigation) = navigation {
            let mut session = session;
            session.browser_navigation.set(None);
            // Native popstate callbacks run outside Dioxus's runtime. Handle
            // navigation here so any spawned load has a runtime and scope.
            session.navigate(navigation);
        }
    });
    let events = use_hook(move || Rc::new(BrowserEvents::new(session)));
    use_effect(move || {
        events
            .set_unsaved(*session.dirty.read() || *session.busy.read() || *session.uncertain.read())
    });
    session
}

struct BrowserEvents {
    unload: Closure<dyn FnMut(web_sys::BeforeUnloadEvent)>,
    pop: Closure<dyn FnMut(web_sys::Event)>,
    registered: Cell<bool>,
}

impl BrowserEvents {
    fn new(session: Session) -> Self {
        let unload = Closure::new(move |event: web_sys::BeforeUnloadEvent| {
            if *session.dirty.peek() || *session.busy.peek() || *session.uncertain.peek() {
                event.prevent_default();
                #[allow(deprecated)]
                event.set_return_value("");
            }
        });
        let pop = Closure::new(move |event: web_sys::Event| {
            if SUPPRESS_POP.with(|flag| flag.replace(false)) {
                return;
            }
            let current_index = HISTORY_INDEX.with(Cell::get);
            let event_index = event
                .dyn_ref::<web_sys::PopStateEvent>()
                .and_then(|event| event.state().as_f64())
                .unwrap_or(0.0) as i32;
            let restored = RESTORE_NAV.with(|pending| {
                if event_index == current_index {
                    pending.borrow_mut().take()
                } else {
                    None
                }
            });
            if let Some((target, index)) = restored {
                session.queue_browser_navigation(Navigation {
                    target,
                    push: false,
                    history_index: Some(index),
                });
                return;
            }
            let target = location_target();
            if current_index == event_index {
                session.queue_browser_navigation(Navigation {
                    target,
                    push: false,
                    history_index: None,
                });
                return;
            }
            // Restore the cursor, not the destination entry's URL. A confirmed
            // transition traverses again; Stay leaves the entire history intact.
            RESTORE_NAV.with(|pending| *pending.borrow_mut() = Some((target, event_index)));
            if let Some(window) = web_sys::window()
                && let Ok(history) = window.history()
            {
                let _ = history.go_with_delta(current_index - event_index);
            }
        });
        if let Some(window) = web_sys::window() {
            let _ =
                window.add_event_listener_with_callback("popstate", pop.as_ref().unchecked_ref());
        }
        Self {
            unload,
            pop,
            registered: Cell::new(false),
        }
    }

    fn set_unsaved(&self, unsaved: bool) {
        if self.registered.replace(unsaved) == unsaved {
            return;
        }
        if let Some(window) = web_sys::window() {
            if unsaved {
                let _ = window.add_event_listener_with_callback(
                    "beforeunload",
                    self.unload.as_ref().unchecked_ref(),
                );
            } else {
                let _ = window.remove_event_listener_with_callback(
                    "beforeunload",
                    self.unload.as_ref().unchecked_ref(),
                );
            }
        }
    }
}

impl Drop for BrowserEvents {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window() {
            let _ = window.remove_event_listener_with_callback(
                "beforeunload",
                self.unload.as_ref().unchecked_ref(),
            );
            let _ = window
                .remove_event_listener_with_callback("popstate", self.pop.as_ref().unchecked_ref());
        }
    }
}

#[component]
pub fn ProjectList(session: Session) -> Element {
    let projects = use_resource(move || async move {
        decode::<Vec<ProjectSummary>>(
            Request::get("/api/projects")
                .send()
                .await
                .map_err(|_| "Could not load projects.".to_string())?,
        )
        .await
    });
    let mut projects = projects;
    rsx! {
        main { class: "project-browser",
            h1 { "Your projects" }
            p { "Each project keeps its own panels, images and planning notes." }
            button { class: "primary-button", disabled: (session.busy)(), onclick: move |_| session.request(Target::New), "New project" }
            match projects.read().as_ref() {
                None => rsx! { p { role: "status", "Loading projects…" } },
                Some(Err(message)) => rsx! { div { role: "alert", "{message}" button { onclick: move |_| projects.restart(), "Retry project list" } } },
                Some(Ok(items)) if items.is_empty() => rsx! { p { "No projects yet. Create your first story." } },
                Some(Ok(items)) => rsx! {
                    div { class: "project-list",
                        for item in items {
                            { let id = item.project_id.clone(); rsx! {
                                button { class: "project-list-item", "aria-label": "Open project {item.name}", disabled: (session.busy)(),
                                    onclick: move |_| session.request(Target::Open(id.clone())),
                                    strong { "{item.name}" } span { "Open project →" }
                                }
                            } }
                        }
                    }
                },
            }
        }
    }
}

#[component]
pub fn NewProject(mut session: Session) -> Element {
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut creation = use_signal(|| None::<ProjectCreate>);
    let mut create_error = use_signal(|| None::<String>);
    rsx! {
        main { class: "project-browser",
            h1 { "New project" }
            p { "Start with a chapter, one composition page and four blank panels. Add more chapters and panels as your story grows." }
            form { onsubmit: move |event| {
                event.prevent_default();
                if (session.busy)() { return; }
                let request = creation.peek().clone().unwrap_or_else(|| ProjectCreate {
                    project_id: uuid::Uuid::new_v4().to_string(), name: name(), description: description(),
                });
                creation.set(Some(request.clone()));
                session.busy.set(true);
                create_error.set(None);
                spawn(async move {
                    let result = async {
                        let response = Request::post("/api/projects").json(&request)
                            .map_err(|_| "Could not encode project.".to_string())?.send().await
                            .map_err(|_| "Creation outcome is unknown. Retry the same creation request.".to_string())?;
                        if !response.ok() && response.status() < 500 {
                            // A definitive rejection permits correcting the fields;
                            // ambiguous failures keep the UUID and input frozen.
                            creation.set(None);
                        }
                        decode::<ProjectSnapshot>(response).await
                    }.await;
                    session.busy.set(false);
                    match result {
                        Ok(snapshot) if snapshot.project_id == request.project_id => {
                            let target = Target::Open(snapshot.project_id.clone());
                            session.generation.set(session.activation() + 1);
                            session.state.set(Some(snapshot)); session.dirty.set(false);
                            session.error.set(None); session.status.set("Saved".into());
                            session.view.set(target.clone()); write_url(&target, true);
                        }
                        Ok(_) => create_error.set(Some("The server returned a different project. Return to Projects and reload.".into())),
                        Err(message) => create_error.set(Some(message)),
                    }
                });
            },
                label { "Name" input { "aria-label": "New project name", required: true, value: "{name}", disabled: (session.busy)() || creation().is_some(), oninput: move |event| name.set(event.value()) } }
                label { "Description (optional)" textarea { "aria-label": "New project description", rows: "4", value: "{description}", disabled: (session.busy)() || creation().is_some(), oninput: move |event| description.set(event.value()) } }
                if let Some(message) = create_error() { p { role: "alert", "{message}" } }
                button { class: "primary-button", r#type: "submit", disabled: (session.busy)() || name().trim().is_empty(), "Create project" }
                button { r#type: "button", disabled: (session.busy)(), onclick: move |_| session.request(Target::List), "Cancel" }
            }
        }
    }
}

#[component]
pub fn UnsavedDialog(mut session: Session) -> Element {
    if session.pending.read().is_none() {
        return rsx! {};
    }
    rsx! {
        div { class: "dialog-backdrop",
            dialog { id: "unsaved-dialog", class: "unsaved-dialog", role: "dialog", "aria-modal": "true", "aria-label": "Unsaved changes", tabindex: "-1",
                onmounted: move |_| {
                    if let Some(document) = web_sys::window().and_then(|window| window.document())
                        && let Some(element) = document.get_element_by_id("unsaved-dialog")
                        && let Ok(dialog) = element.dyn_into::<web_sys::HtmlDialogElement>()
                    { let _ = dialog.show_modal(); }
                },
                onkeydown: move |event| { if event.key() == Key::Escape {
                    event.prevent_default();
                    if !(session.busy)() { session.pending.set(None); session.flushing.set(false); }
                } },
                h2 { "Unsaved changes" }
                if let Some(message) = (session.error)() { p { role: "alert", "{message}" } }
                p { "Save your latest edits before continuing, or discard the changes that have not been sent." }
                button { disabled: (session.busy)(), onclick: move |_| { session.pending.set(None); session.flushing.set(false); }, "Stay" }
                button { disabled: (session.busy)() || (session.uncertain)(), onclick: move |_| {
                    if session.error.peek().as_ref().is_some_and(|message| message.contains("changed in another session")) { return; }
                    session.error.set(None); session.flushing.set(true);
                }, "Save and continue" }
                button { disabled: (session.busy)(), onclick: move |_| {
                    let pending = session.pending.peek().clone();
                    if let Some(navigation) = pending { session.activate(navigation); }
                }, "Discard changes and continue" }
            }
        }
    }
}
