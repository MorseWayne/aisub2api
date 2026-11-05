use std::time::Duration;

use futures::{StreamExt, stream::BoxStream};
use serde_json::Value;
use tokio::time::{Instant, timeout_at};
use tracing::warn;

use super::{
    ChatGptPreparedRequest, ChatGptProvider, ChatGptSseRequestContext, ChatGptSseStreamContext,
    ChatGptToken, UpstreamRetryBudget, map_upstream_response,
};
use crate::provider::{NativeProviderEvent, ProviderError, ProviderEvent};

const STARTUP_WINDOW: Duration = Duration::from_secs(5);
const MAX_STAGED_EVENTS: usize = 64;
const MAX_STAGED_BYTES: u64 = 256 * 1024;

pub(super) fn with_http_retry_advice(
    error: ProviderError,
    headers: &reqwest::header::HeaderMap,
) -> ProviderError {
    let fallback =
        crate::http::upstream_error_metadata_from_parts(200, headers, "", error.to_string());
    let mut metadata = error
        .upstream_metadata()
        .cloned()
        .unwrap_or_else(|| fallback.clone());
    metadata.retry_after = metadata.retry_after.or(fallback.retry_after);
    metadata.request_id = metadata.request_id.or(fallback.request_id);
    for header in fallback.headers {
        if !metadata
            .headers
            .iter()
            .any(|existing| existing.name.eq_ignore_ascii_case(&header.name))
        {
            metadata.headers.push(header);
        }
    }
    let error = match error.without_upstream_metadata().clone() {
        ProviderError::Overloaded {
            message,
            retry_after,
        } => ProviderError::Overloaded {
            message,
            retry_after: retry_after.or(metadata.retry_after),
        },
        ProviderError::RateLimited { retry_after } => ProviderError::RateLimited {
            retry_after: retry_after.or(metadata.retry_after),
        },
        ProviderError::PolicyCapacityUnavailable {
            message,
            retry_after,
        } => ProviderError::PolicyCapacityUnavailable {
            message,
            retry_after: retry_after.or(metadata.retry_after),
        },
        other => other,
    };
    error.with_upstream_metadata(metadata)
}

pub(super) async fn chat_via_sse(
    provider: &ChatGptProvider,
    mut prepared: ChatGptPreparedRequest,
    mut token: ChatGptToken,
) -> Result<BoxStream<'static, Result<ProviderEvent, ProviderError>>, ProviderError> {
    let mut budget = UpstreamRetryBudget::new(provider.request_policy);
    let mut auth_refreshed = false;
    let mut deadline = None;
    loop {
        let response = provider
            .send_responses_request_with_prompt_too_long_retry_and_headers(
                &mut prepared.body,
                &token,
                ChatGptSseRequestContext {
                    compact_request: prepared.compact_request,
                    request_id: prepared.request_id,
                    budget: prepared.output_token_budget,
                    responses_lite: prepared.responses_lite,
                },
                prepared.observer.as_ref(),
                None,
                Some(&mut budget),
            )
            .await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED && !auth_refreshed {
            token = match provider.auth.force_refresh_token().await {
                Ok(token) => token,
                Err(error) => {
                    if error.is_authentication() {
                        provider.auth.clear_token().await;
                    }
                    return Err(error);
                }
            };
            auth_refreshed = true;
            // Preserve the existing one-time credential renewal, independently
            // of transient recovery. Subsequent stream/HTTP retries share this budget.
            budget = UpstreamRetryBudget::new(provider.request_policy);
            continue;
        }
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            provider.auth.clear_token().await;
        }
        if !response.status().is_success() {
            return Err(map_upstream_response(response).await);
        }

        let deadline = *deadline.get_or_insert_with(|| {
            Instant::now()
                + provider
                    .request_policy
                    .attempt_timeout
                    .unwrap_or(STARTUP_WINDOW)
                    .min(STARTUP_WINDOW)
        });
        let stream = provider
            .stream_sse_response(
                response,
                ChatGptSseStreamContext {
                    marker_mode: prepared.marker_mode,
                    request_id: prepared.request_id,
                    compact_request: prepared.compact_request,
                    observer: prepared.observer.clone(),
                    pending_context_usage: prepared.pending_context_usage.clone(),
                    responses_correlation: prepared.responses_correlation.clone(),
                },
            )
            .await;
        match stage_startup(stream, deadline).await {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                let Some(delay) = budget.overload_retry_delay(&error) else {
                    return Err(error);
                };
                if Instant::now() + delay >= deadline {
                    return Err(error);
                }
                warn!(
                    request_id = prepared.request_id,
                    delay_ms = delay.as_millis(),
                    "Retrying ChatGPT SSE overload before output"
                );
                tokio::time::sleep(delay).await;
            }
        }
    }
}

