use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use super::*;
use crate::chatgpt::{
    ChatGptOutputTokenBudget, ResponsesLiteDecision, ResponsesLiteDecisionSource,
};

#[tokio::test]
async fn reviewed_multi_model_config_preserves_lane_and_parallel_contracts() {
    let settings = aisub2api_config::Settings::from_toml(
        include_str!("../../../../docs/reviews/2026-11-06-claude-code-multi-model.toml"),
        std::path::Path::new("2026-11-06-claude-code-multi-model.toml"),
    )
    .unwrap();
    let mut provider =
        crate::chatgpt::tests::test_chatgpt_provider("http://127.0.0.1/responses".into()).await;
    provider.chatgpt_config = settings.providers["chatgpt"].chatgpt.clone().unwrap();

    for (model, lite) in [
        ("gpt-5.6-sol", false),
        ("gpt-6.1-sol", false),
        ("gpt-6-astra", false),
        ("gpt-6-luna", true),
    ] {
        assert_eq!(provider.responses_lite_decision(model).is_enabled(), lite);
        assert_eq!(
            provider
                .responses_lite_decision(&format!("chatgpt/{model}"))
                .is_enabled(),
            lite
        );
        let mut request: aisub2api_core::MessagesRequest = serde_json::from_value(json!({
            "model": model, "max_tokens": 512,
            "messages": [{"role": "user", "content": "Read both independent values."}],
            "tools": [{"name": "Read", "input_schema": {"type": "object", "properties": {}}}]
        }))
        .unwrap();
        let context = crate::chatgpt::responses::CodexRequestContext {
            responses_lite: lite,
            supports_parallel_tool_calls: provider.supports_parallel_tool_calls(model),
            ..Default::default()
        };
        let body =
            crate::chatgpt::build_chatgpt_responses_body_with_codex_context(&request, context);
        assert_eq!(body["parallel_tool_calls"], !lite, "{model}");
        assert_eq!(
            body["reasoning"]["context"].as_str(),
            lite.then_some("all_turns")
        );

        request
            .extra
            .insert("parallel_tool_calls".into(), json!(false));
        let restricted =
            crate::chatgpt::build_chatgpt_responses_body_with_codex_context(&request, context);
        assert_eq!(restricted["parallel_tool_calls"], false);

        // The existing option is provider/model scoped, including native Responses.
        let mut native = json!({"model": model, "input": "hi", "parallel_tool_calls": true});
        crate::chatgpt::normalize_chatgpt_native_responses_body(&mut native, lite);
        assert_eq!(native["parallel_tool_calls"], !lite);
    }
    // Unlisted models keep Auto policy rather than inheriting a blanket Full override.
    assert!(provider.responses_lite_decision("gpt-6-sol").is_enabled());
}

struct Reply {
    status: u16,
    body: String,
    headers: &'static str,
}

fn sse(events: Vec<Value>) -> Reply {
    Reply {
        status: 200,
        body: events
            .iter()
            .map(|event| format!("data: {event}\n\n"))
            .collect(),
        headers: "",
    }
}

fn created(id: &str) -> Value {
    json!({"type":"response.created","response":{"id":id,"model":"gpt-6-astra"}})
}

fn overload(kind: &str) -> Value {
    let error = json!({"type":"service_unavailable_error","code":"server_is_overloaded","message":"temporarily overloaded"});
    if kind == "error" {
        json!({"type":kind,"error":error})
    } else {
        json!({"type":kind,"response":{"status":"failed","error":error}})
    }
}

fn success() -> Reply {
    let mut events = vec![created("resp-winner")];
    let mut output = Vec::new();
    for (index, name) in ["probe_alpha", "probe_beta"].into_iter().enumerate() {
        let item = json!({"type":"function_call","id":format!("fc_{index}"),"call_id":format!("call_{index}"),"name":name,"arguments":"{}","status":"completed"});
        let mut added = item.clone();
        added["arguments"] = json!("");
        added["status"] = json!("in_progress");
        events.extend([
            json!({"type":"response.output_item.added","output_index":index,"item":added}),
            json!({"type":"response.function_call_arguments.delta","output_index":index,"delta":"{}"}),
            json!({"type":"response.output_item.done","output_index":index,"item":item}),
        ]);
        output.push(item);
    }
    events.push(json!({"type":"response.completed","response":{"id":"resp-winner","status":"completed","model":"gpt-6-astra","output":output,"usage":{"input_tokens":100,"output_tokens":20}}}));
    sse(events)
}

