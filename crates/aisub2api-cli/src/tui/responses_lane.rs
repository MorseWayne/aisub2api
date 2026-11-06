use aisub2api_config::settings::{
    ChatGptModelCapabilityOverride, ChatGptProviderConfig, ProviderConfig, ProviderType,
    ResponsesLiteMode,
};
use aisub2api_providers::chatgpt::configured_responses_lite;

use super::app::{App, Focus, Overlay, PickerAction, PickerOverlay, Toast};

fn model_config<'a>(
    app: &'a App,
    model_ref: &'a str,
) -> Option<(&'a str, &'a str, &'a ProviderConfig)> {
    let (provider_id, model_id) = model_ref.split_once('/')?;
    let provider = app.settings.providers.get(provider_id)?;
    (provider.resolve_type(provider_id) == ProviderType::ChatGPT && !model_id.is_empty())
        .then_some((provider_id, model_id, provider))
}

fn lane_name(lite: bool) -> &'static str {
    if lite { "Lite" } else { "Full" }
}

pub(super) fn provider_label(mode: ResponsesLiteMode) -> &'static str {
    match mode {
        ResponsesLiteMode::Auto => "Auto (per-model policy)",
        ResponsesLiteMode::Off => "Full (all models)",
        ResponsesLiteMode::On => "Lite (all models)",
    }
}

pub(super) fn model_label(app: &App, model_ref: &str) -> String {
    let Some((_, model_id, provider)) = model_config(app, model_ref) else {
        return "—".into();
    };
    let defaults = ChatGptProviderConfig::default();
    let config = provider.chatgpt.as_ref().unwrap_or(&defaults);
    let Some(lite) = configured_responses_lite(config, model_id) else {
        return "Auto (catalog)".into();
    };
    let source = if config.responses_lite != ResponsesLiteMode::Auto {
        "provider"
    } else if config
        .model_capabilities
        .get(model_id)
        .and_then(|capability| capability.responses_lite)
        .is_some()
    {
        "override"
    } else {
        "default"
    };
    format!("{} ({source})", lane_name(lite))
}

pub(super) fn model_hint(app: &App, model_ref: &str) -> String {
    let Some((provider_id, model_id, provider)) = model_config(app, model_ref) else {
        return "Responses lane is available for ChatGPT models.".into();
    };
    let mode = provider
        .chatgpt
        .as_ref()
        .map(|c| c.responses_lite)
        .unwrap_or_default();
    if mode != ResponsesLiteMode::Auto {
        return format!(
            "{provider_id} forces {}. Set Providers > Responses Lane to Auto to use model overrides.",
            if mode == ResponsesLiteMode::On {
                "Lite"
            } else {
                "Full"
            }
        );
    }
    format!(
        "{provider_id}/{model_id}: shared by all aliases of this model. Default follows tool policy/catalog. Ctrl+S saves."
    )
}

pub(super) fn show_model_picker(app: &mut App, model_ref: &str) {
    let Some((provider_id, model_id, provider)) = model_config(app, model_ref) else {
        app.show_toast(Toast::info(
            "Select a ChatGPT model to edit its Responses lane",
        ));
        return;
    };
    let defaults = ChatGptProviderConfig::default();
    let config = provider.chatgpt.as_ref().unwrap_or(&defaults);
    let override_value = config
        .model_capabilities
        .get(model_id)
        .and_then(|c| c.responses_lite);
    let inherited = configured_responses_lite(
        &ChatGptProviderConfig {
            responses_lite: config.responses_lite,
            ..Default::default()
        },
        model_id,
    )
    .map(lane_name)
    .unwrap_or("catalog");
    let forced = match config.responses_lite {
        ResponsesLiteMode::Auto => "",
        ResponsesLiteMode::On => " · provider forces Lite",
        ResponsesLiteMode::Off => " · provider forces Full",
    };
    app.overlay = Some(Overlay::Picker(PickerOverlay {
        title: format!("Responses — {provider_id}/{model_id}{forced}"),
        items: vec![
            format!("Default ({inherited})"),
            "Full".into(),
            "Lite".into(),
        ],
        selected: match override_value {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        },
        action: PickerAction::SetModelResponsesLite {
            provider_id: provider_id.into(),
            model_id: model_id.into(),
        },
    }));
    app.focus = Focus::Overlay;
}

