use std::time::Duration;

use base64::Engine;
use futures::stream::{BoxStream, StreamExt};
use serde_json::{Value, json};

use crate::client::{AiAttachment, AiChunk, AiClient, AiError};

/// Client for any OpenAI-compatible `chat/completions` endpoint.
/// Tested against **LM Studio** (http://host:1234/v1) — the same shape also
/// works for Ollama's OpenAI-compat server, vLLM, and llama.cpp's server.
#[derive(Clone, Debug)]
pub struct LmStudioClient {
    base_url: String,
    model: String,
}

impl LmStudioClient {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
        }
    }

    fn chat_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        if base.ends_with("/v1") || base.ends_with("/v1/") {
            format!("{}/chat/completions", base.trim_end_matches('/'))
        } else {
            format!("{base}/v1/chat/completions")
        }
    }
}

impl AiClient for LmStudioClient {
    fn complete(
        &self,
        prompt: String,
        attachments: Vec<AiAttachment>,
    ) -> BoxStream<'static, Result<AiChunk, AiError>> {
        let url = self.chat_url();
        let model = self.model.clone();

        let stream = async_stream::stream! {
            let content_parts = match build_user_content(&prompt, &attachments) {
                Ok(p) => p,
                Err(e) => {
                    yield Err(e);
                    return;
                }
            };

            let body = json!({
                "model": model,
                "stream": true,
                "messages": [{
                    "role": "user",
                    "content": content_parts,
                }],
            });

            let client = match reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    yield Err(AiError::Process(format!("build http client: {e}")));
                    return;
                }
            };

            let resp = match client.post(&url).json(&body).send().await {
                Ok(r) => r,
                Err(e) => {
                    yield Err(AiError::Process(format!("POST {url}: {e}")));
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                yield Err(AiError::Process(format!(
                    "lm-studio returned HTTP {status}: {body}"
                )));
                return;
            }

            let mut accumulated = String::new();
            let mut byte_stream = resp.bytes_stream();
            let mut buffer = Vec::<u8>::new();

            while let Some(chunk) = byte_stream.next().await {
                let bytes = match chunk {
                    Ok(b) => b,
                    Err(e) => {
                        yield Err(AiError::Process(format!("stream read: {e}")));
                        return;
                    }
                };
                buffer.extend_from_slice(&bytes);

                // SSE frames are terminated by a blank line (\n\n). We split on
                // `\n\n` and parse each frame independently; any trailing partial
                // frame is kept in `buffer` for the next iteration.
                while let Some(pos) = find_frame_end(&buffer) {
                    let frame_bytes = buffer.drain(..pos + 2).collect::<Vec<_>>();
                    let frame_end = frame_bytes.len().saturating_sub(2);
                    let frame = match std::str::from_utf8(&frame_bytes[..frame_end]) {
                        Ok(s) => s,
                        Err(e) => {
                            yield Err(AiError::Protocol(format!("non-utf8 frame: {e}")));
                            return;
                        }
                    };

                    match parse_sse_frame(frame, &mut accumulated) {
                        Ok(ParseOutcome::Delta(d)) => yield Ok(AiChunk::Delta(d)),
                        Ok(ParseOutcome::Done) => {
                            yield Ok(AiChunk::Done { full_text: accumulated.clone() });
                            return;
                        }
                        Ok(ParseOutcome::Ignore) => {}
                        Err(e) => {
                            yield Err(e);
                            return;
                        }
                    }
                }
            }

            // Stream closed without an explicit `[DONE]` — treat as natural end.
            yield Ok(AiChunk::Done { full_text: accumulated });
        };

        stream.boxed()
    }
}

fn find_frame_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(2).position(|w| w == b"\n\n")
}

enum ParseOutcome {
    Delta(String),
    Done,
    Ignore,
}

fn parse_sse_frame(frame: &str, accumulated: &mut String) -> Result<ParseOutcome, AiError> {
    // A frame is one or more lines. We only care about lines starting with `data: `.
    let mut data_parts: Vec<&str> = Vec::new();
    for line in frame.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix("data:") {
            data_parts.push(rest.trim_start());
        }
    }
    if data_parts.is_empty() {
        return Ok(ParseOutcome::Ignore);
    }

    let payload = data_parts.join("\n");
    if payload.trim() == "[DONE]" {
        return Ok(ParseOutcome::Done);
    }

    let value: Value = serde_json::from_str(&payload)
        .map_err(|e| AiError::Protocol(format!("invalid sse json: {e} :: {payload}")))?;

    let Some(choices) = value.get("choices").and_then(|v| v.as_array()) else {
        return Ok(ParseOutcome::Ignore);
    };
    let Some(first) = choices.first() else {
        return Ok(ParseOutcome::Ignore);
    };

    // OpenAI chat streaming: `choices[0].delta.content` carries incremental text.
    // Non-stream answer shape (used by some servers for the last frame) is
    // `choices[0].message.content`.
    let text = first
        .get("delta")
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str())
        .or_else(|| {
            first
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
        })
        .unwrap_or("");

    if text.is_empty() {
        return Ok(ParseOutcome::Ignore);
    }
    accumulated.push_str(text);
    Ok(ParseOutcome::Delta(text.to_string()))
}

