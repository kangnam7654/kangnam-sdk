use std::path::PathBuf;
use std::process::Stdio;

use futures::stream::{BoxStream, StreamExt};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_stream::wrappers::LinesStream;

use crate::client::{AiAttachment, AiChunk, AiClient, AiError};

#[derive(Clone, Debug)]
pub struct GeminiCliClient {
    binary: PathBuf,
    extra_args: Vec<String>,
}

impl GeminiCliClient {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
            extra_args: Vec::new(),
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }
}

impl AiClient for GeminiCliClient {
    fn complete(
        &self,
        prompt: String,
        // The `@<path>` tokens are already in `prompt` (rendered by the Tera
        // template). We still use `attachments` here to derive the set of
        // parent directories that must be whitelisted via
        // `--include-directories`. See `include_dirs_for_attachments` for why
        // this is load-bearing — without it Gemini CLI refuses to read files
        // outside its notion of "workspace" and the `@path` tokens silently
        // become literal text, so the model sees nothing.
        attachments: Vec<AiAttachment>,
    ) -> BoxStream<'static, Result<AiChunk, AiError>> {
        let binary = self.binary.clone();
        let extra = self.extra_args.clone();
        let include_dirs = include_dirs_for_attachments(&attachments);

        let stream = async_stream::stream! {
            let mut command = Command::new(&binary);
            command
                .arg("-p")
                .arg(&prompt)
                .arg("-o")
                .arg("stream-json");
            for dir in &include_dirs {
                command.arg("--include-directories").arg(dir);
            }
            command
                .args(&extra)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);

            let mut child = match command.spawn() {
                Ok(c) => c,
                Err(e) => {
                    yield Err(AiError::Process(format!("spawn {}: {e}", binary.display())));
                    return;
                }
            };

            let stdout = match child.stdout.take() {
                Some(s) => s,
                None => {
                    yield Err(AiError::Process("missing stdout".into()));
                    return;
                }
            };

            let mut accumulated = String::new();
            let mut lines = LinesStream::new(BufReader::new(stdout).lines());

            while let Some(line) = lines.next().await {
                let line = match line {
                    Ok(l) => l,
                    Err(e) => {
                        yield Err(AiError::Process(format!("stdout read: {e}")));
                        return;
                    }
                };
                if line.trim().is_empty() {
                    continue;
                }

                match parse_line(&line, &mut accumulated) {
                    Ok(Some(chunk)) => yield Ok(chunk),
                    Ok(None) => {}
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                }
            }

            match child.wait().await {
                Ok(status) if status.success() => {
                    yield Ok(AiChunk::Done { full_text: accumulated });
                }
                Ok(status) => {
                    let stderr = drain_stderr(&mut child).await;
                    yield Err(AiError::Process(format!(
                        "gemini exited {status}: {stderr}"
                    )));
                }
                Err(e) => {
                    yield Err(AiError::Process(format!("wait: {e}")));
                }
            }
        };

        stream.boxed()
    }
}

/// Unique parent directories of every attachment path, sorted for stable
/// command-line output (and easier testing).
///
/// Gemini CLI (v0.38.1) enforces a "workspace" on `-p` non-interactive mode:
/// `@<path>` tokens are rejected with `Path not in workspace` when they
/// resolve outside `cwd` or `~/.gemini/tmp/<project>`. Canvas stores
/// references under `~/.canvas/references/<id>/pages/`, which is always
/// outside — so every run needs `--include-directories` covering the
/// rasterized-pages directories. See the verification session on 2026-04-21
/// (same ref folder, same PNGs): without the flag the model hallucinates
/// trying to read via the `read_file` tool; with it, description matches.
fn include_dirs_for_attachments(attachments: &[AiAttachment]) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    for att in attachments {
        if let Some(parent) = att.path.parent() {
            // Empty parent (e.g. path was just "file.png") means cwd — no need
            // to whitelist. Skip to keep the command-line tidy.
            let s = parent.to_string_lossy();
            if !s.is_empty() {
                seen.insert(s.into_owned());
            }
        }
    }
    seen.into_iter().collect()
}

async fn drain_stderr(child: &mut tokio::process::Child) -> String {
    let mut buf = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        use tokio::io::AsyncReadExt;
        let _ = stderr.read_to_string(&mut buf).await;
    }
    buf
}