struct Fixture {
    endpoint: String,
    requests: Arc<Mutex<Vec<Value>>>,
    websocket_upgrades: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn fixture(replies: Vec<Reply>) -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/responses", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let websocket_upgrades = Arc::new(AtomicUsize::new(0));
    let captured_upgrades = Arc::clone(&websocket_upgrades);
    let task = tokio::spawn(async move {
        let mut replies = VecDeque::from(replies);
        'connections: loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut raw = Vec::new();
            let mut chunk = [0_u8; 4096];
            loop {
                let count = socket.read(&mut chunk).await.unwrap();
                if count == 0 {
                    break;
                }
                raw.extend_from_slice(&chunk[..count]);
                let Some(end) = raw.windows(4).position(|v| v == b"\r\n\r\n") else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&raw[..end]).to_ascii_lowercase();
                if headers.starts_with("get ") && headers.contains("upgrade: websocket") {
                    captured_upgrades.fetch_add(1, Ordering::Relaxed);
                    let body = "fixture websocket unavailable";
                    let reply = format!(
                        "HTTP/1.1 503 Service Unavailable\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    socket.write_all(reply.as_bytes()).await.unwrap();
                    continue 'connections;
                }
                let length: usize = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap();
                if raw.len() < end + 4 + length {
                    continue;
                }
                let body = &raw[end + 4..end + 4 + length];
                let decoded = if headers.contains("content-encoding: zstd") {
                    zstd::stream::decode_all(body).unwrap()
                } else {
                    body.to_vec()
                };
                captured
                    .lock()
                    .await
                    .push(serde_json::from_slice(&decoded).unwrap());
                break;
            }
            let reply = replies
                .pop_front()
                .unwrap_or_else(|| sse(vec![overload("error")]));
            let headers = format!(
                "HTTP/1.1 {} OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n",
                reply.status,
                reply.body.len(),
                reply.headers
            );
            let _ = socket.write_all(headers.as_bytes()).await;
            let _ = socket.write_all(reply.body.as_bytes()).await;
        }
    });
    Fixture {
        endpoint,
        requests,
        websocket_upgrades,
        task,
    }
}

async fn provider(fixture: &Fixture, attempts: usize) -> ChatGptProvider {
    let mut provider = crate::chatgpt::tests::test_chatgpt_provider(fixture.endpoint.clone()).await;
    provider.request_policy.max_attempts = attempts;
    provider.request_policy.base_retry_delay = Duration::ZERO;
    provider
}

fn prepared() -> ChatGptPreparedRequest {
    ChatGptPreparedRequest {
        body: json!({"model":"gpt-6-astra","stream":true,"reasoning":{"effort":"low"},"parallel_tool_calls":true,"tools":[],"input":[]}),
        responses_correlation: Default::default(),
        marker_mode: aisub2api_config::settings::ReasoningMarkerMode::Strict,
        compact_request: false,
        request_id: 1,
        output_token_budget: ChatGptOutputTokenBudget::default(),
        stable_client_conversation_id: None,
        responses_lite: ResponsesLiteDecision::disabled(ResponsesLiteDecisionSource::ForcedOff),
        observer: None,
        pending_context_usage: None,
    }
}

fn token() -> ChatGptToken {
    ChatGptToken {
        access_token: "fixture".into(),
        refresh_token: String::new(),
        expires_at: i64::MAX,
        account_id: None,
    }
}

async fn collect(provider: &ChatGptProvider) -> Result<Vec<ProviderEvent>, ProviderError> {
    let mut stream = tokio::time::timeout(
        Duration::from_secs(3),
        chat_via_sse(provider, prepared(), token()),
    )
    .await
    .unwrap()?;
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event?);
    }
    Ok(events)
}

#[tokio::test]
async fn recovers_both_overload_envelopes_without_leaking_losing_attempts() {
    for kind in ["error", "response.failed"] {
        let fixture = fixture(vec![sse(vec![
            created("resp-loser"),
            json!({"type":"response.output_item.added","item":{"type":"reasoning","summary":[]}}),
            json!({"type":"response.reasoning_summary_part.added","part":{"type":"summary_text","text":""}}),
            overload(kind),
        ]), success()]).await;
        let events = collect(&provider(&fixture, 2).await).await.unwrap();
        let normalized: Vec<_> = events
            .iter()
            .flat_map(ProviderEvent::normalized_events)
            .collect();
        assert_eq!(
            normalized
                .iter()
                .filter(|e| e.event == "message_start")
                .count(),
            1
        );
        assert_eq!(
            normalized
                .iter()
                .filter(|e| e.event == "message_stop")
                .count(),
            1
        );
        let names: Vec<_> = normalized
            .iter()
            .filter(|e| {
                e.event == "content_block_start" && e.data["content_block"]["type"] == "tool_use"
            })
            .map(|e| e.data["content_block"]["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["probe_alpha", "probe_beta"]);
        assert!(events.iter().all(|event| match event.native_event() {
            Some(NativeProviderEvent::OpenAiResponses(event)) =>
                !event.data.to_string().contains("resp-loser"),
            None => true,
        }));
        let requests = fixture.requests.lock().await;
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0], requests[1],
            "recovery must preserve the request body"
        );
    }
}

