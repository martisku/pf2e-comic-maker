use crate::projects::Session;
use comic_model::{Layout, Panel};
use dioxus::prelude::*;

pub fn label(panel: &Panel, number: Option<usize>) -> String {
    if !panel.title.trim().is_empty() {
        panel.title.clone()
    } else {
        number
            .map(|n| format!("Panel {n}"))
            .unwrap_or_else(|| "Untitled panel".into())
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Confirmation {
    Page(u32),
    Panel(u32),
    Layout(u32, Layout),
}

#[component]
pub fn PageManager(
    mut session: Session,
    chapter_id: u32,
    mut selected_page: Signal<u32>,
    mut selected_panel: Signal<u32>,
) -> Element {
    let mut pending = use_signal(|| None::<Confirmation>);
    let mut error = use_signal(|| None::<String>);
    let Some(snapshot) = (session.state)() else {
        return rsx! {};
    };
    let Some(chapter) = snapshot.workspace.chapter(chapter_id).cloned() else {
        return rsx! {};
    };
    let blocked = (session.busy)() || (session.uncertain)() || session.loading();
    let active = chapter
        .pages
        .iter()
        .find(|p| p.id == selected_page())
        .or_else(|| chapter.pages.first());
    let active_id = active.map(|p| p.id);
    let panel = chapter
        .panel_ids
        .iter()
        .find(|id| **id == selected_panel())
        .or_else(|| chapter.panel_ids.first())
        .and_then(|id| snapshot.workspace.panels.iter().find(|p| p.id == *id))
        .cloned();
    let prompt = pending().and_then(|action| match action {
        Confirmation::Page(id) => chapter.pages.iter().position(|p| p.id == id).map(|index| format!("Remove page {}? Its {} placed panels will become unplaced; their images and notes will remain.", index + 1, chapter.pages[index].slots.iter().flatten().count())),
        Confirmation::Panel(id) => panel.as_ref().filter(|p| p.id == id).map(|p| format!("Delete {} and its images? Shared assets and references will remain.", label(p, None))),
        Confirmation::Layout(id, layout) => chapter.pages.iter().find(|p| p.id == id).map(|p| format!("Change layout? {} panels will become unplaced and remain available for reassignment.", p.slots.iter().skip(layout.capacity()).flatten().count())),
    });
    rsx! {
        section { class: "chapter-manager", "aria-label": "Page management",
            h2 { "Pages" }
            if let Some(message) = error() { p { role: "alert", "{message}" } }
            button { disabled: blocked, onclick: move |_| {
                if (session.busy)() || (session.uncertain)() { return; }
                if let Some(s) = session.state.write().as_mut() { match s.workspace.add_page(chapter_id) { Ok(id) => { selected_page.set(id); session.dirty.set(true); }, Err(e) => error.set(Some(e)) } }
            }, "New page" }
            if chapter.pages.is_empty() { p { "No pages. Panels remain editable and unplaced. Create a page to compose them." } }
            ol { class: "chapter-list",
                for (index, page) in chapter.pages.iter().enumerate() {
                    { let id = page.id; rsx! { li { key: "{id}", class: "chapter-row",
                        button { "aria-pressed": Some(id) == active_id, onclick: move |_| selected_page.set(id), "Open page {index + 1}" }
                        button { disabled: blocked || index == 0, onclick: move |_| {
                            if (session.busy)() || (session.uncertain)() { return; }
                            if let Some(s) = session.state.write().as_mut() { s.workspace.move_page(chapter_id, id, -1); session.dirty.set(true); }
                        }, "Earlier" }
                        button { disabled: blocked || index + 1 == chapter.pages.len(), onclick: move |_| {
                            if (session.busy)() || (session.uncertain)() { return; }
                            if let Some(s) = session.state.write().as_mut() { s.workspace.move_page(chapter_id, id, 1); session.dirty.set(true); }
                        }, "Later" }
                        button { disabled: blocked, onclick: move |_| pending.set(Some(Confirmation::Page(id))), "Remove page {index + 1}" }
                    } } }
                }
            }
            if let Some(page) = active {
                { let id = page.id; rsx! { label { key: "{id}:{page.layout.css_class()}:{pending().is_some()}", "Layout"
                    select { disabled: blocked, value: page.layout.css_class(), "aria-label": "Page layout", oninput: move |event| {
                        if (session.busy)() || (session.uncertain)() { return; }
                        let layout = match event.value().as_str() { "full" => Layout::Full, "wide" => Layout::Wide, _ => Layout::Grid };
                        if let Some(s) = session.state.write().as_mut() && let Some(page) = s.workspace.chapter_mut(chapter_id).and_then(|c| c.pages.iter_mut().find(|p| p.id == id)) {
                            if page.slots.iter().skip(layout.capacity()).any(Option::is_some) { pending.set(Some(Confirmation::Layout(id, layout))); } else { page.change_layout(layout); session.dirty.set(true); }
                        }
                    },
                        option { value: "grid", selected: page.layout == Layout::Grid, "2 × 2 grid" } option { value: "wide", selected: page.layout == Layout::Wide, "Wide + two" } option { value: "full", selected: page.layout == Layout::Full, "Full page" }
                    }
                } } }
            }
            if let Some(panel) = panel {
                { let id = panel.id; rsx! { div { class: "chapter-actions",
                    label { "Move panel to chapter"
                        select { disabled: blocked, value: "{chapter_id}", "aria-label": "Move panel to chapter", oninput: move |e| {
                            if (session.busy)() || (session.uncertain)() { return; }
                            if let Ok(destination) = e.value().parse::<u32>() && let Some(s) = session.state.write().as_mut() {
                                match s.workspace.move_panel(id, destination) { Ok(()) => { selected_panel.set(0); session.dirty.set(true); }, Err(e) => error.set(Some(e)) }
                            }
                        }, for chapter in &snapshot.workspace.chapters { option { value: "{chapter.id}", "{chapter.title}" } } }
                    }
                    button { disabled: blocked, onclick: move |_| pending.set(Some(Confirmation::Panel(id))), "Delete panel" }
                } } }
            }
        }
        if let Some(prompt) = prompt {
            dialog { id: "panel-page-dialog", role: "dialog", "aria-modal": "true", class: "unsaved-dialog", "aria-label": "Panel and page confirmation", onmounted: move |_| {
                use wasm_bindgen::JsCast;
                if let Some(element) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id("panel-page-dialog")) && let Ok(dialog) = element.dyn_into::<web_sys::HtmlDialogElement>() { let _ = dialog.show_modal(); }
            }, oncancel: move |_| pending.set(None),
                h2 { "Confirm change" } p { "{prompt}" }
                button { onclick: move |_| pending.set(None), "Cancel" }
                button { disabled: blocked, onclick: move |_| {
                    if (session.busy)() || (session.uncertain)() { return; }
                    if let Some(action) = pending() && let Some(s) = session.state.write().as_mut() {
                        match action {
                            Confirmation::Page(id) => { s.workspace.remove_page(chapter_id, id); if selected_page() == id { selected_page.set(0); } },
                            Confirmation::Panel(id) => { s.workspace.remove_panel(id); selected_panel.set(0); },
                            Confirmation::Layout(id, layout) => { if let Some(page) = s.workspace.chapter_mut(chapter_id).and_then(|c| c.pages.iter_mut().find(|p| p.id == id)) { page.change_layout(layout); } }
                        }
                        session.dirty.set(true);
                    }
                    pending.set(None);
                }, "Confirm" }
            }
        }
    }
}