pub(super) fn show_provider_picker(app: &mut App, provider_id: &str) {
    let Some(provider) = app
        .settings
        .providers
        .get(provider_id)
        .filter(|p| p.resolve_type(provider_id) == ProviderType::ChatGPT)
    else {
        return;
    };
    let mode = provider
        .chatgpt
        .as_ref()
        .map(|c| c.responses_lite)
        .unwrap_or_default();
    app.overlay = Some(Overlay::Picker(PickerOverlay {
        title: format!("Responses Lane — {provider_id}"),
        items: vec![
            provider_label(ResponsesLiteMode::Auto).into(),
            provider_label(ResponsesLiteMode::Off).into(),
            provider_label(ResponsesLiteMode::On).into(),
        ],
        selected: match mode {
            ResponsesLiteMode::Auto => 0,
            ResponsesLiteMode::Off => 1,
            ResponsesLiteMode::On => 2,
        },
        action: PickerAction::SetProviderResponsesLite {
            provider_id: provider_id.into(),
        },
    }));
    app.focus = Focus::Overlay;
}

pub(super) fn apply_model_choice(app: &mut App, provider_id: &str, model_id: &str, index: usize) {
    let value = match index {
        0 => None,
        1 => Some(false),
        2 => Some(true),
        _ => return,
    };
    let Some(provider) = app
        .settings
        .providers
        .get_mut(provider_id)
        .filter(|p| p.resolve_type(provider_id) == ProviderType::ChatGPT)
    else {
        return;
    };
    let current = provider
        .chatgpt
        .as_ref()
        .and_then(|c| c.model_capabilities.get(model_id))
        .and_then(|c| c.responses_lite);
    if value == current {
        return;
    }
    let config = provider
        .chatgpt
        .get_or_insert_with(ChatGptProviderConfig::default);
    if let Some(lite) = value {
        config
            .model_capabilities
            .entry(model_id.into())
            .or_default()
            .responses_lite = Some(lite);
    } else if let Some(capability) = config.model_capabilities.get_mut(model_id) {
        capability.responses_lite = None;
        if *capability == ChatGptModelCapabilityOverride::default() {
            config.model_capabilities.remove(model_id);
        }
    }
    app.provider_statuses.remove(provider_id);
    app.mark_dirty();
    app.show_toast(Toast::success(format!(
        "{provider_id}/{model_id} Responses choice updated; Ctrl+S to save"
    )));
}

pub(super) fn apply_provider_choice(app: &mut App, provider_id: &str, index: usize) {
    let mode = match index {
        0 => ResponsesLiteMode::Auto,
        1 => ResponsesLiteMode::Off,
        2 => ResponsesLiteMode::On,
        _ => return,
    };
    let Some(provider) = app
        .settings
        .providers
        .get_mut(provider_id)
        .filter(|p| p.resolve_type(provider_id) == ProviderType::ChatGPT)
    else {
        return;
    };
    if provider
        .chatgpt
        .as_ref()
        .map(|c| c.responses_lite)
        .unwrap_or_default()
        == mode
    {
        return;
    }
    provider
        .chatgpt
        .get_or_insert_with(ChatGptProviderConfig::default)
        .responses_lite = mode;
    app.provider_statuses.remove(provider_id);
    app.mark_dirty();
    app.show_toast(Toast::success(format!(
        "{provider_id} Responses: {}; Ctrl+S to save",
        provider_label(mode)
    )));
}