#[tokio::test]
async fn auto_websocket_fallback_recovers_sse_overload_and_keeps_sse_selected() {
    for kind in ["error", "response.failed"] {
        let fixture = fixture(vec![
            sse(vec![created("resp-loser"), overload(kind)]),
            success(),
            success(),
        ])
        .await;
        let mut provider = provider(&fixture, 2).await;
        provider.transport = aisub2api_config::settings::ChatGptTransport::Auto;

        for expected_sse_requests in [2, 3] {
            let events = tokio::time::timeout(Duration::from_secs(3), async {
                provider
                    .chat_prepared_with_token(prepared(), token())
                    .await
                    .unwrap()
                    .collect::<Vec<_>>()
                    .await
            })
            .await
            .unwrap();
            assert!(events.iter().all(Result::is_ok));
            let normalized: Vec<_> = events
                .iter()
                .flat_map(|event| event.as_ref().unwrap().normalized_events())
                .collect();
            for terminal in ["message_start", "message_stop"] {
                assert_eq!(
                    normalized
                        .iter()
                        .filter(|event| event.event == terminal)
                        .count(),
                    1
                );
            }
            assert!(
                events
                    .iter()
                    .all(|event| match event.as_ref().unwrap().native_event() {
                        Some(NativeProviderEvent::OpenAiResponses(event)) =>
                            !event.data.to_string().contains("resp-loser"),
                        None => true,
                    })
            );
            assert_eq!(fixture.requests.lock().await.len(), expected_sse_requests);
            assert_eq!(fixture.websocket_upgrades.load(Ordering::Relaxed), 1);
            assert_eq!(
                provider.effective_transport(),
                aisub2api_config::settings::ChatGptTransport::Sse
            );
        }
        let requests = fixture.requests.lock().await;
        assert_eq!(requests[0], requests[1]);
        assert_eq!(provider.websocket_stats.snapshot().fallbacks, 1);
    }
}

#[tokio::test]
async fn http_and_stream_overloads_share_the_attempt_budget() {
    for attempts in [2, 3] {
        let fixture = fixture(vec![
            Reply {
                status: 503,
                body: "overloaded".into(),
                headers: "",
            },
            sse(vec![created("resp-loser"), overload("error")]),
            success(),
        ])
        .await;
        let result = collect(&provider(&fixture, attempts).await).await;
        assert_eq!(fixture.requests.lock().await.len(), attempts);
        if attempts == 2 {
            assert!(matches!(
                result.unwrap_err().without_upstream_metadata(),
                ProviderError::Overloaded { .. }
            ));
        } else {
            assert!(result.is_ok());
        }
    }
}

#[tokio::test]
async fn never_replays_after_text_thinking_tool_or_unknown_output() {
    for event in [
        json!({"type":"response.output_text.delta","delta":"partial"}),
        json!({"type":"response.reasoning_text.delta","delta":"thinking"}),
        json!({"type":"response.created","response":{"id":"resp-snapshot","output":[{"type":"message","content":[{"type":"output_text","text":"already generated"}]}]}}),
        json!({"type":"response.output_item.added","item":{"type":"reasoning","summary":[],"encrypted_content":"opaque-state"}}),
        json!({"type":"response.output_item.added","output_index":0,"item":{"type":"function_call","id":"fc-first","call_id":"call-first","name":"probe_alpha","arguments":""}}),
        json!({"type":"response.future_event","opaque":"state"}),
    ] {
        let fixture = fixture(vec![
            sse(vec![created("resp-first"), event, overload("error")]),
            success(),
        ])
        .await;
        assert!(collect(&provider(&fixture, 3).await).await.is_err());
        assert_eq!(fixture.requests.lock().await.len(), 1);
    }
}

