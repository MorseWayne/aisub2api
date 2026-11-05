pub(super) use crate::sse::stream_anthropic_response as stream_anthropic_sse_response;

#[cfg(test)]
use crate::sse::parse_anthropic_sse_frame as parse_anthropic_sse_text;
#[cfg(test)]
use aisub2api_core::SseEvent;
#[cfg(test)]
use serde_json::Value;

#[cfg(test)]
fn parse_anthropic_sse(bytes: &[u8]) -> SseEvent {
    let text = String::from_utf8_lossy(bytes);
    parse_anthropic_sse_text(&text).unwrap_or_else(|| SseEvent {
        event: String::new(),
        data: Value::Null,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_anthropic_sse_accepts_data_without_space() {
        let event = parse_anthropic_sse(
            br#"event: message_delta
data:{"type":"message_delta"}"#,
        );

        assert_eq!(event.event, "message_delta");
        assert_eq!(event.data["type"], "message_delta");
    }

    #[test]
    fn parse_anthropic_sse_text_ignores_incomplete_empty_frame() {
        assert!(parse_anthropic_sse_text("").is_none());
        assert!(parse_anthropic_sse_text("\n").is_none());
    }

    #[test]
    fn parse_anthropic_sse_text_reads_single_frame_from_buffered_stream() {
        let event = parse_anthropic_sse_text(
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\"}",
        )
        .expect("event");

        assert_eq!(event.event, "content_block_delta");
        assert_eq!(event.data["type"], "content_block_delta");
    }
}
