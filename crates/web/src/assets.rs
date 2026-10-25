use crate::projects::Session;
use comic_model::{Asset, AssetKind, ProjectSnapshot};
use dioxus::html::FileData;
use dioxus::prelude::*;
use gloo_net::http::Request;
use wasm_bindgen::JsCast;

fn edit_asset(
    mut state: Signal<Option<ProjectSnapshot>>,
    mut dirty: Signal<bool>,
    id: &str,
    edit: impl FnOnce(&mut Asset),
) {
    if let Some(snapshot) = state.write().as_mut()
        && let Some(asset) = snapshot
            .workspace
            .assets
            .iter_mut()
            .find(|asset| asset.id == id)
    {
        edit(asset);
        dirty.set(true);
    }
}

#[component]
pub fn AssetLibrary(
    state: Signal<Option<ProjectSnapshot>>,
    dirty: Signal<bool>,
    session: Session,
) -> Element {
    let mut state = state;
    let mut dirty = dirty;
    let mut kind = use_signal(|| AssetKind::Character);
    let mut search = use_signal(String::new);
    let mut selected = use_signal(String::new);
    // Bind confirmation to a project as well as an asset, so a project switch
    // can never apply a pending destructive action to another workspace.
    let mut pending = use_signal(|| None::<(String, String)>);
    let mut error = use_signal(|| None::<String>);
    let Some(snapshot) = state.read().clone() else {
        return rsx! {};
    };
    let project_id = snapshot.project_id.clone();
    let assets = snapshot.workspace.assets.clone();
    let active = assets.iter().find(|asset| asset.id == selected()).cloned();
    let query = search().to_lowercase();

    rsx! {
        style { {include_str!("../assets/asset-library.css")} }
        main { class: "asset-library", "aria-label": "Asset library",
            header { h2 { "Asset library" } p { "Shared characters, locations and equipment for your story." } }
            div { class: "asset-library-toolbar",
                label { "Asset type"
                    select { "aria-label": "Asset type", value: kind().label(),
                        oninput: move |event| { kind.set(match event.value().as_str() { "Location" => AssetKind::Location, "Equipment" => AssetKind::Equipment, _ => AssetKind::Character }); selected.set(String::new()); pending.set(None); },
                        for option in [AssetKind::Character, AssetKind::Location, AssetKind::Equipment] {
                            option { value: option.label(), "{option.label()}" }
                        }
                    }
                }
                label { "Search by name"
                    input { "aria-label": "Search assets", value: "{search}", oninput: move |event| search.set(event.value()) }
                }
                button { disabled: assets.len() >= 200, onclick: move |_| {
                    let id = uuid::Uuid::new_v4().to_string();
                    if let Some(snapshot) = state.write().as_mut() && snapshot.workspace.assets.len() < 200 {
                        snapshot.workspace.assets.push(Asset { id: id.clone(), kind: kind(), name: format!("New {}", kind().label().to_lowercase()), description: String::new(), design_notes: String::new(), character_size: None, reference_images: vec![] });
                        dirty.set(true); selected.set(id); search.set(String::new()); pending.set(None); error.set(None);
                    }
                }, "Create {kind().label().to_lowercase()}" }
            }
            div { class: "asset-library-columns",
                nav { class: "asset-records", "aria-label": "Project assets",
                    if !assets.iter().any(|asset| asset.kind == kind() && asset.name.to_lowercase().contains(&query)) {
                        p { "No matching assets. Create one or change your search." }
                    }
                    for asset in assets.iter().filter(|asset| asset.kind == kind() && asset.name.to_lowercase().contains(&query)) {
                        {
                            let id = asset.id.clone();
                            rsx! { button { "aria-pressed": selected() == asset.id, onclick: move |_| { selected.set(id.clone()); pending.set(None); error.set(None); }, "{asset.name}" } }
                        }
                    }
                }
                section { class: "asset-details", "aria-label": "Asset details",
                    if let Some(asset) = active {
                        {
                            let id_name = asset.id.clone(); let id_description = asset.id.clone();
                            let id_notes = asset.id.clone(); let id_size = asset.id.clone();
                            let id_delete = asset.id.clone(); let id_confirm = asset.id.clone();
                            let project_delete = project_id.clone(); let project_confirm = project_id.clone();
                            let users = snapshot.workspace.asset_users(&asset.id);
                            let confirming = pending() == Some((project_id.clone(), asset.id.clone()));
                            rsx! {
                                p { class: "eyebrow", "{asset.kind.label()}" }
                                label { "Asset name" input { "aria-label": "Asset name", maxlength: "500", value: "{asset.name}", oninput: move |event| edit_asset(state, dirty, &id_name, |asset| asset.name = event.value()) } }
                                label { "Description" textarea { "aria-label": "Asset description", rows: "3", maxlength: "20000", value: "{asset.description}", oninput: move |event| edit_asset(state, dirty, &id_description, |asset| asset.description = event.value()) } }
                                label { "Visual design notes" textarea { "aria-label": "Visual design notes", rows: "3", maxlength: "20000", value: "{asset.design_notes}", oninput: move |event| edit_asset(state, dirty, &id_notes, |asset| asset.design_notes = event.value()) } }
                                if asset.kind == AssetKind::Character {
                                    label { "Height or relative size (optional)" input { "aria-label": "Character size", maxlength: "500", value: "{asset.character_size.as_deref().unwrap_or_default()}", oninput: move |event| edit_asset(state, dirty, &id_size, |asset| { let value = event.value(); asset.character_size = if value.is_empty() { None } else { Some(value) }; }) } }
                                }
                                ReferenceGallery { state, dirty, asset_id: asset.id.clone(), session: Some(session) }
                                if !users.is_empty() {
                                    div { class: "asset-usage", p { "Used by these panels. Remove their associations before deleting this asset." }
                                        ul { for (id, title) in users { li { "Panel {id}: {title}" } } }
                                    }
                                }
                                button { disabled: (session.busy)() || !snapshot.workspace.asset_users(&asset.id).is_empty(), onclick: move |_| pending.set(Some((project_delete.clone(), id_delete.clone()))), "Delete asset" }
                                if confirming {
                                    div { role: "group", "aria-label": "Confirm asset deletion", p { "Delete {asset.name}? This cannot be undone." }
                                        button { onclick: move |_| pending.set(None), "Cancel asset deletion" }
                                        button { disabled: (session.busy)(), onclick: move |_| {
                                            if (session.busy)() { return; }
                                            if let Some(snapshot) = state.write().as_mut() && snapshot.project_id == project_confirm {
                                                match snapshot.workspace.remove_asset(&id_confirm) {
                                                    Ok(()) => { dirty.set(true); selected.set(String::new()); error.set(None); },
                                                    Err(message) => error.set(Some(message)),
                                                }
                                            }
                                            pending.set(None);
                                        }, "Confirm delete asset" }
                                    }
                                }
                                if let Some(message) = error() { p { role: "alert", "{message}" } }
                            }
                        }
                    } else { p { "Select an asset to view and edit its shared definition." } }
                }
            }
        }
    }
}

