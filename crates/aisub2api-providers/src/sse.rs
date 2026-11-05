use aisub2api_core::SseEvent;
use futures::{StreamExt, stream::BoxStream};
use serde_json::Value;

use crate::http::{fmt_reqwest_err, next_upstream_stream_item};
use crate::provider::ProviderError;

/// Stream native Messages events with the same completion boundary for
/// Anthropic and Copilot. A clean HTTP EOF alone does not complete a message.
pub(crate) fn stream_anthropic_response(
    response: reqwest::Response,
    max_sse_frame_bytes: u64,
) -> BoxStream<'static, Result<SseEvent, ProviderError>> {
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    tokio::spawn(async move {
        let mut decoder = SseDecoder::new(max_sse_frame_bytes);
        let mut byte_stream = response.bytes_stream();
        let failure = loop {
            let chunk = match next_upstream_stream_item(byte_stream.next()).await {
                Ok(Some(Ok(chunk))) => chunk,
                Ok(Some(Err(error))) => break ProviderError::Network(fmt_reqwest_err(&error)),
                Ok(None) => {
                    break ProviderError::Network(
                        "upstream Anthropic stream ended before message_stop".to_string(),
                    );
                }
                Err(error) => break error,
            };
            if let Err(error) = decoder.push(&chunk) {
                let _ = tx
                    .send(Err(ProviderError::ResponseTooLarge(format!(
                        "SSE frame exceeds configured limit of {} bytes",
                        error.limit
                    ))))
                    .await;
                return;
            }
            while let Some(frame) = decoder.next_frame() {
                let Some(event) = parse_anthropic_sse_frame(&frame) else {
                    continue;
                };
                let terminal = is_anthropic_terminal_event(&event);
                if tx.send(Ok(event)).await.is_err() || terminal {
                    return;
                }
            }
        };

        // A complete final frame may have no trailing blank line. Recover it
        // even when the HTTP transport reports a chunk/length error at EOF.
        match decoder.finish() {
            Ok(Some(frame)) => {
                if let Some(event) = parse_anthropic_sse_frame(&frame) {
                    let terminal = is_anthropic_terminal_event(&event);
                    if tx.send(Ok(event)).await.is_err() || terminal {
                        return;
                    }
                }
            }
            Ok(None) => {}
            Err(error) => {
                let _ = tx
                    .send(Err(ProviderError::ResponseTooLarge(format!(
                        "SSE frame exceeds configured limit of {} bytes",
                        error.limit
                    ))))
                    .await;
                return;
            }
        }
        let _ = tx.send(Err(failure)).await;
    });
    Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
}

pub(crate) fn parse_anthropic_sse_frame(text: &str) -> Option<SseEvent> {
    let frame = parse_sse_frame(text)?;
    let data = serde_json::from_str::<Value>(frame.data.trim()).unwrap_or(Value::Null);
    Some(SseEvent {
        event: frame.event.unwrap_or_default(),
        data,
    })
}

