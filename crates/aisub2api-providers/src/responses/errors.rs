//! Responses failures keep their server retry advice across SSE and WebSocket streams.

use crate::http::{
    is_audio_validation_error_code, is_spend_limit_code, json_headers_to_header_map,
    upstream_error_metadata_from_parts,
};
use crate::provider::ProviderError;
use serde_json::Value;
use std::time::Duration;

pub(super) fn responses_error_event(value: &Value) -> Option<ProviderError> {
    let kind = value.get("type")?.as_str()?;
    if kind == "response.incomplete" {
        let reason = value
            .pointer("/response/incomplete_details/reason")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        // These have established Messages/Chat finish-reason equivalents.
        if matches!(
            reason,
            "max_output_tokens" | "max_tokens" | "content_filter" | "content_policy_violation"
        ) {
            return None;
        }
        return Some(ProviderError::UpstreamError {
            status: 200,
            body: value.to_string(),
        });
    }
    if !matches!(kind, "error" | "response.failed") {
        return None;
    }
    let response = value.get("response").unwrap_or(value);
    let error = response.get("error").unwrap_or(response);
    let code = error
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .filter(|message| !message.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Responses API request failed ({code})"));
    let explicit_status = responses_error_status(value);
    let status = explicit_status.unwrap_or(200);
    let headers = responses_error_headers(value, error);
    let mut metadata =
        upstream_error_metadata_from_parts(status, &headers, &value.to_string(), message.clone());
    if metadata.retry_after.is_none() && matches!(code, "rate_limit_exceeded" | "slow_down") {
        metadata.retry_after = retry_delay_from_message(&message);
    }
    let retry_after = metadata.retry_after;
    let mapped =
        terminal_responses_error(code, explicit_status, &message).unwrap_or_else(|| match code {
            "policy_capacity_unavailable" => ProviderError::PolicyCapacityUnavailable {
                message,
                retry_after,
            },
            "rate_limit_exceeded" | "slow_down" => ProviderError::RateLimited { retry_after },
            "server_error" | "server_is_overloaded" => ProviderError::Overloaded {
                message,
                retry_after,
            },
            _ if status == 429 => ProviderError::RateLimited { retry_after },
            _ if matches!(status, 408 | 409 | 500..=599) => ProviderError::Overloaded {
                message,
                retry_after,
            },
            _ => ProviderError::UpstreamError {
                status,
                body: value.to_string(),
            },
        });
    Some(mapped.with_upstream_metadata(metadata))
}

pub(crate) fn responses_error_status(value: &Value) -> Option<u16> {
    value
        .get("status")
        .and_then(Value::as_u64)
        .or_else(|| value.get("status_code").and_then(Value::as_u64))
        .and_then(|value| u16::try_from(value).ok())
        .filter(|status| (400..=599).contains(status))
}

pub(crate) fn terminal_responses_error(
    code: &str,
    status: Option<u16>,
    message: &str,
) -> Option<ProviderError> {
    let error = match (code, status) {
        ("context_length_exceeded", None | Some(400)) => {
            ProviderError::RequestTooLarge(message.into())
        }
        ("model_not_found", None | Some(400 | 403 | 404)) => {
            ProviderError::ModelNotFound(message.into())
        }
        ("flex_unavailable", None | Some(429)) => ProviderError::FlexUnavailable(message.into()),
        (code, None | Some(400)) if is_audio_validation_error_code(code) => {
            ProviderError::InvalidRequest(message.into())
        }
        (code, None | Some(429)) if is_spend_limit_code(code) => {
            ProviderError::InvalidRequest(message.into())
        }
        (
            "budget_reached"
            | "content_policy_violation"
            | "invalid_access_program"
            | "unsupported_access_program",
            None | Some(400),
        )
        | ("inference_hook_denied" | "access_program_not_enabled", None | Some(403))
        | (
            "invalid_prompt"
            | "bio_policy"
            | "cyber_policy"
            | "misalignment_policy_violation"
            | "insufficient_quota"
            | "credit_balance_exhausted"
            | "usage_not_included",
            None,
        ) => ProviderError::InvalidRequest(message.into()),
        _ => return None,
    };
    Some(error)
}

pub(crate) fn responses_error_headers(event: &Value, error: &Value) -> reqwest::header::HeaderMap {
    let headers = |value: &Value| {
        value
            .get("headers")
            .and_then(Value::as_object)
            .cloned()
            .map(json_headers_to_header_map)
            .unwrap_or_default()
    };
    let mut merged = headers(event);
    merged.extend(headers(error));
    merged
}

fn retry_delay_from_message(message: &str) -> Option<u64> {
    let lower = message.to_ascii_lowercase();
    let (_, tail) = lower.split_once("try again in")?;
    let tail = tail.trim_start();
    let end = tail
        .find(|ch: char| !ch.is_ascii_digit() && ch != '.')
        .unwrap_or(tail.len());
    let value: f64 = tail[..end].parse().ok()?;
    let unit = tail[end..].trim_start();
    let seconds = if unit.starts_with("ms") {
        value / 1000.0
    } else if unit.starts_with('s') {
        value
    } else {
        return None;
    };
    let duration = Duration::try_from_secs_f64(seconds).ok()?;
    Some(
        duration
            .as_secs()
            .saturating_add(u64::from(duration.subsec_nanos() > 0)),
    )
}

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;
