use comic_model::{Layout, MAX_IMAGE_BYTES, Panel, SaveRequest, Snapshot};
use dioxus::html::FileData;
use dioxus::prelude::*;
use gloo_net::http::Request;

fn main() {
    dioxus::launch(App);
}

async fn load_workspace() -> Result<Snapshot, String> {
    let response = Request::get("/api/workspace")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(response
            .text()
            .await
            .unwrap_or_else(|_| "Load failed.".into()));
    }
    response.json().await.map_err(|e| e.to_string())
}

async fn save_workspace(snapshot: Snapshot) -> Result<Snapshot, String> {
    let response = Request::put("/api/workspace")
        .json(&SaveRequest {
            revision: snapshot.revision,
            workspace: snapshot.workspace,
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(response
            .text()
            .await
            .unwrap_or_else(|_| "Save failed.".into()));
    }
    response.json().await.map_err(|e| e.to_string())
}

fn edit_panel(
    mut state: Signal<Option<Snapshot>>,
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
    let mut state = use_signal(|| None::<Snapshot>);
    let mut dirty = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut status = use_signal(|| "Loading workspace…".to_string());
    let mut error = use_signal(|| None::<String>);
    let mut selected = use_signal(|| 1u32);

    // One serialized save loop: edits made while a request runs remain dirty.
    use_hook(move || {
        spawn(async move {
            match load_workspace().await {
                Ok(snapshot) => {
                    state.set(Some(snapshot));
                    status.set("Saved".into());
                }
                Err(message) => {
                    error.set(Some(message));
                    status.set("Load failed".into());
                }
            }
            loop {
                gloo_timers::future::TimeoutFuture::new(700).await;
                if !*dirty.peek() || *busy.peek() || error.peek().is_some() {
                    continue;
                }
                let Some(sent) = state.peek().clone() else {
                    continue;
                };
                busy.set(true);
                status.set("Saving…".into());
                match save_workspace(sent.clone()).await {
                    Ok(saved) => {
                        let mut current = state.write();
                        if let Some(current) = current.as_mut() {
                            current.revision = saved.revision;
                            if current.workspace == sent.workspace {
                                dirty.set(false);
                                status.set("Saved".into());
                            } else {
                                status.set("Unsaved changes".into());
                            }
                        }
                    }
                    Err(message) => {
                        error.set(Some(message));
                        status.set("Saving failed".into());
                    }
                }
                busy.set(false);
            }
        })
    });

    let snapshot = state.read().clone();
    let workspace = snapshot.as_ref().map(|s| s.workspace.clone());
    let active = workspace
        .as_ref()
        .and_then(|w| w.panels.iter().find(|p| p.id == selected()).cloned());
    let visible_status = if dirty() && !busy() && error().is_none() {
        "Unsaved changes".to_string()
    } else {
        status()
    };

    rsx! {
        style { {include_str!("../assets/studio.css")} }
        div { class: "studio",
            header { class: "topbar",
                div { class: "brand", span { class: "mark", "P" } div { strong { "PF2E" } span { "COMIC STUDIO" } } }
                div { class: "topbar-right", span { class: "prototype", "Dioxus prototype" }
                    span { role: "status", class: if error().is_some() { "save-status failed" } else { "save-status" }, "{visible_status}" }
                }
            }
            if let Some(message) = error() {
                div { class: "error", role: "alert",
                    span { "{message}" }
                    button { disabled: busy(), onclick: move |_| {
                        error.set(None);
                        if state.peek().is_none() {
                            spawn(async move {
                                match load_workspace().await {
                                    Ok(snapshot) => { state.set(Some(snapshot)); status.set("Saved".into()); }
                                    Err(message) => { error.set(Some(message)); status.set("Load failed".into()); }
                                }
                            });
                        } else if !dirty() { status.set("Saved".into()); }
                    }, "Retry" }
                    button { disabled: busy(), onclick: move |_| {
                        if dirty() { status.set("Unsaved edits remain. Copy your text before reloading.".into()); return; }
                        spawn(async move {
                            match load_workspace().await {
                                Ok(snapshot) => { state.set(Some(snapshot)); error.set(None); status.set("Saved".into()); }
                                Err(message) => error.set(Some(message)),
                            }
                        });
                    }, "Reload saved work" }
                }
            }
            if let Some(workspace) = workspace {
                section { class: "project-heading",
                    div { p { class: "eyebrow", "CHAPTER WORKSPACE · 01" }
                        h1 { "The story takes shape." }
                        input { class: "project-title", "aria-label": "Project title", value: "{workspace.title}",
                            oninput: move |event| {
                                if let Some(snapshot) = state.write().as_mut() { snapshot.workspace.title = event.value(); dirty.set(true); }
                            }
                        }
                    }
                    p { class: "intro", "Plan each moment. Arrange the page. Replace only the image you want to change." }
                }
                main { class: "workspace",
                    nav { class: "panel-list", "aria-label": "Chapter panels",
                        div { class: "section-label", span { "PANELS" } span { "04" } }
                        for panel in workspace.panels.iter() {
                            {
                                let id = panel.id;
                                let position = workspace.slots.iter().position(|p| *p == Some(id));
                                let is_active = id == selected();
                                rsx! {
                                    button { class: if is_active { "panel-card active" } else { "panel-card" },
                                        onclick: move |_| selected.set(id), "aria-pressed": is_active,
                                        span { class: "panel-number", "0{id}" }
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
                                select { value: workspace.layout.css_class(), "aria-label": "Page layout",
                                    oninput: move |event| {
                                        let layout = match event.value().as_str() { "full" => Layout::Full, "wide" => Layout::Wide, _ => Layout::Grid };
                                        if let Some(snapshot) = state.write().as_mut() { snapshot.workspace.change_layout(layout); dirty.set(true); }
                                    },
                                    option { value: "grid", selected: workspace.layout == Layout::Grid, "2 × 2 grid" }
                                    option { value: "wide", selected: workspace.layout == Layout::Wide, "Wide + two" }
                                    option { value: "full", selected: workspace.layout == Layout::Full, "Full page" }
                                }
                            }
                        }
                        div { class: "page-surround",
                            div { class: "comic-page {workspace.layout.css_class()}",
                                for (slot, panel_id) in workspace.slots.iter().enumerate() {
                                    {
                                        let panel = panel_id.and_then(|id| workspace.panels.iter().find(|p| p.id == id)).cloned();
                                        rsx! {
                                            if let Some(panel) = panel {
                                                button { class: if panel.id == selected() { "page-panel selected" } else { "page-panel" },
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
                            let position = workspace.slots.iter().position(|p| *p == Some(id));
                            let upload_disabled = busy() || dirty() || error().is_some();
                            rsx! {
                                aside { class: "editor", "aria-label": "Panel editor",
                                    p { class: "eyebrow", "PANEL 0{id}" } h2 { "Shape this moment" }
                                    label { "Title"
                                        input { value: "{panel.title}", oninput: move |e| edit_panel(state, dirty, id, |p| p.title = e.value()) }
                                    }
                                    label { "Action & composition"
                                        textarea { rows: "4", value: "{panel.action}", oninput: move |e| edit_panel(state, dirty, id, |p| p.action = e.value()) }
                                    }
                                    label { "Dialogue"
                                        textarea { rows: "2", placeholder: "What is said in this moment?", value: "{panel.dialogue}",
                                            oninput: move |e| edit_panel(state, dirty, id, |p| p.dialogue = e.value()) }
                                    }
                                    label { "Page position"
                                        select { value: position.map(|i| i.to_string()).unwrap_or_else(|| "unplaced".into()),
                                            oninput: move |event| {
                                                let target = event.value().parse::<usize>().ok();
                                                if let Some(snapshot) = state.write().as_mut() { snapshot.workspace.place(id, target); dirty.set(true); }
                                            },
                                            option { value: "unplaced", selected: position.is_none(), "Unplaced" }
                                            for slot in 0..workspace.slots.len() { option { value: "{slot}", selected: position == Some(slot), "Position {slot + 1}" } }
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
                                                    busy.set(true); status.set("Uploading…".into());
                                                    spawn(async move {
                                                        let result = upload_image(id, before.revision, files).await;
                                                        match result {
                                                            Ok(saved) => {
                                                                if let Some(current) = state.write().as_mut() {
                                                                    current.revision = saved.revision;
                                                                    for (local, remote) in current.workspace.panels.iter_mut().zip(saved.workspace.panels) {
                                                                        local.images = remote.images;
                                                                        if local.id == id { local.selected_image = remote.selected_image; }
                                                                    }
                                                                }
                                                                status.set(if dirty() { "Unsaved changes" } else { "Saved" }.into());
                                                            }
                                                            Err(message) => { error.set(Some(message)); status.set("Upload failed".into()); }
                                                        }
                                                        busy.set(false);
                                                    });
                                                }
                                            }
                                        }
                                        small { "PNG, JPEG or WebP · up to 20 MB" }
                                        if dirty() { small { "Your edits are saving before upload is available." } }
                                        if !panel.images.is_empty() {
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
            } else {
                div { class: "loading", "{status}" }
            }
            footer { "A small working study in Rust + Dioxus. Sample workspace; milestone features are still to come." }
        }
    }
}

async fn upload_image(
    panel_id: u32,
    revision: u64,
    files: Vec<FileData>,
) -> Result<Snapshot, String> {
    let file = files
        .first()
        .ok_or_else(|| "No image selected.".to_string())?;
    if file.size() > MAX_IMAGE_BYTES as u64 {
        return Err("Choose an image no larger than 20 MB.".into());
    }
    let bytes = file.read_bytes().await.map_err(|e| e.to_string())?;
    let response = Request::post(&format!("/api/panels/{panel_id}/images"))
        .header("content-type", "application/octet-stream")
        .header("x-comic-revision", &revision.to_string())
        .body(bytes.to_vec())
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(response
            .text()
            .await
            .unwrap_or_else(|_| "Upload failed.".into()));
    }
    response.json().await.map_err(|e| e.to_string())
}