#[component]
pub fn PanelAssetsEditor(
    state: Signal<Option<ProjectSnapshot>>,
    dirty: Signal<bool>,
    panel_id: u32,
) -> Element {
    let Some(snapshot) = state.read().clone() else {
        return rsx! {};
    };
    let Some(panel) = snapshot
        .workspace
        .panels
        .iter()
        .find(|panel| panel.id == panel_id)
    else {
        return rsx! {};
    };
    rsx! {
        style { {include_str!("../assets/asset-library.css")} }
        section { class: "panel-assets", "aria-label": "Panel assets",
            h3 { "Shared assets" }
            if snapshot.workspace.assets.is_empty() { p { "Create characters, locations and equipment in the Asset library to use them here." } }
            for kind in [AssetKind::Character, AssetKind::Location, AssetKind::Equipment] {
                fieldset { legend { "{kind.label()}" }
                    if kind == AssetKind::Location {
                        {
                            let mut state = state; let mut dirty = dirty;
                            rsx! { select { "aria-label": "Panel location", value: "{panel.assets.location.as_deref().unwrap_or_default()}", oninput: move |event| {
                                if let Some(snapshot) = state.write().as_mut() && let Some(panel) = snapshot.workspace.panels.iter_mut().find(|panel| panel.id == panel_id) {
                                    let value = event.value(); panel.assets.location = if value.is_empty() { None } else { Some(value) }; dirty.set(true);
                                }
                            }, option { value: "", "No location" }
                                for asset in snapshot.workspace.assets.iter().filter(|asset| asset.kind == kind) { option { value: "{asset.id}", "{asset.name}" } }
                            } }
                        }
                    } else {
                        for asset in snapshot.workspace.assets.iter().filter(|asset| asset.kind == kind) {
                            {
                                let id = asset.id.clone(); let mut state = state; let mut dirty = dirty;
                                rsx! { label { class: "asset-checkbox",
                                    input { r#type: "checkbox", checked: panel.assets.contains(&asset.id), onchange: move |event| {
                                        if let Some(snapshot) = state.write().as_mut() && let Some(panel) = snapshot.workspace.panels.iter_mut().find(|panel| panel.id == panel_id) {
                                            let ids = if kind == AssetKind::Character { &mut panel.assets.characters } else { &mut panel.assets.equipment };
                                            ids.retain(|value| value != &id); if event.checked() { ids.push(id.clone()); } dirty.set(true);
                                        }
                                    } } "{asset.name}"
                                } }
                            }
                        }
                    }
                }
            }
            for asset in snapshot.workspace.assets.iter().filter(|asset| panel.assets.contains(&asset.id)) {
                details { class: "shared-asset-definition", summary { "{asset.name} — shared definition" }
                    p { "{asset.description}" } p { "{asset.design_notes}" }
                    if let Some(size) = &asset.character_size { p { "Size: {size}" } }
                    ReferenceGallery { state, dirty, asset_id: asset.id.clone() }
                }
            }
        }
    }
}

