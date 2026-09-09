#[cfg(target_os = "macos")]
mod macos {
    use std::{
        collections::BTreeMap,
        sync::{
            Mutex,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
    };

    use serde::Serialize;
    use tauri::{AppHandle, Emitter, Manager, State, Url};

    const OPENED_DOCUMENTS_EVENT: &str = "macos-local-ro-opened";

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct OpenedDocument {
        id: String,
        name: String,
    }

    #[derive(Default)]
    struct OpenedDocuments {
        frontend_ready: AtomicBool,
        next_id: AtomicU64,
        pending: Mutex<Option<Vec<OpenedDocument>>>,
        grants: Mutex<BTreeMap<String, Url>>,
    }

    impl OpenedDocuments {
        fn grant(&self, urls: Vec<Url>) -> Vec<OpenedDocument> {
            let mut grants = self
                .grants
                .lock()
                .expect("opened-document grants are available");
            urls.into_iter()
                .map(|url| {
                    let id = self.next_id.fetch_add(1, Ordering::Relaxed).to_string();
                    let name = url
                        .to_file_path()
                        .ok()
                        .and_then(|path| {
                            path.file_name()
                                .map(|value| value.to_string_lossy().into_owned())
                        })
                        .unwrap_or_else(|| url.to_string());
                    grants.insert(id.clone(), url);
                    OpenedDocument { id, name }
                })
                .collect()
        }
    }

    #[tauri::command]
    #[allow(clippy::needless_pass_by_value)] // Tauri extracts managed state by value.
    fn take_pending_opened_documents(state: State<'_, OpenedDocuments>) -> Vec<OpenedDocument> {
        state.frontend_ready.store(true, Ordering::Release);
        state
            .pending
            .lock()
            .expect("pending opened-document latch is available")
            .take()
            .unwrap_or_default()
    }

    #[tauri::command]
    #[allow(clippy::needless_pass_by_value)] // Tauri command extraction owns decoded arguments/state.
    fn read_opened_document(
        id: String,
        state: State<'_, OpenedDocuments>,
    ) -> Result<Vec<u8>, String> {
        let url = state
            .grants
            .lock()
            .map_err(|_| "The native opened-document grant is unavailable.".to_owned())?
            .remove(&id)
            .ok_or_else(|| "The native opened-document grant is no longer available.".to_owned())?;
        let path = url.to_file_path().map_err(|()| {
            "The operating system did not provide a readable local file.".to_owned()
        })?;
        std::fs::read(path)
            .map_err(|error| format!("Could not read the selected local file: {error}"))
    }

    #[tauri::command]
    #[allow(clippy::needless_pass_by_value)] // Tauri extracts managed state by value.
    fn release_opened_documents(ids: Vec<String>, state: State<'_, OpenedDocuments>) {
        if let Ok(mut grants) = state.grants.lock() {
            for id in ids {
                grants.remove(&id);
            }
        }
    }

    fn deliver_opened_documents(app: &AppHandle, urls: Vec<Url>) {
        let state = app.state::<OpenedDocuments>();
        let documents = state.grant(urls);
        if state.frontend_ready.load(Ordering::Acquire) {
            let _ = app.emit(OPENED_DOCUMENTS_EVENT, documents);
            return;
        }

        // This is the sole cold-start latch. It is taken and cleared once the
        // frontend listener is ready; warm events are never queued here.
        let mut pending = state
            .pending
            .lock()
            .expect("pending opened-document latch is available");
        if let Some(latched_documents) = pending.as_mut() {
            // There is still only one cold-start handoff. If LaunchServices
            // reports another open before the frontend is ready, retaining all
            // grants makes the existing single-document ingress reject safely
            // rather than silently choosing or dropping a file.
            latched_documents.extend(documents);
        } else {
            *pending = Some(documents);
        }
    }

    pub fn run() {
        let app = tauri::Builder::default()
            .manage(OpenedDocuments::default())
            .invoke_handler(tauri::generate_handler![
                take_pending_opened_documents,
                read_opened_document,
                release_opened_documents
            ])
            .build(tauri::generate_context!())
            .expect("error while building Tachiko Work Designer");
        app.run(|app, event| {
            if let tauri::RunEvent::Opened { urls } = event {
                deliver_opened_documents(app, urls);
            }
        });
    }
}

#[cfg(target_os = "macos")]
fn main() {
    macos::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("tachiko-designer-desktop is a macOS-only host");
}
