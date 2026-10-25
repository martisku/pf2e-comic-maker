use crate::projects::Session;
use comic_model::ProjectSnapshot;
use dioxus::prelude::*;
use gloo_net::http::{Request, Response};
use wasm_bindgen::JsCast;

const MAX_BACKUP_BYTES: u64 = 268_435_456;

async fn failure(response: Response) -> String {
    response
        .text()
        .await
        .unwrap_or_else(|_| "The server rejected the backup request.".into())
}

fn download(bytes: &[u8], name: &str) -> Result<(), String> {
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts)
        .map_err(|_| "Could not prepare the backup download.".to_string())?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "Could not prepare the backup download.".to_string())?;
    let result = (|| {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or("Browser document unavailable.")?;
        let anchor = document
            .create_element("a")
            .map_err(|_| "Could not create download.")?
            .dyn_into::<web_sys::HtmlAnchorElement>()
            .map_err(|_| "Could not create download.")?;
        anchor.set_href(&url);
        anchor.set_download(name);
        let body = document.body().ok_or("Browser document unavailable.")?;
        body.append_child(&anchor)
            .map_err(|_| "Could not create download.")?;
        anchor.click();
        anchor.remove();
        Ok(())
    })();
    // Give the browser time to consume the object URL before releasing it.
    spawn(async move {
        gloo_timers::future::TimeoutFuture::new(30_000).await;
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    result.map_err(str::to_string)
}

#[component]
pub fn ExportBackup(mut session: Session) -> Element {
    let mut message = use_signal(|| None::<String>);
    let mut error = use_signal(|| None::<String>);
    let blocked = (session.busy)()
        || (session.backup)()
        || (session.dirty)()
        || (session.uncertain)()
        || (session.error)().is_some()
        || session.loading();
    rsx! {
        section { class: "backup-controls", "aria-label": "Project backup",
            button { disabled: blocked, onclick: move |_| {
                if blocked { return; }
                let Some(snapshot) = session.state.peek().clone() else { return; };
                session.backup.set(true);
                session.busy.set(true);
                message.set(Some("Exporting project backup…".into())); error.set(None);
                let activation = session.activation();
                spawn(async move {
                    let result = async {
                        let response = Request::get(&format!("/api/projects/{}/backup?revision={}", snapshot.project_id, snapshot.revision))
                            .send().await.map_err(|_| "Could not download the backup. Try again.".to_string())?;
                        if !response.ok() { return Err(failure(response).await); }
                        let bytes = response.binary().await.map_err(|_| "Could not read the backup download. Try again.".to_string())?;
                        download(&bytes, &format!("project-{}.comicbackup", snapshot.project_id))
                    }.await;
                    if session.activation() != activation { return; }
                    session.backup.set(false); session.busy.set(false);
                    match result {
                        Ok(()) => message.set(Some("Backup download started.".into())),
                        Err(problem) => { message.set(None); error.set(Some(problem)); }
                    }
                });
            }, "Export project backup" }
            p { "One self-contained file with saved project data and images. Maximum backup size: 256 MiB." }
            if blocked && !(session.backup)() {
                p { "Finish saving edits and images before exporting. Resolve any save or upload error using Retry or Reload saved work." }
            }
            if let Some(text) = message() { p { role: "status", "{text}" } }
            if let Some(text) = error() { p { role: "alert", "{text}" } }
        }
    }
}

#[derive(Clone)]
struct Attempt {
    id: String,
    bytes: Vec<u8>,
    name: String,
}

#[component]
pub fn RestoreBackup(mut session: Session) -> Element {
    let mut attempt = use_signal(|| None::<Attempt>);
    let mut message = use_signal(|| None::<String>);
    let mut error = use_signal(|| None::<String>);
    let mut ambiguous = use_signal(|| false);
    let mut restore = move || {
        if (session.busy)() || ((session.backup)() && !ambiguous()) {
            return;
        }
        let Some(request) = attempt.peek().clone() else {
            return;
        };
        session.busy.set(true);
        session.backup.set(true);
        error.set(None);
        message.set(Some("Restoring project backup…".into()));
        let activation = session.activation();
        spawn(async move {
            let result = async {
                let response = Request::post("/api/projects/restore")
                    .header("content-type", "application/octet-stream")
                    .header("x-restore-id", &request.id)
                    .body(request.bytes.clone()).map_err(|_| (false, "Could not prepare backup upload.".into()))?
                    .send().await.map_err(|_| (true, "Restoration outcome is unknown. Retry restoration with this same file.".into()))?;
                if !response.ok() {
                    let uncertain = response.status() >= 500;
                    return Err((uncertain, failure(response).await));
                }
                let snapshot: ProjectSnapshot = response.json().await
                    .map_err(|_| (true, "Restoration outcome is unknown. Retry restoration with this same file.".into()))?;
                if snapshot.project_id != request.id {
                    return Err((true, "Unexpected restored project identity. Retry restoration with this same file.".into()));
                }
                Ok(snapshot)
            }.await;
            if session.activation() != activation {
                return;
            }
            session.busy.set(false);
            session.backup.set(false);
            match result {
                Ok(snapshot) => session.open_restored(snapshot),
                Err((uncertain, problem)) => {
                    session.backup.set(uncertain);
                    ambiguous.set(uncertain);
                    message.set(None);
                    error.set(Some(problem));
                }
            }
        });
    };
    rsx! {
        section { class: "backup-controls", "aria-label": "Restore backup",
            h2 { "Restore a project backup" }
            p { "Choose a .comicbackup file up to 256 MiB. Restoration creates an independent project copy and preserves existing projects." }
            input { r#type: "file", "aria-label": "Restore project backup", accept: ".comicbackup,application/octet-stream",
                disabled: (session.busy)() || (session.backup)() || ambiguous(),
                onchange: move |event| {
                    if (session.busy)() || ambiguous() { return; }
                    attempt.set(None); error.set(None); message.set(None);
                    let Some(file) = event.files().into_iter().next() else { return; };
                    if file.size() > MAX_BACKUP_BYTES {
                        error.set(Some("Choose a backup no larger than 256 MiB.".into())); return;
                    }
                    session.busy.set(true); session.backup.set(true);
                    message.set(Some("Reading project backup…".into()));
                    spawn(async move {
                        let result = file.read_bytes().await;
                        session.busy.set(false); session.backup.set(false); message.set(None);
                        match result {
                            Ok(bytes) => attempt.set(Some(Attempt { id: uuid::Uuid::new_v4().to_string(), bytes: bytes.to_vec(), name: file.name() })),
                            Err(_) => error.set(Some("Could not read the selected backup. Choose the file again.".into())),
                        }
                    });
                }
            }
            if let Some(request) = attempt() { p { "Selected: {request.name}" } }
            button { disabled: (session.busy)() || attempt().is_none(), onclick: move |_| restore(),
                if ambiguous() { "Retry restoration" } else { "Restore backup" }
            }
            if ambiguous() { p { "Retry uses the same restoration request, so it cannot create a duplicate copy. Keep this page open until the result is confirmed." } }
            if let Some(text) = message() { p { role: "status", "{text}" } }
            if let Some(text) = error() { p { role: "alert", "{text}" } }
        }
    }
}
