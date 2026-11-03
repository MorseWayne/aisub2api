use super::*;
use serde_json::json;

#[test]
fn failed_events_preserve_retry_advice_and_filter_headers() {
    for (code, expected) in [
        ("rate_limit_exceeded", "rate"),
        ("slow_down", "rate"),
        ("server_error", "overload"),
    ] {
        let event = json!({"type":"response.failed","response":{"error":{
            "code":code,"message":"Try again in 1s",
            "headers":{"Retry-After":90,"x-request-id":"req-test","set-cookie":"secret", "invalid\nname": "value"}
        }}});
        let error = responses_error_event(&event).unwrap();
        match (expected, error.without_upstream_metadata()) {
            (
                "rate",
                ProviderError::RateLimited {
                    retry_after: Some(90),
                },
            )
            | (
                "overload",
                ProviderError::Overloaded {
                    retry_after: Some(90),
                    ..
                },
            ) => {}
            _ => panic!("incorrect error: {error:?}"),
        }
        let metadata = error.upstream_metadata().unwrap();
        assert_eq!(metadata.retry_after, Some(90));
        assert_eq!(metadata.request_id.as_deref(), Some("req-test"));
        assert!(
            !metadata
                .headers
                .iter()
                .any(|header| header.name == "set-cookie")
        );
    }
}

#[test]
fn retry_message_fallback_only_applies_to_throttling() {
    for (message, seconds) in [
        ("Try again in 1500ms", Some(2)),
        ("TRY AGAIN IN 1.25 seconds", Some(2)),
        ("try again in nonsense", None),
    ] {
        let error = responses_error_event(&json!({"type":"response.failed","response":{"error":{
            "code":"slow_down", "message":message
        }}}))
        .unwrap();
        assert_eq!(error.upstream_metadata().unwrap().retry_after, seconds);
    }
    let error = responses_error_event(&json!({"type":"response.failed","response":{"error":{
        "code":"server_error", "message":"Try again in 30s"
    }}}))
    .unwrap();
    assert_eq!(error.upstream_metadata().unwrap().retry_after, None);
}

#[test]
fn quota_and_policy_errors_are_terminal_and_flex_retains_its_classification() {
    for code in [
        "credit_balance_exhausted",
        "organization_spend_limit_exceeded",
        "project_spend_limit_exceeded",
        "misalignment_policy_violation",
    ] {
        let error = responses_error_event(
            &json!({"type":"response.failed","response":{"error":{"code":code}}}),
        )
        .unwrap();
        assert!(matches!(
            error.without_upstream_metadata(),
            ProviderError::InvalidRequest(_)
        ));
    }
    for event in [
        json!({"type":"error","status":429,"error":{"code":"flex_unavailable","message":"no capacity"}}),
        json!({"type":"response.failed","response":{"error":{"code":"flex_unavailable","message":"no capacity"}}}),
    ] {
        let error = responses_error_event(&event).unwrap();
        assert!(matches!(
            error.without_upstream_metadata(),
            ProviderError::FlexUnavailable(_)
        ));
    }
}

#[test]
fn codex_terminal_provider_codes_preserve_their_semantics() {
    for code in [
        "audio_too_large",
        "audio_too_short",
        "invalid_audio",
        "invalid_audio_data",
        "invalid_audio_format",
        "missing_audio_sample_rate",
    ] {
        let error = responses_error_event(&json!({
            "type":"response.failed",
            "response":{"error":{"code":code,"message":"Invalid recording."}}
        }))
        .unwrap();
        assert!(matches!(
            error.without_upstream_metadata(),
            ProviderError::InvalidRequest(message) if message == "Invalid recording."
        ));
    }

    let error = responses_error_event(&json!({
        "type":"error",
        "error":{"code":"model_not_found","message":"Unknown model."}
    }))
    .unwrap();
    assert!(matches!(
        error.without_upstream_metadata(),
        ProviderError::ModelNotFound(message) if message == "Unknown model."
    ));

    let error = responses_error_event(&json!({
        "type":"response.failed",
        "response":{"error":{
            "code":"policy_capacity_unavailable",
            "message":"Policy route unavailable.",
            "headers":{"Retry-After": 45}
        }}
    }))
    .unwrap();
    assert!(matches!(
        error.without_upstream_metadata(),
        ProviderError::PolicyCapacityUnavailable {
            message,
            retry_after: Some(45)
        } if message == "Policy route unavailable."
    ));
}

#[test]
fn unknown_incomplete_reasons_are_not_successful_translated_responses() {
    for reason in ["budget_exhausted", "interrupted", "unknown"] {
        let event = json!({"type":"response.incomplete","response":{"id":"resp-test","incomplete_details":{"reason":reason},"usage":{"output_tokens":17}}});
        let error = responses_error_event(&event).unwrap();
        match error {
            ProviderError::UpstreamError { body, .. } => {
                assert_eq!(serde_json::from_str::<Value>(&body).unwrap(), event)
            }
            _ => panic!("incorrect incomplete error"),
        }
    }
    for reason in ["max_output_tokens", "max_tokens", "content_filter"] {
        assert!(responses_error_event(&json!({"type":"response.incomplete","response":{"incomplete_details":{"reason":reason}}})).is_none());
    }
}