#[component]
fn ReferenceGallery(
    state: Signal<Option<ProjectSnapshot>>,
    dirty: Signal<bool>,
    asset_id: String,
    session: Option<Session>,
) -> Element {
    let mut expanded = use_signal(|| None::<String>);
    let mut removal = use_signal(|| None::<(String, String, String)>);
    let Some(snapshot) = state.read().clone() else {
        return rsx! {};
    };
    let Some(asset) = snapshot
        .workspace
        .assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .cloned()
    else {
        return rsx! {};
    };
    let blocked = session.is_some_and(|session| (session.busy)() || (session.error)().is_some());
    let upload_disabled = blocked || dirty() || asset.reference_images.len() >= 100;
    let upload_asset = asset_id.clone();
    rsx! {
        section { class: "reference-gallery", "aria-label": "Visual references",
            h4 { "Visual references" }
            if let Some(mut session) = session {
                label { class: "reference-upload", "Import reference image"
                    input { r#type: "file", "aria-label": "Import reference image", accept: "image/png,image/jpeg,image/webp", disabled: upload_disabled,
                        onchange: move |event| {
                            if (session.busy)() || dirty() || (session.error)().is_some() { return; }
                            let files = event.files();
                            if files.is_empty() { return; }
                            let Some(before) = state.peek().clone() else { return; };
                            let activation = session.activation();
                            let asset_id = upload_asset.clone();
                            session.busy.set(true); session.status.set("Uploading reference…".into());
                            spawn(async move {
                                let result = upload_reference(&before.project_id, &asset_id, before.revision, files).await;
                                if session.activation() != activation { return; }
                                match result {
                                    Ok(saved) if saved.project_id == before.project_id => {
                                        if let Some(current) = session.state.write().as_mut() {
                                            current.revision = saved.revision;
                                            if let Some(local) = current.workspace.assets.iter_mut().find(|asset| asset.id == asset_id)
                                                && let Some(remote) = saved.workspace.assets.iter().find(|asset| asset.id == asset_id) {
                                                local.reference_images = remote.reference_images.clone();
                                            }
                                        }
                                        session.status.set(if dirty() { "Unsaved changes" } else { "Saved" }.into());
                                    }
                                    Ok(_) => { session.error.set(Some("The server returned a different project. Reload saved work.".into())); session.status.set("Upload failed".into()); }
                                    Err(message) => { session.uncertain.set(message.contains("outcome is unknown")); session.error.set(Some(message)); session.status.set("Upload failed".into()); }
                                }
                                session.busy.set(false);
                            });
                        }
                    }
                }
                small { "PNG, JPEG or WebP · up to 20 MB · 100 references per asset" }
                if dirty() { small { "Your edits are saving before upload is available." } }
            }
            if asset.reference_images.is_empty() { p { "No reference images attached." } }
            div { class: "reference-thumbnails",
                for reference in &asset.reference_images {
                    {
                        let open_id = reference.id.clone();
                        let label_id = reference.id.clone(); let description_id = reference.id.clone();
                        let label_asset = asset_id.clone(); let description_asset = asset_id.clone();
                        let remove_target = (snapshot.project_id.clone(), asset_id.clone(), reference.id.clone());
                        let confirm_target = remove_target.clone();
                        let mut state = state; let mut dirty = dirty;
                        rsx! {
                            article { class: "reference-card",
                                button { class: "reference-thumbnail", "aria-label": "Open reference {reference.label}", onclick: move |_| expanded.set(Some(open_id.clone())),
                                    img { src: "{reference.url}", alt: "{reference.label}", loading: "lazy" }
                                }
                                if session.is_some() {
                                    label { "Reference label" input { "aria-label": "Reference label", maxlength: "500", value: "{reference.label}", disabled: blocked,
                                        oninput: move |event| edit_asset(state, dirty, &label_asset, |asset| { if let Some(reference) = asset.reference_images.iter_mut().find(|reference| reference.id == label_id) { reference.label = event.value(); } })
                                    } }
                                    label { "Reference description" textarea { "aria-label": "Reference description", maxlength: "20000", rows: "2", value: "{reference.description}", disabled: blocked,
                                        oninput: move |event| edit_asset(state, dirty, &description_asset, |asset| { if let Some(reference) = asset.reference_images.iter_mut().find(|reference| reference.id == description_id) { reference.description = event.value(); } })
                                    } }
                                    button { disabled: blocked, onclick: move |_| removal.set(Some(remove_target.clone())), "Remove reference" }
                                    if removal() == Some(confirm_target.clone()) {
                                        div { role: "group", "aria-label": "Confirm reference removal",
                                            p { "Remove {reference.label}? Imported panel images will be preserved." }
                                            button { onclick: move |_| removal.set(None), "Cancel reference removal" }
                                            button { disabled: blocked, onclick: move |_| {
                                                if let Some(snapshot) = state.write().as_mut() && snapshot.project_id == confirm_target.0
                                                    && let Some(asset) = snapshot.workspace.assets.iter_mut().find(|asset| asset.id == confirm_target.1) {
                                                    asset.reference_images.retain(|reference| reference.id != confirm_target.2); dirty.set(true);
                                                }
                                                removal.set(None);
                                            }, "Confirm remove reference" }
                                        }
                                    }
                                } else { strong { "{reference.label}" } p { "{reference.description}" } }
                            }
                        }
                    }
                }
            }
            if let Some(reference) = asset.reference_images.iter().find(|reference| Some(&reference.id) == expanded().as_ref()) {
                dialog { id: "reference-dialog-{asset_id}", class: "reference-lightbox", "aria-label": "Reference image: {reference.label}",
                    onmounted: {
                        let dialog_id = format!("reference-dialog-{asset_id}");
                        move |_| {
                            if let Some(document) = web_sys::window().and_then(|window| window.document())
                                && let Some(element) = document.get_element_by_id(&dialog_id)
                                && let Ok(dialog) = element.dyn_into::<web_sys::HtmlDialogElement>() {
                                let _ = dialog.show_modal();
                            }
                        }
                    },
                    oncancel: move |_| expanded.set(None),
                    div { class: "reference-lightbox-content",
                        button { autofocus: true, onclick: move |_| expanded.set(None), "Close reference image" }
                        h4 { "{reference.label}" } img { src: "{reference.url}", alt: "{reference.label}" } p { "{reference.description}" }
                    }
                }
            }
        }
    }
}

async fn upload_reference(
    project: &str,
    asset: &str,
    revision: u64,
    files: Vec<FileData>,
) -> Result<ProjectSnapshot, String> {
    let file = files
        .first()
        .ok_or_else(|| "No image selected.".to_string())?;
    if file.size() > comic_model::MAX_IMAGE_BYTES as u64 {
        return Err("Choose an image no larger than 20 MB (20,000,000 bytes).".into());
    }
    let bytes = file.read_bytes().await.map_err(|error| error.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let mut failure = String::from("No server response");
    for attempt in 0..3 {
        let response = Request::post(&format!(
            "/api/projects/{project}/assets/{asset}/references"
        ))
        .header("content-type", "application/octet-stream")
        .header("x-comic-revision", &revision.to_string())
        .header("x-comic-upload-id", &id)
        .body(bytes.to_vec())
        .map_err(|error| error.to_string())?
        .send()
        .await;
        failure = match response {
            Ok(response) if response.ok() => {
                if let Ok(snapshot) = response.json().await {
                    return Ok(snapshot);
                }
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
        "Upload outcome is unknown ({failure}; reference {id}). Reload saved work before selecting the file again."
    ))
}
