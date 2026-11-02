use super::*;
use serde_json::json;

#[tokio::test]
async fn new_codex_models_use_lite_requests_and_catalog_tiers() {
    let provider = tests::test_chatgpt_provider("http://127.0.0.1:1/responses".to_string()).await;
    for model in ["gpt-6-sol", "gpt-6-luna", "gpt-6.1-sol"] {
        let mut request: MessagesRequest = serde_json::from_value(json!({
            "model": model, "messages":[{"role":"user","content":"hi"}],
            "reasoning":{"effort":"ultra"}, "temperature":0.5, "top_p":0.8,
            "tools":[{"name":"lookup","input_schema":{"type":"object","properties":{}}}]
        }))
        .unwrap();
        assert!(normalize_chatgpt_reasoning(&mut request));
        let info = provider.model_info(model).unwrap();
        let lite = provider.responses_lite_decision(model).is_enabled();
        let tier = provider.codex_service_tier(model);
        let body = responses::build_body_with_context(
            &request,
            DEFAULT_CHATGPT_INSTRUCTIONS,
            responses::CodexRequestContext {
                responses_lite: lite,
                model: Some(&info),
                service_tier: tier,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(body["input"][0]["type"], "additional_tools");
        assert_eq!(body["input"][1]["role"], "developer");
        assert_eq!(body["reasoning"]["effort"], "max");
        assert_eq!(body["reasoning"]["context"], "all_turns");
        assert_eq!(body["parallel_tool_calls"], false);
        assert!(body.get("temperature").is_none());
        assert!(body.get("top_p").is_none());
        assert_eq!(info.capabilities.limits.context_window, Some(272_000));
        assert!(body.get("service_tier").is_none());
    }
}

#[tokio::test]
async fn catalog_defaults_are_not_implicit_tiers_and_explicit_flex_is_preserved() {
    let mut provider =
        tests::test_chatgpt_provider("http://127.0.0.1:1/responses".to_string()).await;
    let remote: ChatGptRemoteModel = serde_json::from_value(json!({
        "slug":"catalog-model", "service_tiers":[{"id":"priority"}], "default_service_tier":"priority"
    })).unwrap();
    {
        let mut catalog = provider.remote_models.write().unwrap();
        catalog.identity = provider.model_cache_identity();
        catalog.models.insert(
            "catalog-model".to_string(),
            chatgpt_catalog_model_from_remote(remote, &provider.chatgpt_config),
        );
    }
    assert_eq!(provider.codex_service_tier("catalog-model"), None);
    provider.runtime.openai.service_tier = Some("flex".to_string());
    assert_eq!(provider.codex_service_tier("catalog-model"), Some("flex"));
    assert_eq!(provider.codex_service_tier("gpt-6-sol"), Some("flex"));
}

#[test]
fn native_normalization_preserves_new_codex_items_and_numeric_reasoning() {
    let mut body = json!({
        "model":"gpt-6.1-sol", "reasoning":{"effort":4096},
        "input":[
            {"type":"additional_tools","id":"at-1","role":"developer","tools":[{"type":"namespace","name":"extra","tools":[]}]},
            {"type":"message","role":"user","content":[{"type":"input_image","file_id":"file-1"}]},
            {"type":"message","role":"assistant","phase":"partial_answer","encrypted_content":"opaque","status":"completed","content":[{"type":"output_text","text":"partial","annotations":[],"logprobs":[]}]},
            {"type":"configuration_update","reasoning":{"effort":8192}}
        ],
        "include":["output.encrypted_content"],
        "client_metadata":{"x-codex-turn-metadata":"{\"history_initialized\":true}","future":"keep"},
        "access_programs":{"cyber":["example"]}
    });
    let original = body.clone();
    normalize_chatgpt_native_reasoning(&mut body, "gpt-6.1-sol");
    normalize_chatgpt_native_responses_body(&mut body, true);
    for key in ["input", "include", "client_metadata", "access_programs"] {
        assert_eq!(body[key], original[key]);
    }
    assert_eq!(
        body["reasoning"],
        json!({"effort":4096,"context":"all_turns"})
    );
}

#[test]
fn chatgpt_http_capacity_and_throttling_errors_keep_their_meaning() {
    let error = map_chatgpt_error_status_body(
        StatusCode::TOO_MANY_REQUESTS,
        json!({"error":{"code":"flex_unavailable","message":"no capacity"}}).to_string(),
    );
    assert!(matches!(
        error.without_upstream_metadata(),
        ProviderError::FlexUnavailable(_)
    ));
    let headers = HeaderMap::from_iter([("retry-after".parse().unwrap(), "90".parse().unwrap())]);
    let error = map_chatgpt_error_status_body_with_headers(
        StatusCode::SERVICE_UNAVAILABLE,
        &headers,
        json!({"error":{"code":"slow_down","message":"try later"}}).to_string(),
    );
    assert!(matches!(
        error.without_upstream_metadata(),
        ProviderError::RateLimited {
            retry_after: Some(90)
        }
    ));
}