fn build_user_content(prompt: &str, attachments: &[AiAttachment]) -> Result<Value, AiError> {
    if attachments.is_empty() {
        // Optimisation: a plain string is the canonical shape most OpenAI-compat
        // servers test against; use it when there are no attachments to widen
        // compatibility with smaller / older local servers.
        return Ok(Value::String(prompt.to_string()));
    }

    let mut parts: Vec<Value> = Vec::with_capacity(attachments.len() + 1);
    parts.push(json!({ "type": "text", "text": prompt }));
    for att in attachments {
        let bytes = std::fs::read(&att.path).map_err(|e| {
            AiError::Process(format!("read attachment {}: {e}", att.path.display()))
        })?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let data_url = format!("data:{};base64,{}", att.mime_type, b64);
        parts.push(json!({
            "type": "image_url",
            "image_url": { "url": data_url },
        }));
    }
    Ok(Value::Array(parts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Minimal HTTP/1.1 SSE mock. Starts a listener on an ephemeral port,
    /// accepts ONE request, reads the request body (for assertions), and
    /// writes a canned SSE response back. Returns the bound address and
    /// a oneshot receiver carrying the request body JSON.
    async fn mock_sse_server(
        sse_body: &'static str,
    ) -> (SocketAddr, tokio::sync::oneshot::Receiver<Value>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.expect("accept");
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            let mut headers_done = false;
            let mut content_len: Option<usize> = None;
            let mut body_start: usize = 0;

            loop {
                let n = sock.read(&mut tmp).await.expect("read");
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
                if !headers_done {
                    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        headers_done = true;
                        body_start = pos + 4;
                        let headers = std::str::from_utf8(&buf[..pos]).unwrap();
                        for line in headers.split("\r\n") {
                            if let Some(v) =
                                line.to_ascii_lowercase().strip_prefix("content-length:")
                            {
                                content_len = v.trim().parse().ok();
                            }
                        }
                    }
                }
                if headers_done {
                    if let Some(cl) = content_len {
                        if buf.len() - body_start >= cl {
                            break;
                        }
                    }
                }
            }

            let body_bytes = &buf[body_start..];
            let body_json: Value = serde_json::from_slice(body_bytes).unwrap_or(Value::Null);
            let _ = tx.send(body_json);

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{sse_body}"
            );
            sock.write_all(response.as_bytes()).await.expect("write");
            sock.shutdown().await.ok();
        });

        (addr, rx)
    }

    async fn mock_sse_server_with_status(status: u16, body: &'static str) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.expect("accept");
            let mut tmp = [0u8; 4096];
            let _ = sock.read(&mut tmp).await;
            let response = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(response.as_bytes()).await.expect("write");
            sock.shutdown().await.ok();
        });

        addr
    }

    #[tokio::test]
    async fn parses_sse_chat_completion_deltas_and_done() {
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\ndata: [DONE]\n\n";
        let (addr, _body_rx) = mock_sse_server(sse).await;

        let client = LmStudioClient::new(format!("http://{addr}"), "test-model");
        let mut stream = client.complete("hi".into(), vec![]);

        let mut chunks = Vec::new();
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
    async fn surfaces_http_error_status() {
        let addr = mock_sse_server_with_status(500, "internal boom").await;
        let client = LmStudioClient::new(format!("http://{addr}"), "test-model");
        let mut stream = client.complete("hi".into(), vec![]);
        let first = stream.next().await.expect("one item").unwrap_err();
        match first {
            AiError::Process(msg) => {
                assert!(msg.contains("500"));
                assert!(msg.contains("internal boom"));
            }
            other => panic!("expected Process, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn attaches_images_as_data_url_parts() {
        // Write a small PNG-ish file to disk so build_user_content can read it.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.png");
        std::fs::write(&path, [0x89, 0x50, 0x4E, 0x47, 0x01, 0x02]).unwrap();

        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n";
        let (addr, body_rx) = mock_sse_server(sse).await;

        let client = LmStudioClient::new(format!("http://{addr}"), "vision-model");
        let mut stream = client.complete(
            "describe".into(),
            vec![AiAttachment::image(&path, "image/png", "a.png")],
        );
        while stream.next().await.is_some() {}

        let body = body_rx.await.expect("body captured");
        let content = &body["messages"][0]["content"];
        assert!(
            content.is_array(),
            "expected multimodal array, got {content}"
        );
        let parts = content.as_array().unwrap();
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[0]["text"], "describe");
        assert_eq!(parts[1]["type"], "image_url");
        let url = parts[1]["image_url"]["url"].as_str().unwrap();
        assert!(url.starts_with("data:image/png;base64,"), "got {url}");
    }

    #[tokio::test]
    async fn surfaces_protocol_error_on_malformed_frame() {
        let sse = "data: {not json}\n\n";
        let (addr, _body_rx) = mock_sse_server(sse).await;
        let client = LmStudioClient::new(format!("http://{addr}"), "m");
        let mut stream = client.complete("hi".into(), vec![]);
        let first = stream.next().await.expect("one item").unwrap_err();
        assert!(matches!(first, AiError::Protocol(_)));
    }

    #[test]
    fn chat_url_appends_v1_when_missing() {
        let c = LmStudioClient::new("http://localhost:1234", "m");
        assert_eq!(c.chat_url(), "http://localhost:1234/v1/chat/completions");

        let c = LmStudioClient::new("http://localhost:1234/", "m");
        assert_eq!(c.chat_url(), "http://localhost:1234/v1/chat/completions");

        let c = LmStudioClient::new("http://localhost:1234/v1", "m");
        assert_eq!(c.chat_url(), "http://localhost:1234/v1/chat/completions");

        let c = LmStudioClient::new("http://localhost:1234/v1/", "m");
        assert_eq!(c.chat_url(), "http://localhost:1234/v1/chat/completions");
    }
}
