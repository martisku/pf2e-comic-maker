use crate::projects::Session;
use dioxus::prelude::*;

#[component]
pub fn ChapterManager(
    mut session: Session,
    mut selected: Signal<u32>,
    mut selected_panel: Signal<u32>,
) -> Element {
    let mut pending = use_signal(|| None::<(String, u32)>);
    let mut local_error = use_signal(|| None::<String>);
    let snapshot = (session.state)();
    let Some(snapshot) = snapshot else {
        return rsx! {};
    };
    let chapter = snapshot
        .workspace
        .chapters
        .iter()
        .find(|c| c.id == selected())
        .or_else(|| snapshot.workspace.chapters.first())
        .cloned();
    let active_id = chapter.as_ref().map(|c| c.id).unwrap_or(0);
    let blocked = (session.busy)() || (session.uncertain)();
    let deletion = pending()
        .filter(|(project, _)| project == &snapshot.project_id)
        .and_then(|(_, id)| {
            snapshot
                .workspace
                .chapters
                .iter()
                .find(|c| c.id == id)
                .cloned()
        });
    rsx! {
        style { {"
            .chapter-manager { margin: 0 0 1.5rem; padding: 1rem; border: 1px solid #b99b6355; border-radius: 12px; background: #142923; }
            .chapter-heading, .chapter-row, .chapter-actions { display: flex; align-items: center; flex-wrap: wrap; gap: .6rem; }
            .chapter-heading { justify-content: space-between; }
            .chapter-heading h2 { margin: 0; }
            .chapter-manager button, #chapter-delete-dialog button { padding: 10px 14px; border: 1px solid #58604b; border-radius: 3px; background: #263025; color: var(--paper); }
            .chapter-manager button:hover:not(:disabled), #chapter-delete-dialog button:hover:not(:disabled) { border-color: var(--brass); background: #303c2c; }
            .chapter-manager button:disabled, #chapter-delete-dialog button:disabled { cursor: not-allowed; opacity: .55; }
            .chapter-list { display: grid; gap: .5rem; padding-left: 1.4rem; }
            .chapter-row button:first-child { flex: 1; text-align: left; min-width: 130px; }
            .chapter-row button[aria-pressed=true] { border-color: var(--brass); background: #c7ac781a; }
            .chapter-details { display: grid; grid-template-columns: 1fr 2fr; gap: 1rem; margin: 1rem 0; }
            .chapter-details label { display: grid; gap: .4rem; }
            .chapter-details input, .chapter-details textarea { width: 100%; }
            @media (max-width: 700px) { .chapter-details { grid-template-columns: 1fr; } .chapter-manager button, #chapter-delete-dialog button { min-height: 44px; } }
        "} }
        section { class: "chapter-manager", "aria-label": "Chapter management",
            div { class: "chapter-heading",
                h2 { "Chapters" }
                button { disabled: blocked, onclick: move |_| {
                    if (session.busy)() || (session.uncertain)() { return; }
                    if let Some(snapshot) = session.state.write().as_mut() {
                        match snapshot.workspace.add_chapter() {
                            Ok(id) => { selected.set(id); selected_panel.set(0); session.dirty.set(true); local_error.set(None); }
                            Err(message) => local_error.set(Some(message)),
                        }
                    }
                }, "New chapter" }
            }
            if let Some(message) = local_error() { p { role: "alert", "{message}" } }
            if snapshot.workspace.chapters.is_empty() {
                p { "No chapters yet. Create a chapter to start planning your story." }
            }
            ol { class: "chapter-list",
                for (index, chapter) in snapshot.workspace.chapters.iter().enumerate() {
                    { let id = chapter.id; rsx! {
                        li { key: "{id}",
                            div { class: "chapter-row",
                                button { "aria-label": "Open chapter {chapter.title}", "aria-pressed": id == active_id,
                                    onclick: move |_| { selected.set(id); selected_panel.set(0); },
                                    "{index + 1}. {chapter.title} · {chapter.panel_ids.len()} panels"
                                }
                                button { "aria-label": "Move chapter {chapter.title} up", disabled: blocked || index == 0,
                                    onclick: move |_| {
                                        if let Some(snapshot) = session.state.write().as_mut() { snapshot.workspace.move_chapter(id, -1); session.dirty.set(true); }
                                    }, "↑" }
                                button { "aria-label": "Move chapter {chapter.title} down", disabled: blocked || index + 1 == snapshot.workspace.chapters.len(),
                                    onclick: move |_| {
                                        if let Some(snapshot) = session.state.write().as_mut() { snapshot.workspace.move_chapter(id, 1); session.dirty.set(true); }
                                    }, "↓" }
                            }
                        }
                    } }
                }
            }
            if let Some(chapter) = chapter {
                div { class: "chapter-details",
                    label { "Chapter title"
                        input { "aria-label": "Chapter title", value: "{chapter.title}", maxlength: "500",
                            oninput: move |event| {
                                if let Some(snapshot) = session.state.write().as_mut()
                                    && let Some(chapter) = snapshot.workspace.chapter_mut(active_id) {
                                    chapter.title = event.value(); session.dirty.set(true);
                                }
                            }
                        }
                    }
                    label { "Chapter description"
                        textarea { "aria-label": "Chapter description", rows: "2", value: "{chapter.description}",
                            oninput: move |event| {
                                if let Some(snapshot) = session.state.write().as_mut()
                                    && let Some(chapter) = snapshot.workspace.chapter_mut(active_id) {
                                    chapter.description = event.value(); session.dirty.set(true);
                                }
                            }
                        }
                    }
                }
                div { class: "chapter-actions",
                    button { disabled: blocked, onclick: move |_| {
                        if (session.busy)() || (session.uncertain)() { return; }
                        if let Some(snapshot) = session.state.write().as_mut() {
                            match snapshot.workspace.add_panel(active_id) {
                                Ok(id) => { selected_panel.set(id); session.dirty.set(true); local_error.set(None); }
                                Err(message) => local_error.set(Some(message)),
                            }
                        }
                    }, "Add panel" }
                    button { disabled: blocked, onclick: move |_| {
                        if let Some(snapshot) = session.state.peek().as_ref() { pending.set(Some((snapshot.project_id.clone(), active_id))); }
                    }, "Delete chapter" }
                    small { "New panels remain unplaced until assigned a page position." }
                }
            }
        }
        if let Some(chapter) = deletion {
            div { class: "dialog-backdrop",
                dialog { id: "chapter-delete-dialog", class: "unsaved-dialog", role: "dialog", "aria-modal": "true", "aria-label": "Delete chapter confirmation",
                    onmounted: move |_| {
                        use wasm_bindgen::JsCast;
                        if let Some(element) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id("chapter-delete-dialog"))
                            && let Ok(dialog) = element.dyn_into::<web_sys::HtmlDialogElement>() { let _ = dialog.show_modal(); }
                    },
                    oncancel: move |_| pending.set(None),
                    h2 { "Delete {chapter.title}?" }
                    p { "This will delete {chapter.panel_ids.len()} panels and this chapter’s pages. Shared assets and references will remain." }
                    button { onclick: move |_| pending.set(None), "Cancel chapter deletion" }
                    button { disabled: blocked, onclick: move |_| {
                        if (session.busy)() || (session.uncertain)() { return; }
                        if let Some((project, id)) = pending()
                            && let Some(snapshot) = session.state.write().as_mut()
                            && snapshot.project_id == project {
                            if snapshot.workspace.remove_chapter(id) { session.dirty.set(true); }
                            if selected() == id { selected.set(0); selected_panel.set(0); }
                        }
                        pending.set(None);
                    }, "Confirm chapter deletion" }
                }
            }
        }
    }
}
