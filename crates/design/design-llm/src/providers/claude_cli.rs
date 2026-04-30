use std::path::PathBuf;
use std::process::Stdio;

use futures::stream::{BoxStream, StreamExt};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_stream::wrappers::LinesStream;

use crate::client::{AiAttachment, AiChunk, AiClient, AiError};

/// Client that drives the `claude` CLI in print mode with streaming JSON.
///
/// Invocation shape (mirrors the user's manual capture on 2026-04-21):
///
/// ```text
/// claude -p "<prompt>" --output-format stream-json --include-partial-messages --verbose [--model <name>]
/// ```
///
/// We parse only the text portions of each `stream_event.content_block_delta`
/// envelope (dropping `thinking_delta` and `signature_delta` — those are
/// internal reasoning and cryptographic attestation, not user-visible output).
/// The final `result` envelope carries the full text and success/error flag.
#[derive(Clone, Debug)]
pub struct ClaudeCliClient {
    binary: PathBuf,
    model: Option<String>,
}

impl ClaudeCliClient {
    pub fn new(binary: impl Into<PathBuf>, model: Option<String>) -> Self {
        Self {
            binary: binary.into(),
            model,
        }
    }
}

impl AiClient for ClaudeCliClient {
    fn complete(
        &self,
        prompt: String,
        // Claude CLI's print mode has no image-attach syntax — the design
        // doc (2026-04-21-ai-provider-selection) records this as a known v1
        // limitation. Reference labels + paths are still visible to the
        // model because the template renders them into `prompt`; we just do
        // not need to append anything here.
        _attachments: Vec<AiAttachment>,
    ) -> BoxStream<'static, Result<AiChunk, AiError>> {
        let binary = self.binary.clone();
        let model = self.model.clone();

        let stream = async_stream::stream! {
            let mut command = Command::new(&binary);
            command
                .arg("-p")
                .arg(&prompt)
                .arg("--output-format")
                .arg("stream-json")
                .arg("--include-partial-messages")
                .arg("--verbose");
            if let Some(m) = &model {
                command.arg("--model").arg(m);
            }
            command
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
            let mut final_text: Option<String> = None;
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

                match parse_line(&line, &mut accumulated, &mut final_text) {
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
                    let full = final_text.unwrap_or(accumulated);
                    yield Ok(AiChunk::Done { full_text: full });
                }
                Ok(status) => {
                    let stderr = drain_stderr(&mut child).await;
                    yield Err(AiError::Process(format!(
                        "claude exited {status}: {stderr}"
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

async fn drain_stderr(child: &mut tokio::process::Child) -> String {
    let mut buf = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        use tokio::io::AsyncReadExt;
        let _ = stderr.read_to_string(&mut buf).await;
    }
    buf
}

fn parse_line(
    line: &str,
    accumulated: &mut String,
    final_text: &mut Option<String>,
) -> Result<Option<AiChunk>, AiError> {
    let value: serde_json::Value = serde_json::from_str(line)
        .map_err(|e| AiError::Protocol(format!("invalid json: {e} :: {line}")))?;

    let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match kind {
        "stream_event" => {
            let Some(event) = value.get("event") else {
                return Ok(None);
            };
            let ev_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if ev_type != "content_block_delta" {
                return Ok(None);
            }
            let Some(delta) = event.get("delta") else {
                return Ok(None);
            };
            let delta_type = delta.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if delta_type != "text_delta" {
                // Skip thinking_delta / signature_delta — not user-visible.
                return Ok(None);
            }
            let Some(text) = delta.get("text").and_then(|v| v.as_str()) else {
                return Ok(None);
            };
            if text.is_empty() {
                return Ok(None);
            }
            accumulated.push_str(text);
            Ok(Some(AiChunk::Delta(text.to_string())))
        }
        "result" => {
            let is_error = value
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let result_text = value
                .get("result")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if is_error {
                Err(AiError::Process(format!(
                    "claude status=error: {}",
                    if result_text.is_empty() { "(no detail)" } else { &result_text }
                )))
            } else {
                *final_text = Some(result_text);
                Ok(None)
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
        let path = dir.path().join("claude-mock.sh");
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
    async fn parses_text_deltas_and_result_into_done() {
        let dir = mock_binary(
            r#"cat <<'EOF'
{"type":"system","subtype":"init","model":"claude-haiku-4-5"}
{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hello"}}}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" world"}}}
{"type":"result","subtype":"success","is_error":false,"result":"hello world","session_id":"s"}
EOF
"#,
        );
        let bin = dir.path().join("claude-mock.sh");
        let client = ClaudeCliClient::new(bin, None);

        let mut chunks = Vec::new();
        let mut stream = client.complete("prompt".into(), vec![]);
        while let Some(item) = stream.next().await {
            chunks.push(item.unwrap());
        }

        assert_eq!(
            chunks,
            vec![
                AiChunk::Delta("hello".into()),
                AiChunk::Delta(" world".into()),
                AiChunk::Done {
                    full_text: "hello world".into(),
                },
            ]
        );
    }

    #[tokio::test]
    async fn skips_thinking_and_signature_deltas() {
        let dir = mock_binary(
            r#"cat <<'EOF'
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"internal reasoning"}}}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"abc=="}}}
{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"OK"}}}
{"type":"result","subtype":"success","is_error":false,"result":"OK","session_id":"s"}
EOF
"#,
        );
        let bin = dir.path().join("claude-mock.sh");
        let client = ClaudeCliClient::new(bin, None);

        let mut chunks = Vec::new();
        let mut stream = client.complete("prompt".into(), vec![]);
        while let Some(item) = stream.next().await {
            chunks.push(item.unwrap());
        }

        assert_eq!(
            chunks,
            vec![
                AiChunk::Delta("OK".into()),
                AiChunk::Done {
                    full_text: "OK".into(),
                },
            ]
        );
    }

    #[tokio::test]
    async fn surfaces_result_error_as_process_error() {
        let dir = mock_binary(
            r#"cat <<'EOF'
{"type":"result","subtype":"success","is_error":true,"result":"Not logged in","session_id":"s"}
EOF
"#,
        );
        let bin = dir.path().join("claude-mock.sh");
        let client = ClaudeCliClient::new(bin, None);

        let mut stream = client.complete("prompt".into(), vec![]);
        let first = stream.next().await.expect("one item").unwrap_err();
        match first {
            AiError::Process(msg) => assert!(msg.contains("Not logged in")),
            other => panic!("expected Process, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn surfaces_protocol_error_on_invalid_json() {
        let dir = mock_binary(
            r#"cat <<'EOF'
this is not json
{"type":"result","subtype":"success","is_error":false,"result":"x"}
EOF
"#,
        );
        let bin = dir.path().join("claude-mock.sh");
        let client = ClaudeCliClient::new(bin, None);

        let mut stream = client.complete("prompt".into(), vec![]);
        let first = stream.next().await.expect("one item").unwrap_err();
        assert!(matches!(first, AiError::Protocol(_)));
    }

    #[tokio::test]
    async fn missing_binary_yields_process_error() {
        let client = ClaudeCliClient::new("/nonexistent/path/to/claude-xyz", None);
        let mut stream = client.complete("prompt".into(), vec![]);
        let first = stream.next().await.expect("one item").unwrap_err();
        assert!(matches!(first, AiError::Process(_)));
    }

}
