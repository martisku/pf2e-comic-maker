mod assets;
mod chapters;
mod projects;

use comic_model::{Layout, MAX_IMAGE_BYTES, Panel, ProjectSnapshot};
use dioxus::html::FileData;
use dioxus::prelude::*;
use gloo_net::http::Request;
use projects::Target;

const ARCHIVE_ART: &str = "/archive-forest.png";

fn main() {
    dioxus::launch(App);
}

fn edit_panel(
    mut state: Signal<Option<ProjectSnapshot>>,
    mut dirty: Signal<bool>,
    id: u32,
    edit: impl FnOnce(&mut Panel),
) {
    if let Some(snapshot) = state.write().as_mut()
        && let Some(panel) = snapshot.workspace.panels.iter_mut().find(|p| p.id == id)
    {
        edit(panel);
        dirty.set(true);
    }
}

#[component]
fn App() -> Element {
    let mut session = projects::use_session();
    let mut state = session.state;
    let mut dirty = session.dirty;
    let mut busy = session.busy;
    let mut status = session.status;
    let mut error = session.error;
    let mut selected = use_signal(|| 1u32);
    let mut overview = use_signal(|| false);
    let mut library = use_signal(|| false);
    let mut selected_chapter = use_signal(|| 1u32);
    let mut selection_project = use_signal(|| None::<String>);
    use_effect(move || {
        let project = state
            .read()
            .as_ref()
            .map(|snapshot| snapshot.project_id.clone());
        if project != *selection_project.peek() {
            selection_project.set(project);
            selected_chapter.set(0);
            selected.set(0);
        }
    });

    let snapshot = state.read().clone();
    let project_key = snapshot
        .as_ref()
        .map(|snapshot| snapshot.project_id.as_str())
        .unwrap_or_default();
    let workspace = snapshot.as_ref().map(|s| s.workspace.clone());
    let chapter = workspace.as_ref().and_then(|w| {
        w.chapters
            .iter()
            .find(|c| c.id == selected_chapter())
            .or_else(|| w.chapters.first())
            .cloned()
    });
    let chapter_id = chapter.as_ref().map(|c| c.id).unwrap_or(0);
    let page = chapter.as_ref().and_then(|c| c.pages.first()).cloned();
    let panels: Vec<Panel> = workspace
        .as_ref()
        .map(|w| {
            w.panels
                .iter()
                .filter(|p| {
                    chapter
                        .as_ref()
                        .is_some_and(|c| c.panel_ids.contains(&p.id))
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let active = panels
        .iter()
        .find(|p| p.id == selected())
        .or_else(|| panels.first())
        .cloned();
    let active_id = active.as_ref().map(|p| p.id);
    let visible_status = if dirty() && !busy() && error().is_none() {
        "Unsaved changes".to_string()
    } else {
        status()
    };

    rsx! {
        style { {include_str!("../assets/studio.css")} }
        aside { class: "archive-sidebar", "aria-label": "Workspace navigation",
            div { class: "archive-brand", span { class: "archive-sigil", "✧" }
                div { strong { "STORY ARCHIVE" } small { "PF2E · COMIC MAKER" } }
            }
            div { class: "archive-project", p { class: "eyebrow", "YOUR STORY WORLD" }
                strong { if let Some(ref workspace) = workspace { "{workspace.title}" } else { "Your comic workspace" } }
                small { "A Pathfinder story in the making" }
            }
            nav { "aria-label": "Workspace views",
                button { class: "archive-nav", disabled: busy(), onclick: move |_| session.request(Target::List), "Projects" }

                button { class: if overview() && !library() { "archive-nav active" } else { "archive-nav" }, "aria-pressed": overview() && !library(), onclick: move |_| { if session.show_list() || session.show_new() { session.current(); } overview.set(true); library.set(false); },
                    span { "◇" } "Story overview"
                }
                button { class: if !overview() && !library() { "archive-nav active" } else { "archive-nav" }, "aria-pressed": !overview() && !library(), onclick: move |_| { if session.show_list() || session.show_new() { session.current(); } overview.set(false); library.set(false); },
                    span { "▤" } "Page composition"
                }
                button { class: if library() { "archive-nav active" } else { "archive-nav" }, "aria-pressed": library(), onclick: move |_| { if session.show_list() || session.show_new() { session.current(); } library.set(true); },
                    span { "◇" } "Asset library"
                }
            }
            div { class: "archive-motto", span { "◇" } p { "Every story begins" br {} "with a single panel." }
                small { "YOUR COMIC WORKSPACE" }
            }
        }
        div { class: "studio",
            header { class: "topbar",
                div { class: "breadcrumb", span { "Workspace" } span { class: "breadcrumb-divider", "/" } strong { if library() { "Asset library" } else if overview() { "Story overview" } else { "Page composition" } } }
                div { class: "topbar-right", span { class: "prototype", "WORKING EDITION" }
                    span { role: "status", class: if error().is_some() { "save-status failed" } else { "save-status" }, "{visible_status}" }
                }
            }
            if let Some(message) = error().filter(|_| !session.dialog_open()) {
                div { class: "error", role: "alert",
                    span { "{message}" }
                    button { disabled: busy(), onclick: move |_| session.retry(), "Retry" }
                    button { disabled: busy(), onclick: move |_| session.reload(), "Reload saved work" }
                }
            }
            if session.show_list() {
                projects::ProjectList { session }
            } else if session.show_new() {
                projects::NewProject { session }
            } else if session.invalid() {
                div { class: "loading", "Choose Projects to open your saved work." }
            } else if let Some(workspace) = workspace {
                fieldset { class: "workspace-fields", disabled: session.loading(),

                section { class: "project-heading",
                    div { p { class: "eyebrow", "YOUR ILLUSTRATED STORY" }
                        h1 { if overview() { "Your story, unfolding." } else { "The story takes shape." } }
                        input { class: "project-title", "aria-label": "Project title", value: "{workspace.title}",
                            oninput: move |event| {
                                if let Some(snapshot) = state.write().as_mut() { snapshot.workspace.title = event.value(); dirty.set(true); }
                            }
                        }
                    }
                    label { class: "project-description", "Project description"
                        textarea { "aria-label": "Project description", rows: "2", value: "{snapshot.as_ref().map(|s| s.description.as_str()).unwrap_or_default()}",
                            oninput: move |event| {
                                if let Some(snapshot) = state.write().as_mut() { snapshot.description = event.value(); dirty.set(true); }
                            }
                        }
                    }
                    p { class: "intro", "Plan each moment. Arrange the page. Replace only the image you want to change." }
                }
                if library() {
                    assets::AssetLibrary { key: "{project_key}", state, dirty }
                } else {
                chapters::ChapterManager { key: "{project_key}", session, selected: selected_chapter, selected_panel: selected }
                if let Some(page) = page {
                if overview() {
                    main { class: "story-overview", "aria-label": "Story overview",
                        section { class: "archive-hero",
                            img { class: "archive-art", src: ARCHIVE_ART, alt: "" }
                            div { class: "hero-copy", p { class: "eyebrow", "CONTINUE YOUR STORY" }
                                h2 { "{workspace.title}" }
                                p { "A world waiting to be imagined. A story told one panel at a time." }
                                button { class: "primary-button", onclick: move |_| overview.set(false), "Open page composition →" }
                            }
                            span { class: "art-credit", "ATMOSPHERE ART · NOT PROJECT CONTENT" }
                        }
                        div { class: "overview-heading", h2 { "Moments in your story" }
                            span { "{panels.len()} panels · one working page" }
                        }
                        div { class: "story-grid",
                            for panel in panels.iter() {
                                {
                                    let id = panel.id;
                                    let position = page.slots.iter().position(|p| *p == Some(id));
                                    rsx! {
                                        button { class: "story-card", onclick: move |_| { selected.set(id); overview.set(false); library.set(false); },
                                            div { class: "story-card-art",
                                                if let Some(url) = panel.image_url() {
                                                    img { src: "{url}", alt: "", loading: "lazy" }
                                                } else {
                                                    span { class: "story-placeholder", "◇" } small { "Your illustration belongs here" }
                                                }
                                            }
                                            div { class: "story-card-copy",
                                                p { class: "eyebrow", if let Some(slot) = position { "POSITION {slot + 1}" } else { "UNPLACED" } }
                                                h3 { "{panel.title}" }
                                                p { class: "story-excerpt", "{panel.action}" }
                                                span { class: "story-card-footer", span { "{panel.images.len()} image alternatives" } span { "Open panel →" } }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        p { class: "archive-note", "✧  Your panels, images, and planning notes belong to the same saved workspace." }
                    }
                }
                main { class: if overview() { "workspace is-hidden" } else { "workspace" },
                    nav { class: "panel-list", "aria-label": "Chapter panels",
                        div { class: "section-label", span { "PANELS" } span { "{panels.len()}" } }
                        for panel in panels.iter() {
                            {
                                let id = panel.id;
                                let position = page.slots.iter().position(|p| *p == Some(id));
                                let is_active = Some(id) == active_id;
                                rsx! {
                                    button { class: if is_active { "panel-card active" } else { "panel-card" },
                                        onclick: move |_| selected.set(id), "aria-pressed": is_active,
                                        span { class: "panel-number", if let Some(slot) = position { "{slot + 1}" } else { "◇" } }
                                        div { strong { "{panel.title}" }
                                            small { if let Some(slot) = position { "Position {slot + 1}" } else { "Unplaced" } }
                                        }
                                        span { class: "panel-arrow", "›" }
                                    }
                                }
                            }
                        }
                        p { class: "list-note", "Panel identities stay the same when you change their position." }
                    }
                    section { class: "composition", "aria-label": "Page composition",
                        div { class: "composition-toolbar",
                            div { p { class: "eyebrow", "PAGE 01" } h2 { "Page composition" } }
                            label { "Layout"
                                select { value: page.layout.css_class(), "aria-label": "Page layout",
                                    oninput: move |event| {
                                        let layout = match event.value().as_str() { "full" => Layout::Full, "wide" => Layout::Wide, _ => Layout::Grid };
                                        if let Some(snapshot) = state.write().as_mut()
                                            && let Some(page) = snapshot.workspace.chapter_mut(chapter_id).and_then(|c| c.pages.first_mut()) {
                                            page.change_layout(layout); dirty.set(true);
                                        }
                                    },
                                    option { value: "grid", selected: page.layout == Layout::Grid, "2 × 2 grid" }
                                    option { value: "wide", selected: page.layout == Layout::Wide, "Wide + two" }
                                    option { value: "full", selected: page.layout == Layout::Full, "Full page" }
                                }
                            }
                        }
                        div { class: "page-surround",
                            div { class: "comic-page {page.layout.css_class()}",
                                for (slot, panel_id) in page.slots.iter().enumerate() {
                                    {
                                        let panel = panel_id.and_then(|id| panels.iter().find(|p| p.id == id)).cloned();
                                        rsx! {
                                            if let Some(panel) = panel {
                                                button { class: if Some(panel.id) == active_id { "page-panel selected" } else { "page-panel" },
                                                    "aria-label": "Edit position {slot + 1}: {panel.title}",
                                                    onclick: move |_| selected.set(panel.id),
                                                    if let Some(url) = panel.image_url() {
                                                        img { src: "{url}", alt: "{panel.title}",
                                                            style: format!("object-fit: {}; object-position: {}% {}%;", if panel.fill { "cover" } else { "contain" }, panel.crop_x, panel.crop_y) }
                                                    } else {
                                                        div { class: "empty-art", span { class: "art-symbol", "◇" }
                                                            strong { "{panel.title}" } small { "Select to plan & add an image" }
                                                        }
                                                    }
                                                    span { class: "position-badge", "{slot + 1}" }
                                                }
                                            } else {
                                                div { class: "empty-slot", span { "+" } small { "Empty position {slot + 1}" } }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        p { class: "composition-note", "Original images are preserved. Panels outside this layout remain in the panel list." }
                    }
                    if let Some(panel) = active {
                        {
                            let id = panel.id;
                            let position = page.slots.iter().position(|p| *p == Some(id));
                            let upload_disabled = busy() || dirty() || error().is_some();
                            rsx! {
                                aside { class: "editor", "aria-label": "Panel editor",
                                    p { class: "eyebrow", if let Some(slot) = position { "POSITION {slot + 1}" } else { "UNPLACED PANEL" } } h2 { "Shape this moment" }
                                    label { "Title"
                                        input { value: "{panel.title}", oninput: move |e| edit_panel(state, dirty, id, |p| p.title = e.value()) }
                                    }
                                    label { "Action & composition"
                                        textarea { rows: "4", value: "{panel.action}", oninput: move |e| edit_panel(state, dirty, id, |p| p.action = e.value()) }
                                    }
                                    label { "Expression"
                                        textarea { rows: "2", "aria-label": "Panel expression", value: "{panel.expression}", oninput: move |e| edit_panel(state, dirty, id, |p| p.expression = e.value()) }
                                    }
                                    assets::PanelAssetsEditor { state, dirty, panel_id: id }
                                    label { "Dialogue"
                                        textarea { rows: "2", placeholder: "What is said in this moment?", value: "{panel.dialogue}",
                                            oninput: move |e| edit_panel(state, dirty, id, |p| p.dialogue = e.value()) }
                                    }
                                    label { "Page position"
                                        select { value: position.map(|i| i.to_string()).unwrap_or_else(|| "unplaced".into()),
                                            oninput: move |event| {
                                                let target = event.value().parse::<usize>().ok();
                                                if let Some(snapshot) = state.write().as_mut()
                                                    && let Some(page) = snapshot.workspace.chapter_mut(chapter_id).and_then(|c| c.pages.first_mut()) {
                                                    page.place(id, target); dirty.set(true);
                                                }
                                            },
                                            option { value: "unplaced", selected: position.is_none(), "Unplaced" }
                                            for slot in 0..page.slots.len() { option { value: "{slot}", selected: position == Some(slot), "Position {slot + 1}" } }
                                        }
                                    }
                                    div { class: "image-section",
                                        div { class: "section-label", span { "PANEL IMAGE" } span { "{panel.images.len()} versions" } }
                                        label { class: if upload_disabled { "upload disabled" } else { "upload" },
                                            span { "+ Import an image" }
                                            input { r#type: "file", accept: "image/png,image/jpeg,image/webp", disabled: upload_disabled,
                                                onchange: move |event| {
                                                    if busy() || dirty() || error().is_some() { return; }
                                                    let files = event.files();
                                                    if files.is_empty() { return; }
                                                    let Some(before) = state.peek().clone() else { return; };
                                                    let activation = session.activation();
                                                    busy.set(true); status.set("Uploading…".into());
                                                    spawn(async move {
                                                        let result = upload_image(&before.project_id, id, before.revision, files).await;
                                                        if session.activation() != activation { return; }
                                                        match result {
                                                            Ok(saved) if saved.project_id == before.project_id => {
                                                                if let Some(current) = state.write().as_mut() {
                                                                    current.revision = saved.revision;
                                                                    for local in &mut current.workspace.panels {
                                                                        if let Some(remote) = saved.workspace.panels.iter().find(|remote| remote.id == local.id) {
                                                                            local.images = remote.images.clone();
                                                                            if local.id == id { local.selected_image = remote.selected_image.clone(); }
                                                                        }
                                                                    }
                                                                }
                                                                status.set(if dirty() { "Unsaved changes" } else { "Saved" }.into());
                                                            }
                                                            Ok(_) => {
                                                                error.set(Some("The server returned a different project. Reload saved work.".into()));
                                                                status.set("Upload failed".into());
                                                            }
                                                            Err(message) => {
                                                                session.uncertain.set(message.contains("outcome is unknown"));
                                                                error.set(Some(message)); status.set("Upload failed".into());
                                                            }
                                                        }
                                                        busy.set(false);
                                                    });
                                                }
                                            }
                                        }
                                        small { "PNG, JPEG or WebP · up to 20 MB" }
                                        if dirty() { small { "Your edits are saving before upload is available." } }
                                        if !panel.images.is_empty() {
                                            div { class: "image-alternatives", role: "group", "aria-label": "Image alternatives",
                                                for (index, image) in panel.images.iter().enumerate() {
                                                    {
                                                        let image_id = image.id.clone();
                                                        let is_selected = panel.selected_image.as_ref() == Some(&image.id);
                                                        rsx! {
                                                            button { class: if is_selected { "image-alternative active" } else { "image-alternative" },
                                                                "aria-label": "Select image alternative {index + 1}", "aria-pressed": is_selected,
                                                                onclick: move |_| edit_panel(state, dirty, id, |p| p.selected_image = Some(image_id.clone())),
                                                                img { src: "{image.url}", alt: "", loading: "lazy" }
                                                                span { "Image {index + 1}" }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            label { "Selected image"
                                                select { value: panel.selected_image.clone().unwrap_or_default(),
                                                    oninput: move |e| edit_panel(state, dirty, id, |p| p.selected_image = Some(e.value())),
                                                    for (index, image) in panel.images.iter().enumerate() {
                                                        option { value: "{image.id}", selected: panel.selected_image.as_ref() == Some(&image.id), "Image {index + 1}" }
                                                    }
                                                }
                                            }
                                            label { "Image fitting"
                                                select { value: if panel.fill { "fill" } else { "contain" },
                                                    oninput: move |e| edit_panel(state, dirty, id, |p| p.fill = e.value() == "fill"),
                                                    option { value: "fill", selected: panel.fill, "Fill with cropping" } option { value: "contain", selected: !panel.fill, "Show entire image" }
                                                }
                                            }
                                            if panel.fill {
                                                label { "Horizontal crop"
                                                    input { r#type: "range", min: "0", max: "100", value: "{panel.crop_x}",
                                                        oninput: move |e| edit_panel(state, dirty, id, |p| p.crop_x = e.value().parse().unwrap_or(50)) }
                                                }
                                                label { "Vertical crop"
                                                    input { r#type: "range", min: "0", max: "100", value: "{panel.crop_y}",
                                                        oninput: move |e| edit_panel(state, dirty, id, |p| p.crop_y = e.value().parse().unwrap_or(50)) }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                }
                }
            }
            } else {
                div { class: "loading", "{status}" }
            }
            projects::UnsavedDialog { session }
            footer { span { "PF2E COMIC MAKER · STORY ARCHIVE" } span { "The story is yours. Give it shape. ✧" } }
        }
    }
}

async fn upload_image(
    project_id: &str,
    panel_id: u32,
    revision: u64,
    files: Vec<FileData>,
) -> Result<ProjectSnapshot, String> {
    let file = files
        .first()
        .ok_or_else(|| "No image selected.".to_string())?;
    if file.size() > MAX_IMAGE_BYTES as u64 {
        return Err("Choose an image no larger than 20 MB.".into());
    }
    let bytes = file.read_bytes().await.map_err(|e| e.to_string())?;
    let upload_id = uuid::Uuid::new_v4().to_string();
    let mut last_failure = String::from("No server response");
    for attempt in 0..3 {
        let response = Request::post(&format!(
            "/api/projects/{project_id}/panels/{panel_id}/images"
        ))
        .header("content-type", "application/octet-stream")
        .header("x-comic-revision", &revision.to_string())
        .header("x-comic-upload-id", &upload_id)
        .body(bytes.to_vec())
        .map_err(|e| e.to_string())?
        .send()
        .await;
        last_failure = match response {
            Ok(response) if response.ok() => {
                if let Ok(snapshot) = response.json().await {
                    return Ok(snapshot);
                }
                // A lost/malformed success body is ambiguous too: reuse the ID.
                format!("HTTP {}, invalid server response", response.status())
            }
            Ok(response) if response.status() < 500 => {
                return Err(response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Upload rejected.".into()));
            }
            Ok(response) => format!("HTTP {}", response.status()),
            Err(_) => "Network request failed".into(),
        };
        if attempt < 2 {
            gloo_timers::future::TimeoutFuture::new(500 * (attempt + 1)).await;
        }
    }
    Err(format!(
        "Upload outcome is unknown ({last_failure}; reference {upload_id}). Reload saved work before selecting the file again."
    ))
}
