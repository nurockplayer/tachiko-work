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

        fn take_pending(&self) -> Vec<OpenedDocument> {
            // Readiness and the sole cold-start latch share this mutex boundary.
            // A delivery that observed `false` before we acquired the latch must
            // recheck readiness after acquiring it, so it cannot enqueue after
            // this one-shot take has already completed.
            let mut pending = self
                .pending
                .lock()
                .expect("pending opened-document latch is available");
            self.frontend_ready.store(true, Ordering::Release);
            pending.take().unwrap_or_default()
        }

        fn latch_before_frontend_ready(
            &self,
            documents: Vec<OpenedDocument>,
        ) -> Result<(), Vec<OpenedDocument>> {
            self.latch_before_frontend_ready_after_initial_read(documents, || {})
        }

        fn latch_before_frontend_ready_after_initial_read(
            &self,
            documents: Vec<OpenedDocument>,
            after_initial_read: impl FnOnce(),
        ) -> Result<(), Vec<OpenedDocument>> {
            if self.frontend_ready.load(Ordering::Acquire) {
                return Err(documents);
            }

            after_initial_read();

            let mut pending = self
                .pending
                .lock()
                .expect("pending opened-document latch is available");
            if self.frontend_ready.load(Ordering::Acquire) {
                return Err(documents);
            }

            if let Some(latched_documents) = pending.as_mut() {
                // There is still only one cold-start handoff. If LaunchServices
                // reports another open before the frontend is ready, retaining all
                // grants makes the existing single-document ingress reject safely
                // rather than silently choosing or dropping a file.
                latched_documents.extend(documents);
            } else {
                *pending = Some(documents);
            }
            Ok(())
        }
    }

    #[tauri::command]
    #[allow(clippy::needless_pass_by_value)] // Tauri extracts managed state by value.
    fn take_pending_opened_documents(state: State<'_, OpenedDocuments>) -> Vec<OpenedDocument> {
        state.take_pending()
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
        if let Err(documents) = state.latch_before_frontend_ready(documents) {
            let _ = app.emit(OPENED_DOCUMENTS_EVENT, documents);
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

    #[cfg(test)]
    mod tests {
        use std::{
            sync::{Arc, mpsc},
            thread,
        };

        use super::{OpenedDocument, OpenedDocuments};

        fn document(id: &str) -> OpenedDocument {
            OpenedDocument {
                id: id.to_owned(),
                name: format!("{id}.ro"),
            }
        }

        #[test]
        fn pending_documents_are_taken_once_before_warm_delivery() {
            let state = OpenedDocuments::default();
            assert!(
                state
                    .latch_before_frontend_ready(vec![document("cold")])
                    .is_ok()
            );

            let pending = state.take_pending();
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].id, "cold");
            assert!(state.take_pending().is_empty());

            let warm = state
                .latch_before_frontend_ready(vec![document("warm")])
                .expect_err("ready frontend must receive direct delivery");
            assert_eq!(warm.len(), 1);
            assert_eq!(warm[0].id, "warm");
        }

        #[test]
        fn pending_latch_rechecks_readiness_after_the_one_shot_take() {
            let state = Arc::new(OpenedDocuments::default());
            let (initial_read, initial_read_complete) = mpsc::channel();
            let (resume_latch, resume_latch_wait) = mpsc::channel();
            let delivery_state = Arc::clone(&state);

            let delivery = thread::spawn(move || {
                delivery_state.latch_before_frontend_ready_after_initial_read(
                    vec![document("cold")],
                    || {
                        initial_read
                            .send(())
                            .expect("test must observe the initial readiness read");
                        resume_latch_wait
                            .recv()
                            .expect("test must resume the blocked delivery");
                    },
                )
            });

            initial_read_complete
                .recv()
                .expect("delivery must read not-ready before the frontend take");
            assert!(state.take_pending().is_empty());
            resume_latch
                .send(())
                .expect("delivery must remain available after the one-shot take");

            let documents = delivery
                .join()
                .expect("delivery thread must not panic")
                .expect_err("delivery must not latch after the one-shot take");
            assert_eq!(documents.len(), 1);
            assert_eq!(documents[0].id, "cold");
            assert!(state.take_pending().is_empty());
        }
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