fn is_anthropic_terminal_event(event: &SseEvent) -> bool {
    matches!(event.event.as_str(), "message_stop" | "error")
        || matches!(event.data["type"].as_str(), Some("message_stop" | "error"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SseFrameTooLarge {
    pub(crate) limit: u64,
}

#[derive(Debug)]
pub(crate) struct SseDecoder {
    buffer: Vec<u8>,
    start: usize,
    max_frame_bytes: u64,
}

impl SseDecoder {
    pub(crate) fn new(max_frame_bytes: u64) -> Self {
        Self {
            buffer: Vec::new(),
            start: 0,
            max_frame_bytes,
        }
    }

    /// Feed transport bytes into the decoder without ever retaining an
    /// incomplete logical frame larger than the configured budget.
    pub(crate) fn push(&mut self, chunk: &[u8]) -> Result<(), SseFrameTooLarge> {
        self.compact_consumed();
        let pending = &self.buffer[self.start..];
        let tail_start = last_frame_end(pending).unwrap_or(0);
        validate_frame_budget(
            pending[tail_start..]
                .iter()
                .copied()
                .chain(chunk.iter().copied()),
            self.max_frame_bytes,
        )?;
        self.buffer.extend_from_slice(chunk);
        Ok(())
    }

    pub(crate) fn next_frame(&mut self) -> Option<String> {
        let search = &self.buffer[self.start..];
        let (relative_end, delimiter_len) = find_frame_end(search)?;
        let frame_start = self.start;
        let frame_end = frame_start + relative_end;
        self.start = frame_end + delimiter_len;
        let frame = String::from_utf8_lossy(&self.buffer[frame_start..frame_end]).into_owned();
        self.compact_consumed();
        Some(frame)
    }

    pub(crate) fn finish(&mut self) -> Result<Option<String>, SseFrameTooLarge> {
        let remaining = &self.buffer[self.start..];
        let remaining_len = u64::try_from(remaining.len()).map_err(|_| SseFrameTooLarge {
            limit: self.max_frame_bytes,
        })?;
        if remaining_len > self.max_frame_bytes {
            self.buffer.clear();
            self.start = 0;
            return Err(SseFrameTooLarge {
                limit: self.max_frame_bytes,
            });
        }
        if remaining.iter().all(|byte| matches!(byte, b'\r' | b'\n')) {
            self.buffer.clear();
            self.start = 0;
            return Ok(None);
        }

        let frame = String::from_utf8_lossy(remaining).into_owned();
        self.buffer.clear();
        self.start = 0;
        Ok(Some(frame))
    }

    fn compact_consumed(&mut self) {
        if self.start == 0 {
            return;
        }
        if self.start == self.buffer.len() {
            self.buffer.clear();
            self.start = 0;
            return;
        }
        if self.start >= self.buffer.len() / 2 {
            self.buffer.drain(..self.start);
            self.start = 0;
        }
    }
}

fn last_frame_end(buffer: &[u8]) -> Option<usize> {
    let mut offset = 0;
    let mut last_end = None;
    while let Some((relative_end, delimiter_len)) = find_frame_end(&buffer[offset..]) {
        offset += relative_end + delimiter_len;
        last_end = Some(offset);
    }
    last_end
}

fn validate_frame_budget(
    bytes: impl Iterator<Item = u8>,
    max_frame_bytes: u64,
) -> Result<(), SseFrameTooLarge> {
    let mut frame_len = 0_u64;
    let mut previous = [0_u8; 4];
    let mut seen = 0_usize;
    for byte in bytes {
        frame_len = frame_len.checked_add(1).ok_or(SseFrameTooLarge {
            limit: max_frame_bytes,
        })?;
        previous.rotate_left(1);
        previous[3] = byte;
        seen = seen.saturating_add(1);
        let delimiter_len = if seen >= 2 && previous[2..] == *b"\n\n" {
            2
        } else if seen >= 4 && previous == *b"\r\n\r\n" {
            4
        } else {
            0
        };
        if delimiter_len > 0 {
            let payload_len = frame_len.saturating_sub(delimiter_len);
            if payload_len > max_frame_bytes {
                return Err(SseFrameTooLarge {
                    limit: max_frame_bytes,
                });
            }
            frame_len = 0;
            previous = [0; 4];
            seen = 0;
        } else if frame_len > max_frame_bytes.saturating_add(3) {
            return Err(SseFrameTooLarge {
                limit: max_frame_bytes,
            });
        }
    }
    let suffix_len = if seen >= 3 && previous[1..] == *b"\r\n\r" {
        3
    } else if seen >= 2 && previous[2..] == *b"\r\n" {
        2
    } else if seen >= 1 && matches!(previous[3], b'\r' | b'\n') {
        1
    } else {
        0
    };
    if frame_len.saturating_sub(suffix_len) > max_frame_bytes {
        return Err(SseFrameTooLarge {
            limit: max_frame_bytes,
        });
    }
    Ok(())
}

fn find_frame_end(buffer: &[u8]) -> Option<(usize, usize)> {
    buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|pos| (pos, 2))
        .into_iter()
        .chain(
            buffer
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|pos| (pos, 4)),
        )
        .min_by_key(|(pos, _)| *pos)
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SseFrame {
    pub(crate) event: Option<String>,
    pub(crate) data: String,
}

pub(crate) fn parse_sse_frame(text: &str) -> Option<SseFrame> {
    if text.trim().is_empty() {
        return None;
    }

    let mut event = None;
    let mut data_lines = Vec::new();

    for line in text.lines() {
        if line.starts_with(':') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("event:") {
            event = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim().to_string());
        }
    }

    if event.is_none() && data_lines.is_empty() {
        return None;
    }

    Some(SseFrame {
        event,
        data: data_lines.join("\n"),
    })
}

pub(crate) fn parse_sse_json_value(text: &str) -> Option<Value> {
    let frame = parse_sse_frame(text)?;
    let data = frame.data.trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    serde_json::from_str(data).ok()
}

pub(crate) fn is_sse_done(text: &str) -> bool {
    parse_sse_frame(text)
        .map(|frame| frame.data.trim() == "[DONE]")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    const LIMIT: u64 = 1024;

    async fn native_sse_response(
        body: &str,
        extra_declared_bytes: usize,
        hold_open: bool,
    ) -> (reqwest::Response, oneshot::Sender<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let body = body.to_string();
        let (release, released) = oneshot::channel();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let count = socket.read(&mut buffer).await.unwrap();
                if count == 0 {
                    return;
                }
                request.extend_from_slice(&buffer[..count]);
            }
            let headers = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len() + extra_declared_bytes
            );
            socket.write_all(headers.as_bytes()).await.unwrap();
            socket.write_all(body.as_bytes()).await.unwrap();
            if hold_open {
                let _ = released.await;
            }
        });
        let response = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap()
            .get(format!("http://{address}/"))
            .send()
            .await
            .unwrap();
        (response, release)
    }

    #[tokio::test]
    async fn native_anthropic_stream_requires_message_stop_for_text_and_thinking() {
        for kind in ["text", "thinking"] {
            let start = serde_json::json!({"type":"content_block_start", "index":0,
                "content_block":{"type":kind, kind:""}});
            let body = format!(
                "event: message_start\ndata: {{\"type\":\"message_start\",\"message\":{{\"id\":\"msg_partial\",\"content\":[]}}}}\n\nevent: content_block_start\ndata: {start}\n\nevent: content_block_stop\ndata: {{\"type\":\"content_block_stop\",\"index\":0}}\n\n"
            );
            let (response, _release) = native_sse_response(&body, 0, false).await;
            let events = stream_anthropic_response(response, LIMIT)
                .collect::<Vec<_>>()
                .await;
            assert!(events[..3].iter().all(Result::is_ok));
            assert!(
                matches!(events.last(), Some(Err(ProviderError::Network(message)))
                if message.contains("ended before message_stop"))
            );
        }
    }

    #[tokio::test]
    async fn native_anthropic_stream_rejects_empty_success_response() {
        let (response, _release) = native_sse_response("", 0, false).await;
        let events = stream_anthropic_response(response, LIMIT)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], Err(ProviderError::Network(message))
            if message.contains("ended before message_stop")));
    }

    #[tokio::test]
    async fn native_anthropic_stream_recovers_undelimited_stop_at_transport_eof() {
        let body = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_done\",\"content\":[]}}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n",
            "data: {\"type\":\"message_stop\"}"
        );
        let (response, _release) = native_sse_response(body, 10, false).await;
        let events = stream_anthropic_response(response, LIMIT)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(events.len(), 3);
        assert!(events.iter().all(Result::is_ok));
        assert_eq!(
            events.last().unwrap().as_ref().unwrap().data["type"],
            "message_stop"
        );
    }

    #[tokio::test]
    async fn native_anthropic_stream_completes_before_transport_eof() {
        let body = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_done\",\"content\":[]}}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
        );
        let (response, release) = native_sse_response(body, 10, true).await;
        let events = tokio::time::timeout(
            Duration::from_secs(1),
            stream_anthropic_response(response, LIMIT).collect::<Vec<_>>(),
        )
        .await
        .expect("message_stop must complete without waiting for HTTP EOF");
        assert_eq!(events.len(), 3);
        assert!(events.iter().all(Result::is_ok));
        let _ = release.send(());
    }

    #[tokio::test]
    async fn native_anthropic_stream_preserves_terminal_api_error() {
        let body = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"local fixture\"}}\n\n";
        let (response, release) = native_sse_response(body, 10, true).await;
        let events = tokio::time::timeout(
            Duration::from_secs(1),
            stream_anthropic_response(response, LIMIT).collect::<Vec<_>>(),
        )
        .await
        .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].as_ref().unwrap().data["error"]["type"],
            "overloaded_error"
        );
        let _ = release.send(());
    }

    #[test]
    fn decoder_returns_complete_frames_and_keeps_partial_tail() {
        let mut decoder = SseDecoder::new(LIMIT);
        decoder.push(b"data: {\"a\":1}\n\n").unwrap();
        decoder
            .push(b"event: message_delta\ndata: {\"b\":")
            .unwrap();

        assert_eq!(decoder.next_frame().unwrap(), "data: {\"a\":1}");
        assert!(decoder.next_frame().is_none());

        decoder.push(b"2}\n\n").unwrap();
        assert_eq!(
            decoder.next_frame().unwrap(),
            "event: message_delta\ndata: {\"b\":2}"
        );
        assert!(decoder.finish().unwrap().is_none());
    }

    #[test]
    fn decoder_supports_crlf_frame_delimiters() {
        let mut decoder = SseDecoder::new(LIMIT);
        decoder.push(b"data: {\"a\":1}\r\n\r\n").unwrap();

        assert_eq!(decoder.next_frame().unwrap(), "data: {\"a\":1}");
    }

    #[test]
    fn decoder_supports_crlf_delimiter_split_across_chunks() {
        let mut decoder = SseDecoder::new(LIMIT);
        decoder.push(b"data: {\"a\":1}\r\n").unwrap();
        decoder.push(b"\r\n").unwrap();

        assert_eq!(decoder.next_frame().unwrap(), "data: {\"a\":1}");
        assert!(decoder.finish().unwrap().is_none());
    }

    #[test]
    fn decoder_returns_adjacent_frames_with_mixed_delimiters() {
        let mut decoder = SseDecoder::new(LIMIT);
        decoder
            .push(b"data: one\n\nevent: two\r\ndata: three\r\n\r\n\n\n")
            .unwrap();

        assert_eq!(decoder.next_frame().unwrap(), "data: one");
        assert_eq!(decoder.next_frame().unwrap(), "event: two\r\ndata: three");
        assert_eq!(decoder.next_frame().unwrap(), "");
        assert!(decoder.finish().unwrap().is_none());
    }

    #[test]
    fn decoder_finish_returns_partial_tail_after_consumed_frames() {
        let mut decoder = SseDecoder::new(LIMIT);
        decoder.push(b"data: one\n\ndata: partial").unwrap();

        assert_eq!(decoder.next_frame().unwrap(), "data: one");
        assert_eq!(decoder.finish().unwrap().unwrap(), "data: partial");
    }

    #[test]
    fn decoder_rejects_incomplete_frame_before_copying_over_limit() {
        let mut decoder = SseDecoder::new(8);
        decoder.push(b"12345678").unwrap();
        assert_eq!(decoder.push(b"9"), Err(SseFrameTooLarge { limit: 8 }));
        assert_eq!(decoder.finish().unwrap().unwrap(), "12345678");
    }

    #[test]
    fn decoder_finish_does_not_treat_partial_delimiter_as_free_payload() {
        let mut decoder = SseDecoder::new(8);
        decoder.push(b"12345678\r").unwrap();

        assert_eq!(decoder.finish(), Err(SseFrameTooLarge { limit: 8 }));
    }

    #[test]
    fn decoder_resets_budget_after_complete_frame() {
        let mut decoder = SseDecoder::new(8);
        decoder.push(b"12345678\n\n12345678").unwrap();

        assert_eq!(decoder.next_frame().unwrap(), "12345678");
        assert_eq!(decoder.finish().unwrap().unwrap(), "12345678");
    }

    #[test]
    fn decoder_allows_large_chunk_with_multiple_small_frames() {
        let mut decoder = SseDecoder::new(9);
        decoder.push(b"data: 1\n\ndata: 2\n\n").unwrap();
        assert_eq!(decoder.next_frame().unwrap(), "data: 1");
        assert_eq!(decoder.next_frame().unwrap(), "data: 2");
    }

    #[test]
    fn parse_frame_accepts_data_without_space_and_multiline_data() {
        let frame = parse_sse_frame("event:message\ndata:{\"a\":1}\ndata:{\"b\":2}").unwrap();

        assert_eq!(frame.event.as_deref(), Some("message"));
        assert_eq!(frame.data, "{\"a\":1}\n{\"b\":2}");
    }

    #[test]
    fn parse_json_value_ignores_done_sentinel() {
        assert!(parse_sse_json_value("data: [DONE]").is_none());
    }
}