async fn stage_startup(
    mut stream: BoxStream<'static, Result<ProviderEvent, ProviderError>>,
    deadline: Instant,
) -> Result<BoxStream<'static, Result<ProviderEvent, ProviderError>>, ProviderError> {
    let mut staged = Vec::new();
    let mut bytes = 0_u64;
    loop {
        let event = match timeout_at(deadline, stream.next()).await {
            Ok(Some(event)) => event?,
            Ok(None) | Err(_) => break,
        };
        let commit = !is_initialization_event(&event);
        bytes = bytes.saturating_add(event.retained_size_bytes().unwrap_or(u64::MAX));
        staged.push(Ok(event));
        if commit || bytes >= MAX_STAGED_BYTES || staged.len() >= MAX_STAGED_EVENTS {
            break;
        }
    }
    Ok(futures::stream::iter(staged).chain(stream).boxed())
}

/// Only known, empty lifecycle items may be discarded on a failed attempt.
/// Tools, encrypted reasoning, successful terminals and unknown events commit
/// the stream even when their normalized output happens to be empty.
fn is_initialization_event(event: &ProviderEvent) -> bool {
    let Some(NativeProviderEvent::OpenAiResponses(event)) = event.native_event() else {
        return false;
    };
    let data = &event.data;
    match data["type"].as_str() {
        Some("response.created" | "response.in_progress") => {
            data.get("response").is_some_and(|response| {
                response.is_object()
                    && response
                        .get("output")
                        .is_none_or(|output| output.as_array().is_some_and(Vec::is_empty))
            })
        }
        Some("response.output_item.added") => {
            let item = &data["item"];
            matches!(item["type"].as_str(), Some("message" | "reasoning"))
                && item.get("encrypted_content").is_none_or(Value::is_null)
                && item
                    .get("text")
                    .is_none_or(|text| text.as_str() == Some(""))
                && item.get("signature").is_none_or(Value::is_null)
                && ["content", "summary"].iter().all(|key| {
                    item.get(key).is_none_or(|value| {
                        value
                            .as_array()
                            .is_some_and(|parts| parts.iter().all(is_empty_part))
                    })
                })
        }
        Some("response.content_part.added" | "response.reasoning_summary_part.added") => {
            is_empty_part(&data["part"])
        }
        Some(
            "response.output_text.delta"
            | "response.reasoning_text.delta"
            | "response.reasoning_summary_text.delta",
        ) => data["delta"].as_str() == Some(""),
        Some(
            "response.output_text.done"
            | "response.reasoning_text.done"
            | "response.reasoning_summary_text.done",
        ) => data["text"].as_str() == Some(""),
        _ => false,
    }
}

fn is_empty_part(part: &Value) -> bool {
    matches!(
        part["type"].as_str(),
        Some("output_text" | "summary_text" | "reasoning_text")
    ) && part["text"].as_str() == Some("")
}

#[cfg(test)]
#[path = "sse_recovery_tests.rs"]
mod tests;