#[cfg(test)]
mod tests {
    use super::super::app::{NavItem, ProviderFocus};
    use super::super::{handle_content_key, handle_overlay_key, handle_providers_key};
    use super::*;
    use aisub2api_config::{Settings, settings::ModelAliasConfig};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use serde_json::json;

    fn app() -> App {
        let mut settings = Settings::default();
        settings.providers.clear();
        settings.providers.insert(
            "work".into(),
            serde_json::from_value(json!({"provider_type":"chatgpt"})).unwrap(),
        );
        settings.model.default = ModelAliasConfig::new("work/gpt-6.1-sol");
        settings.model.sonnet = Some(ModelAliasConfig::new("work/gpt-6.1-sol"));
        settings.model.haiku = Some(ModelAliasConfig::new("work/gpt-6-luna"));
        let mut app = App::new(settings);
        app.nav = NavItem::Model;
        app.focus = Focus::Content;
        app
    }

    fn key(app: &mut App, code: KeyCode) {
        handle_content_key(app, code, KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn choose(app: &mut App, index: usize) {
        let Some(Overlay::Picker(picker)) = app.overlay.as_mut() else {
            panic!("expected picker")
        };
        picker.selected = index;
        handle_overlay_key(app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    }

    #[test]
    fn keyboard_edits_canonical_model_and_all_aliases_share_the_result() {
        let mut app = app();
        key(&mut app, KeyCode::Right);
        key(&mut app, KeyCode::Right);
        assert_eq!(app.detail_idx, 2);
        key(&mut app, KeyCode::Enter);
        let Some(Overlay::Picker(picker)) = &app.overlay else {
            panic!("expected picker")
        };
        assert_eq!(picker.items, ["Default (Full)", "Full", "Lite"]);
        assert_eq!(picker.selected, 0);
        handle_overlay_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.dirty);
        key(&mut app, KeyCode::Enter);
        choose(&mut app, 2);
        assert!(app.dirty);
        assert_eq!(
            app.settings.providers["work"]
                .chatgpt
                .as_ref()
                .unwrap()
                .model_capabilities["gpt-6.1-sol"]
                .responses_lite,
            Some(true)
        );
        assert_eq!(
            model_label(&app, app.settings.model.sonnet_name().unwrap()),
            "Lite (override)"
        );
        assert_eq!(app.settings.model.default.name, "work/gpt-6.1-sol");
        assert_eq!(app.settings.model.sonnet_name(), Some("work/gpt-6.1-sol"));
    }

    #[test]
    fn default_clears_only_the_lane_and_preserves_other_capabilities_on_round_trip() {
        let mut app = app();
        apply_model_choice(&mut app, "work", "gpt-6.1-sol", 2);
        let config = app
            .settings
            .providers
            .get_mut("work")
            .unwrap()
            .chatgpt
            .as_mut()
            .unwrap();
        config.fast_mode = true;
        let capability = config.model_capabilities.get_mut("gpt-6.1-sol").unwrap();
        capability.context_window = Some(500_000);
        capability.image_input = Some(false);
        capability.reasoning_effort_levels = Some(vec!["low".into()]);
        apply_model_choice(&mut app, "work", "gpt-6.1-sol", 0);
        let restored =
            Settings::from_toml(&app.settings.to_toml(), std::path::Path::new("config.toml"))
                .unwrap();
        let config = restored.providers["work"].chatgpt.as_ref().unwrap();
        assert!(config.fast_mode);
        assert_eq!(
            config.model_capabilities["gpt-6.1-sol"],
            ChatGptModelCapabilityOverride {
                responses_lite: None,
                context_window: Some(500_000),
                image_input: Some(false),
                reasoning_effort_levels: Some(vec!["low".into()])
            }
        );
        assert_eq!(restored.model.default.name, "work/gpt-6.1-sol");
        assert_eq!(model_label(&app, "work/gpt-6.1-sol"), "Full (default)");
    }

    #[test]
    fn clearing_a_lane_only_entry_removes_it_and_noop_default_creates_no_config() {
        let mut app = app();
        apply_model_choice(&mut app, "work", "gpt-6.1-sol", 0);
        assert!(!app.dirty);
        assert!(app.settings.providers["work"].chatgpt.is_none());
        apply_model_choice(&mut app, "work", "gpt-6.1-sol", 2);
        apply_model_choice(&mut app, "work", "gpt-6.1-sol", 0);
        assert!(
            app.settings.providers["work"]
                .chatgpt
                .as_ref()
                .unwrap()
                .model_capabilities
                .is_empty()
        );
    }

    #[test]
    fn provider_picker_controls_global_mode_without_losing_model_overrides() {
        let mut app = app();
        app.nav = NavItem::Providers;
        app.provider_focus = ProviderFocus::Detail;
        app.detail_idx = 7;
        handle_providers_key(&mut app, KeyCode::Enter);
        choose(&mut app, 2);
        assert_eq!(
            app.settings.providers["work"]
                .chatgpt
                .as_ref()
                .unwrap()
                .responses_lite,
            ResponsesLiteMode::On
        );
        assert_eq!(model_label(&app, "work/gpt-6.1-sol"), "Lite (provider)");
        show_model_picker(&mut app, "work/gpt-6.1-sol");
        let Some(Overlay::Picker(picker)) = &app.overlay else {
            panic!("expected picker")
        };
        assert!(picker.title.contains("provider forces Lite"));
        choose(&mut app, 1);
        assert_eq!(model_label(&app, "work/gpt-6.1-sol"), "Lite (provider)");
        show_provider_picker(&mut app, "work");
        choose(&mut app, 0);
        assert_eq!(model_label(&app, "work/gpt-6.1-sol"), "Full (override)");
        let restored =
            Settings::from_toml(&app.settings.to_toml(), std::path::Path::new("config.toml"))
                .unwrap();
        assert_eq!(
            restored.providers["work"]
                .chatgpt
                .as_ref()
                .unwrap()
                .responses_lite,
            ResponsesLiteMode::Auto
        );
        assert_eq!(
            restored.providers["work"]
                .chatgpt
                .as_ref()
                .unwrap()
                .model_capabilities["gpt-6.1-sol"]
                .responses_lite,
            Some(false)
        );
    }

    #[test]
    fn unknown_catalog_lane_is_not_guessed_and_non_chatgpt_models_cannot_be_edited() {
        let mut app = app();
        assert_eq!(model_label(&app, "work/new-model"), "Auto (catalog)");
        app.settings
            .providers
            .get_mut("work")
            .unwrap()
            .provider_type = Some(ProviderType::OpenAI);
        assert_eq!(model_label(&app, "work/gpt-6.1-sol"), "—");
        app.detail_idx = 2;
        key(&mut app, KeyCode::Enter);
        assert!(app.overlay.is_none());
        assert!(!app.dirty);
        assert_eq!(app.provider_detail_field_count(), 5);
        show_model_picker(&mut app, "");
        assert!(app.overlay.is_none());
    }

    #[test]
    fn renders_model_lane_sources_and_provider_control_at_common_terminal_sizes() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut app = app();
        app.detail_idx = 2;
        for (width, height) in [(120, 32), (80, 24)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| super::super::ui::render(frame, &app))
                .unwrap();
            let content = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(content.contains("Responses"), "{width}x{height}");
            assert!(content.contains("Full (default)"), "{width}x{height}");
            assert!(content.contains("Lite (default)"), "{width}x{height}");
            app.nav = NavItem::Providers;
            app.provider_focus = ProviderFocus::Detail;
            app.detail_idx = 7;
            terminal
                .draw(|frame| super::super::ui::render(frame, &app))
                .unwrap();
            let content = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(content.contains("Responses Lane"), "{width}x{height}");
            app.nav = NavItem::Model;
            app.detail_idx = 2;
        }
    }
}
