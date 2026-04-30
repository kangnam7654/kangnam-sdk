//! Eight design-mode tools the agent calls during a discovery → preview
//! → done loop. All implement [`kangnam_harness_runtime::DesignTool`]
//! and route side effects through the runtime's `ToolCtx` so a host
//! can swap the fs / web / image / preview-bridge callbacks for tests.
//!
//! | Tool | Suspends turn? | Notes |
//! |---|---|---|
//! | `ask` | yes (QuestionForm) | posts a `<question-form>` schema and awaits user response |
//! | `scaffold` | no | copies a skill's `assets/` into the workspace |
//! | `skill` | no | lazy-loads a skill body from the catalog |
//! | `preview` | yes (Preview) | renders an artifact and awaits screenshot + console |
//! | `tweaks` | no | declares cross-file editable controls |
//! | `done` | no | turn-end signal; returns marker payload |
//! | `brand_asset_extract` | no | multi-step web-fetch + grep hex + write `brand-spec.md` |
//! | `gen_image` | no | calls the host image API and writes the result to disk |
//!
//! `tools::all_tools()` returns the eight as `Arc<dyn DesignTool>` so a
//! host can iterate to advertise capabilities to the agent.

pub mod catalog;
pub mod tools;

pub use catalog::{SkillCatalog, StaticSkillCatalog};
pub use tools::all_tools;

#[cfg(test)]
pub(crate) mod tests {
    //! Shared test helpers — ToolCtx fakes that record fs / web /
    //! image / bridge calls so individual tool tests don't have to
    //! restate the boilerplate.

    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use kangnam_harness_runtime::{
        FrontendBridge, FsCallbacks, ImageCallbacks, ToolCtx, ToolError, WebCallbacks,
    };
    use serde_json::Value;
    use tempfile::TempDir;
    use tokio::sync::oneshot;

    pub type StrReplaceLog = Arc<Mutex<Vec<(PathBuf, String, String)>>>;

    pub fn record_str_replaces() -> StrReplaceLog {
        Arc::new(Mutex::new(Vec::new()))
    }

    pub struct RealFs {
        pub recorder: Option<StrReplaceLog>,
    }

    #[async_trait]
    impl FsCallbacks for RealFs {
        async fn read(&self, path: &Path) -> Result<Vec<u8>, ToolError> {
            tokio::fs::read(path).await.map_err(Into::into)
        }
        async fn write(&self, path: &Path, body: &[u8]) -> Result<(), ToolError> {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(path, body).await.map_err(Into::into)
        }
        async fn str_replace(&self, path: &Path, old: &str, new: &str) -> Result<(), ToolError> {
            if let Some(rec) = &self.recorder {
                rec.lock().unwrap().push((path.to_path_buf(), old.into(), new.into()));
            }
            Ok(())
        }
    }

    pub struct StubWeb {
        pub body: Vec<u8>,
    }

    #[async_trait]
    impl WebCallbacks for StubWeb {
        async fn fetch(&self, _url: &str) -> Result<Vec<u8>, ToolError> {
            Ok(self.body.clone())
        }
    }

    pub struct StubImage;

    #[async_trait]
    impl ImageCallbacks for StubImage {
        async fn generate(&self, _prompt: &str, out: &Path) -> Result<PathBuf, ToolError> {
            // Pretend we wrote a 1-byte file so callers can `exists()` if they want.
            if let Some(parent) = out.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            tokio::fs::write(out, b"\0").await.ok();
            Ok(out.to_path_buf())
        }
    }

    pub struct StubBridge;

    #[async_trait]
    impl FrontendBridge for StubBridge {
        async fn register_question_form(
            &self,
            _payload: &Value,
        ) -> Result<(String, oneshot::Receiver<Value>), ToolError> {
            let (_tx, rx) = oneshot::channel();
            Ok(("await-q-1".into(), rx))
        }
        async fn register_preview(
            &self,
            _payload: &Value,
        ) -> Result<(String, oneshot::Receiver<Value>), ToolError> {
            let (_tx, rx) = oneshot::channel();
            Ok(("await-p-1".into(), rx))
        }
    }

    pub fn test_ctx() -> ToolCtx {
        ToolCtx {
            working_dir: PathBuf::from("/tmp"),
            session_id: "test-session".into(),
            fs: Arc::new(RealFs { recorder: None }),
            web: Arc::new(StubWeb { body: vec![] }),
            image: Arc::new(StubImage),
            bridge: Arc::new(StubBridge),
        }
    }

    pub fn test_ctx_with_workspace() -> (ToolCtx, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolCtx {
            working_dir: dir.path().to_path_buf(),
            session_id: "test-session".into(),
            fs: Arc::new(RealFs { recorder: None }),
            web: Arc::new(StubWeb { body: vec![] }),
            image: Arc::new(StubImage),
            bridge: Arc::new(StubBridge),
        };
        (ctx, dir)
    }

    pub fn test_ctx_with_web_workspace(body: Vec<u8>) -> (ToolCtx, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolCtx {
            working_dir: dir.path().to_path_buf(),
            session_id: "test-session".into(),
            fs: Arc::new(RealFs { recorder: None }),
            web: Arc::new(StubWeb { body }),
            image: Arc::new(StubImage),
            bridge: Arc::new(StubBridge),
        };
        (ctx, dir)
    }

    pub fn test_ctx_recording(rec: StrReplaceLog) -> ToolCtx {
        ToolCtx {
            working_dir: PathBuf::from("/tmp"),
            session_id: "test-session".into(),
            fs: Arc::new(RealFs { recorder: Some(rec) }),
            web: Arc::new(StubWeb { body: vec![] }),
            image: Arc::new(StubImage),
            bridge: Arc::new(StubBridge),
        }
    }
}
