use comic_model::{Asset, AssetKind, ProjectSnapshot};
use dioxus::prelude::*;

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
pub fn AssetLibrary(state: Signal<Option<ProjectSnapshot>>, dirty: Signal<bool>) -> Element {
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
                        snapshot.workspace.assets.push(Asset { id: id.clone(), kind: kind(), name: format!("New {}", kind().label().to_lowercase()), description: String::new(), design_notes: String::new(), character_size: None });
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
                                if !users.is_empty() {
                                    div { class: "asset-usage", p { "Used by these panels. Remove their associations before deleting this asset." }
                                        ul { for (id, title) in users { li { "Panel {id}: {title}" } } }
                                    }
                                }
                                button { disabled: !snapshot.workspace.asset_users(&asset.id).is_empty(), onclick: move |_| pending.set(Some((project_delete.clone(), id_delete.clone()))), "Delete asset" }
                                if confirming {
                                    div { role: "group", "aria-label": "Confirm asset deletion", p { "Delete {asset.name}? This cannot be undone." }
                                        button { onclick: move |_| pending.set(None), "Cancel asset deletion" }
                                        button { onclick: move |_| {
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
                }
            }
        }
    }
}