fn parse_line(line: &str, accumulated: &mut String) -> Result<Option<AiChunk>, AiError> {
    let value: serde_json::Value = serde_json::from_str(line)
        .map_err(|e| AiError::Protocol(format!("invalid json: {e} :: {line}")))?;

    let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match kind {
        "message" => {
            let role = value.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "assistant" {
                return Ok(None);
            }
            let Some(content) = value.get("content").and_then(|v| v.as_str()) else {
                return Ok(None);
            };
            accumulated.push_str(content);
            Ok(Some(AiChunk::Delta(content.to_string())))
        }
        "result" => {
            let status = value.get("status").and_then(|v| v.as_str()).unwrap_or("");
            if status == "success" {
                Ok(None)
            } else {
                let detail = value
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(no detail)");
                Err(AiError::Process(format!(
                    "gemini status={status}: {detail}"
                )))
            }
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    fn mock_binary(script: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("gemini-mock.sh");
        let mut f = std::fs::File::create(&path).expect("create script");
        writeln!(f, "#!/usr/bin/env bash").unwrap();
        writeln!(f, "set -eu").unwrap();
        writeln!(f, "{}", script).unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        dir
    }

    #[tokio::test]
    async fn parses_stream_json_into_deltas_and_done() {
        let dir = mock_binary(
            r#"cat <<'EOF'
{"type":"init","timestamp":"t","session_id":"s","model":"m"}
{"type":"message","timestamp":"t","role":"user","content":"hi"}
{"type":"message","timestamp":"t","role":"assistant","content":"hello"}
{"type":"message","timestamp":"t","role":"assistant","content":" world"}
{"type":"result","timestamp":"t","status":"success","stats":{}}
EOF"#,
        );
        let bin = dir.path().join("gemini-mock.sh");
        let client = GeminiCliClient::new(bin);

        let chunks: Vec<_> = client
            .complete("ignored".into(), vec![])
            .collect::<Vec<_>>()
            .await;

        let unwrapped: Vec<AiChunk> = chunks.into_iter().map(|r| r.expect("ok")).collect();
        assert_eq!(
            unwrapped,
            vec![
                AiChunk::Delta("hello".into()),
                AiChunk::Delta(" world".into()),
                AiChunk::Done {
                    full_text: "hello world".into()
                },
            ]
        );
    }

    #[tokio::test]
    async fn surfaces_error_when_result_status_is_error() {
        let dir = mock_binary(
            r#"cat <<'EOF'
{"type":"init","timestamp":"t","session_id":"s","model":"m"}
{"type":"result","timestamp":"t","status":"error","error":"quota exhausted"}
EOF"#,
        );
        let bin = dir.path().join("gemini-mock.sh");
        let client = GeminiCliClient::new(bin);

        let chunks: Vec<_> = client
            .complete("ignored".into(), vec![])
            .collect::<Vec<_>>()
            .await;
        let last = chunks.last().expect("at least one item");
        assert!(matches!(last, Err(AiError::Process(msg)) if msg.contains("quota exhausted")));
    }

    #[tokio::test]
    async fn surfaces_error_when_binary_missing() {
        let client = GeminiCliClient::new("/nonexistent/path/to/gemini-xyz");
        let chunks: Vec<_> = client
            .complete("ignored".into(), vec![])
            .collect::<Vec<_>>()
            .await;
        assert_eq!(chunks.len(), 1);
        assert!(matches!(chunks[0], Err(AiError::Process(_))));
    }

    #[test]
    fn include_dirs_dedupes_parents_and_sorts() {
        let atts = vec![
            AiAttachment::image(
                "/Users/x/.canvas/references/abc/pages/0.png",
                "image/png",
                "",
            ),
            AiAttachment::image(
                "/Users/x/.canvas/references/abc/pages/1.png",
                "image/png",
                "",
            ),
            // Different ref → different parent directory; must appear too.
            AiAttachment::image(
                "/Users/x/.canvas/references/xyz/original.png",
                "image/png",
                "",
            ),
        ];
        let dirs = include_dirs_for_attachments(&atts);
        assert_eq!(
            dirs,
            vec![
                "/Users/x/.canvas/references/abc/pages".to_string(),
                "/Users/x/.canvas/references/xyz".to_string(),
            ]
        );
    }

    #[test]
    fn include_dirs_empty_for_no_attachments() {
        assert!(include_dirs_for_attachments(&[]).is_empty());
    }

    #[tokio::test]
    async fn surfaces_protocol_error_on_invalid_json() {
        let dir = mock_binary(
            r#"cat <<'EOF'
not json at all
EOF"#,
        );
        let bin = dir.path().join("gemini-mock.sh");
        let client = GeminiCliClient::new(bin);

        let chunks: Vec<_> = client
            .complete("ignored".into(), vec![])
            .collect::<Vec<_>>()
            .await;
        let last = chunks.last().expect("at least one");
        assert!(matches!(last, Err(AiError::Protocol(_))));
    }
}
