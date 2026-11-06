use super::*;
use serde_json::json;

#[tokio::test]
async fn default_lane_policy_applies_without_overrides_and_precedes_remote_lane_metadata() {
    let provider = tests::test_chatgpt_provider("http://127.0.0.1:1/responses".into()).await;
    assert!(provider.chatgpt_config.model_capabilities.is_empty());
    let cases = [
        ("gpt-5.6-sol", false),
        ("gpt-6.1-sol", false),
        ("gpt-6-astra", false),
        ("gpt-6-luna", true),
    ];
    for online_catalog in [false, true] {
        if online_catalog {
            let mut catalog = provider.remote_models.write().unwrap();
            catalog.identity = provider.model_cache_identity();
            for (model, lite) in cases {
                let remote = serde_json::from_value(json!({
                    "slug": model, "use_responses_lite": !lite,
                    "supports_parallel_tool_calls": true
                }))
                .unwrap();
                catalog.models.insert(
                    model.into(),
                    chatgpt_catalog_model_from_remote(remote, &provider.chatgpt_config),
                );
            }
        }
        for (model, lite) in cases {
            let decision = provider.responses_lite_decision(model);
            assert_eq!(
                decision.is_enabled(),
                lite,
                "{model}, online={online_catalog}"
            );
            assert_eq!(decision.source_str(), "default_policy");
            assert_eq!(
                provider
                    .responses_lite_decision(&format!("chatgpt/{model}"))
                    .is_enabled(),
                lite
            );
            let request = serde_json::from_value(json!({
                "model": model, "messages": [{"role": "user", "content": "Read the value."}],
                "tools": [{"name": "Read", "input_schema": {"type": "object", "properties": {}}}]
            }))
            .unwrap();
            let body = responses::build_body_with_context(
                &request,
                DEFAULT_CHATGPT_INSTRUCTIONS,
                responses::CodexRequestContext {
                    responses_lite: decision.is_enabled(),
                    supports_parallel_tool_calls: provider.supports_parallel_tool_calls(model),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(body["parallel_tool_calls"], !lite);
            assert_eq!(
                body["reasoning"]["context"].as_str(),
                lite.then_some("all_turns")
            );
            let mut native = json!({"model": model, "input": "hi", "parallel_tool_calls": true});
            normalize_chatgpt_native_responses_body(&mut native, decision.is_enabled());
            assert_eq!(native["parallel_tool_calls"], !lite);
        }
    }
}

#[tokio::test]
async fn default_lane_policy_respects_user_choices_and_leaves_other_models_on_auto() {
    let mut provider = tests::test_chatgpt_provider("http://127.0.0.1:1/responses".into()).await;
    provider.chatgpt_config.model_capabilities.insert(
        "gpt-5.6-sol".into(),
        ChatGptModelCapabilityOverride {
            context_window: Some(872_000),
            ..Default::default()
        },
    );
    assert!(!provider.responses_lite_decision("gpt-5.6-sol").is_enabled());
    for (model, lite) in [("gpt-5.6-sol", true), ("gpt-6-luna", false)] {
        provider.chatgpt_config.model_capabilities.insert(
            model.into(),
            ChatGptModelCapabilityOverride {
                responses_lite: Some(lite),
                ..Default::default()
            },
        );
        assert_eq!(provider.responses_lite_decision(model).is_enabled(), lite);
        provider.chatgpt_config.responses_lite = ResponsesLiteMode::On;
        assert!(provider.responses_lite_decision(model).is_enabled());
        provider.chatgpt_config.responses_lite = ResponsesLiteMode::Off;
        assert!(!provider.responses_lite_decision(model).is_enabled());
        provider.chatgpt_config.responses_lite = ResponsesLiteMode::Auto;
    }
    assert!(provider.responses_lite_decision("gpt-6-sol").is_enabled());
    assert!(
        !provider
            .responses_lite_decision("unknown-model")
            .is_enabled()
    );
    {
        let mut catalog = provider.remote_models.write().unwrap();
        catalog.identity = provider.model_cache_identity();
        for (model, lite) in [("gpt-6-sol", false), ("new-catalog-model", true)] {
            let remote =
                serde_json::from_value(json!({"slug": model, "use_responses_lite": lite})).unwrap();
            catalog.models.insert(
                model.into(),
                chatgpt_catalog_model_from_remote(remote, &provider.chatgpt_config),
            );
        }
    }
    assert!(!provider.responses_lite_decision("gpt-6-sol").is_enabled());
    assert!(
        provider
            .responses_lite_decision("new-catalog-model")
            .is_enabled()
    );

    // Full permits batching but must not override upstream or client restrictions.
    let remote = serde_json::from_value(json!({
        "slug": "gpt-6.1-sol", "use_responses_lite": true, "supports_parallel_tool_calls": false
    }))
    .unwrap();
    provider.remote_models.write().unwrap().models.insert(
        "gpt-6.1-sol".into(),
        chatgpt_catalog_model_from_remote(remote, &provider.chatgpt_config),
    );
    assert!(!provider.responses_lite_decision("gpt-6.1-sol").is_enabled());
    let request = serde_json::from_value(json!({
        "model": "gpt-6.1-sol", "messages": [{"role": "user", "content": "Read a value."}],
        "tools": [{"name": "Read", "input_schema": {"type": "object", "properties": {}}}],
        "parallel_tool_calls": true
    }))
    .unwrap();
    let body = responses::build_body_with_context(
        &request,
        DEFAULT_CHATGPT_INSTRUCTIONS,
        responses::CodexRequestContext {
            responses_lite: provider.responses_lite_decision("gpt-6.1-sol").is_enabled(),
            supports_parallel_tool_calls: provider.supports_parallel_tool_calls("gpt-6.1-sol"),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(body["parallel_tool_calls"], false);
}

#[tokio::test]
async fn new_codex_models_use_default_lanes_and_catalog_tiers() {
    let provider = tests::test_chatgpt_provider("http://127.0.0.1:1/responses".to_string()).await;
    for (model, context_window, expected_lite) in [
        ("gpt-6-sol", 272_000, true),
        ("gpt-6-luna", 272_000, true),
        ("gpt-6.1-sol", 272_000, false),
        ("gpt-6.1-terra", 500_000, true),
    ] {
        let mut request: MessagesRequest = serde_json::from_value(json!({
            "model": model, "messages":[{"role":"user","content":"hi"}],
            "reasoning":{"effort":"ultra"}, "temperature":0.5, "top_p":0.8,
            "tools":[{"name":"lookup","input_schema":{"type":"object","properties":{}}}]
        }))
        .unwrap();
        assert!(normalize_chatgpt_reasoning(&mut request));
        let info = provider.model_info(model).unwrap();
        let lite = provider.responses_lite_decision(model).is_enabled();
        assert_eq!(lite, expected_lite);
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
        if expected_lite {
            assert_eq!(body["input"][0]["type"], "additional_tools");
            assert_eq!(body["input"][1]["role"], "developer");
        } else {
            assert!(!body["tools"].as_array().unwrap().is_empty());
        }
        assert_eq!(body["reasoning"]["effort"], "max");
        assert_eq!(
            body["reasoning"]["context"].as_str(),
            expected_lite.then_some("all_turns")
        );
        assert_eq!(body["parallel_tool_calls"], !expected_lite);
        assert!(body.get("temperature").is_none());
        assert!(body.get("top_p").is_none());
        assert_eq!(
            info.capabilities.limits.context_window,
            Some(context_window)
        );
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

    let error = map_chatgpt_error_status_body_with_headers(
        StatusCode::SERVICE_UNAVAILABLE,
        &headers,
        json!({"error":{"code":"policy_capacity_unavailable","message":"policy route unavailable"}}).to_string(),
    );
    assert!(matches!(
        error.without_upstream_metadata(),
        ProviderError::PolicyCapacityUnavailable {
            retry_after: Some(90),
            ..
        }
    ));

    let error = map_chatgpt_error_status_body(
        StatusCode::FORBIDDEN,
        json!({"error":{"code":"model_not_found","message":"unknown model"}}).to_string(),
    );
    assert!(matches!(
        error.without_upstream_metadata(),
        ProviderError::ModelNotFound(message) if message == "unknown model"
    ));

    let error = map_chatgpt_error_status_body(
        StatusCode::BAD_REQUEST,
        json!({"error":{"code":"invalid_audio_format","message":"Invalid recording."}}).to_string(),
    );
    assert!(matches!(
        error.without_upstream_metadata(),
        ProviderError::InvalidRequest(message) if message == "Invalid recording."
    ));
}