#[tokio::test]
async fn does_not_retry_policy_errors_or_disabled_transient_retries() {
    let fixture = fixture(vec![sse(vec![created("resp-first"), json!({"type":"response.failed","response":{"error":{"code":"cyber_policy","message":"policy rejection"}}})]), success()]).await;
    assert!(matches!(
        collect(&provider(&fixture, 3).await)
            .await
            .unwrap_err()
            .without_upstream_metadata(),
        ProviderError::InvalidRequest(_)
    ));
    assert_eq!(fixture.requests.lock().await.len(), 1);

    let fixture = self::fixture(vec![
        sse(vec![created("resp-first"), overload("error")]),
        success(),
    ])
    .await;
    let mut provider = provider(&fixture, 3).await;
    provider.request_policy.retry_transient_statuses = false;
    assert!(collect(&provider).await.is_err());
    assert_eq!(fixture.requests.lock().await.len(), 1);
}

#[tokio::test]
async fn preserves_long_retry_after_without_retrying_early() {
    let mut rejection = sse(vec![created("resp-first"), overload("error")]);
    rejection.headers = "Retry-After: 9\r\n";
    let fixture = fixture(vec![rejection, success()]).await;
    let error = collect(&provider(&fixture, 3).await).await.unwrap_err();
    assert_eq!(fixture.requests.lock().await.len(), 1);
    assert_eq!(
        error.upstream_metadata().and_then(|m| m.retry_after),
        Some(9)
    );
}

#[tokio::test]
async fn sse_error_request_id_falls_back_to_http_and_preserves_event_identity() {
    for event_request_id in [None, Some("req-sse-event")] {
        let mut event = overload("error");
        if let Some(request_id) = event_request_id {
            event["error"]["headers"] = json!({"x-oai-request-id": request_id});
        }
        let mut rejection = sse(vec![created("resp-first"), event]);
        rejection.headers = "Retry-After: 9\r\nx-oai-request-id: req-http-envelope\r\n";
        let fixture = fixture(vec![rejection]).await;
        let error = collect(&provider(&fixture, 2).await).await.unwrap_err();
        assert_eq!(fixture.requests.lock().await.len(), 1);
        assert_eq!(
            error.upstream_metadata().unwrap().request_id.as_deref(),
            Some(event_request_id.unwrap_or("req-http-envelope"))
        );
    }
}

#[tokio::test]
async fn stream_retry_advice_takes_precedence_over_http_headers() {
    let mut error = overload("error");
    error["error"]["headers"] = json!({"retry-after":"0"});
    let mut rejection = sse(vec![created("resp-first"), error]);
    rejection.headers = "Retry-After: 9\r\n";
    let fixture = fixture(vec![rejection, success()]).await;
    assert!(collect(&provider(&fixture, 2).await).await.is_ok());
    assert_eq!(fixture.requests.lock().await.len(), 2);
}

#[tokio::test]
async fn bounded_staging_commits_instead_of_discarding_events() {
    let mut many = vec![created("resp-first"); MAX_STAGED_EVENTS];
    many.push(overload("error"));
    let mut large = created("resp-first");
    large["response"]["padding"] = json!("x".repeat(MAX_STAGED_BYTES as usize));
    for events in [many, vec![large, overload("error")]] {
        let fixture = fixture(vec![sse(events), success()]).await;
        assert!(collect(&provider(&fixture, 3).await).await.is_err());
        assert_eq!(fixture.requests.lock().await.len(), 1);
    }
}

#[tokio::test]
async fn staging_deadline_releases_initialization_without_waiting_for_output() {
    let initial = ProviderEvent::openai_responses(
        aisub2api_core::SseEvent {
            event: "response.created".into(),
            data: created("resp-first"),
        },
        vec![],
    );
    let stream = futures::stream::iter(vec![Ok(initial)])
        .chain(futures::stream::pending())
        .boxed();
    let mut stream = tokio::time::timeout(
        Duration::from_secs(1),
        stage_startup(stream, Instant::now() + Duration::from_millis(10)),
    )
    .await
    .unwrap()
    .unwrap();
    let event = stream.next().await.unwrap().unwrap();
    assert!(
        matches!(event.native_event(), Some(NativeProviderEvent::OpenAiResponses(event)) if event.data["response"]["id"] == "resp-first")
    );
}

#[tokio::test]
async fn cancellation_during_recovery_stops_further_requests() {
    let fixture = fixture(vec![
        sse(vec![created("resp-first"), overload("error")]),
        success(),
    ])
    .await;
    let mut provider = provider(&fixture, 2).await;
    provider.request_policy.base_retry_delay = Duration::from_millis(200);
    let task = tokio::spawn(async move { collect(&provider).await });
    tokio::time::timeout(Duration::from_secs(1), async {
        while fixture.requests.lock().await.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::sleep(Duration::from_millis(220)).await;
    assert_eq!(fixture.requests.lock().await.len(), 1);
}
