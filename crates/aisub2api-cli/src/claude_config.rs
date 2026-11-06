use std::path::{Path, PathBuf};

use aisub2api_config::Settings;
use serde_json::{Map, Value};

use crate::tui;

pub(crate) fn read_settings(path: &Path) -> Result<Value, String> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("failed to read Claude Code settings: {error}")),
    };
    let value = if content.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_str(&content)
            .map_err(|error| format!("failed to parse Claude Code settings JSON: {error}"))?
    };
    if !value.is_object() {
        return Err("Claude Code settings root must be a JSON object".into());
    }
    if value.get("env").is_some_and(|env| !env.is_object()) {
        return Err("Claude Code settings env must be a JSON object".into());
    }
    Ok(value)
}

pub(crate) fn write_settings(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create Claude Code config directory: {error}"))?;
    }
    let content = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize Claude Code settings: {error}"))?;
    std::fs::write(path, format!("{content}\n"))
        .map_err(|error| format!("failed to write Claude Code settings: {error}"))
}

pub(crate) fn set_webfetch_preflight(skip: bool) -> Result<PathBuf, String> {
    let path = tui::claude_code_settings_path()?;
    let mut value = read_settings(&path)?;
    value["skipWebFetchPreflight"] = Value::Bool(skip);
    if let Some(env) = value.get_mut("env").and_then(Value::as_object_mut) {
        env.remove("skipWebFetchPreflight");
    }
    write_settings(&path, &value)?;
    Ok(path)
}

pub(crate) struct Diagnostic {
    pub warning: bool,
    pub message: String,
}

impl Diagnostic {
    fn info(message: impl Into<String>) -> Self {
        Self {
            warning: false,
            message: message.into(),
        }
    }

    fn warning(message: impl Into<String>) -> Self {
        Self {
            warning: true,
            message: message.into(),
        }
    }
}

pub(crate) fn check(settings: &Settings) -> Result<(PathBuf, Vec<Diagnostic>), String> {
    let path = tui::claude_code_settings_path()?;
    if !path.exists() {
        return Ok((
            path,
            vec![Diagnostic::info(
                "Claude Code settings not found; run `aisub2api config claude` to sync if you use Claude Code.",
            )],
        ));
    }
    let value = read_settings(&path)?;
    Ok((path, diagnose(&value, settings)?))
}

fn diagnose(value: &Value, settings: &Settings) -> Result<Vec<Diagnostic>, String> {
    if value
        .get("skipWebFetchPreflight")
        .is_some_and(|v| !v.is_boolean())
    {
        return Err("skipWebFetchPreflight must be a top-level JSON boolean; run `aisub2api config claude --webfetch-preflight check` or `skip` to set it.".into());
    }
    let empty = Map::new();
    let env = value
        .get("env")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let get = |key: &str| env.get(key).and_then(Value::as_str);
    let mut diagnostics = Vec::new();

    // Compare the configuration we actually generate, including role inheritance
    // and per-model context projection. Never print credentials or input values.
    let mut expected = Value::Object(Map::new());
    tui::apply_claude_code_env(&mut expected, settings);
    for key in [
        "ANTHROPIC_BASE_URL",
        "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
    ] {
        if let Some(want) = expected["env"].get(key)
            && env.get(key) != Some(want)
        {
            diagnostics.push(Diagnostic::warning(format!(
                "{key} differs from aisub2api's generated route; run `aisub2api config claude` to sync if this override is unintended."
            )));
        }
    }
    if let Some(want) = expected["env"].get("ANTHROPIC_API_KEY")
        && env.get("ANTHROPIC_API_KEY") != Some(want)
    {
        diagnostics.push(Diagnostic::warning(
            "ANTHROPIC_API_KEY differs from the proxy credential; run `aisub2api config claude` to sync.",
        ));
    }
    if get("ANTHROPIC_AUTH_TOKEN").is_some_and(|v| !v.is_empty()) {
        diagnostics.push(Diagnostic::warning(
            "ANTHROPIC_AUTH_TOKEN may select a different credential; normal aisub2api sync removes it.",
        ));
    }

    match get("CLAUDE_CODE_ENABLE_TODO_TOOLS") {
        Some(v) if enabled(v) => diagnostics.push(Diagnostic::info("Local task tools are enabled.")),
        Some(_) => diagnostics.push(Diagnostic::warning(
            "CLAUDE_CODE_ENABLE_TODO_TOOLS disables local task tools for unrecognized models. Explicit overrides are preserved; set it to \"1\" if tasks are missing.",
        )),
        None => diagnostics.push(Diagnostic::warning(
            "CLAUDE_CODE_ENABLE_TODO_TOOLS is missing; unrecognized models may lack TaskCreate/TaskUpdate. Run `aisub2api config claude` to add the default.",
        )),
    }
    if get("CLAUDE_CODE_ENABLE_TASKS").is_some_and(|v| !enabled(v)) {
        diagnostics.push(Diagnostic::info(
            "CLAUDE_CODE_ENABLE_TASKS selects legacy TodoWrite; TaskCreate/TaskUpdate will not be available even when TODO_TOOLS is enabled.",
        ));
    }
    match get("ENABLE_TOOL_SEARCH") {
        Some("false" | "0") => diagnostics.push(Diagnostic::info("Tool search is disabled; tool schemas load upfront.")),
        Some("true" | "1") => diagnostics.push(Diagnostic::info("Tool search is forced; tool-reference discovery must work through the selected provider.")),
        Some(v) if v == "auto" || v.strip_prefix("auto:").is_some_and(|n| n.parse::<u32>().is_ok()) =>
            diagnostics.push(Diagnostic::info("Tool search uses an automatic threshold.")),
        Some(_) => diagnostics.push(Diagnostic::warning("ENABLE_TOOL_SEARCH is unrecognized; use true, false, auto, or auto:N.")),
        None => diagnostics.push(Diagnostic::warning("ENABLE_TOOL_SEARCH is unset; Claude may defer tools by default. Run `aisub2api config claude` to select auto.")),
    }

    if env.contains_key("skipWebFetchPreflight") {
        diagnostics.push(Diagnostic::warning(
            "env.skipWebFetchPreflight is misplaced; this is a top-level boolean, not an environment variable. Use `aisub2api config claude --webfetch-preflight skip` to set it at the correct location.",
        ));
    }
    diagnostics.push(Diagnostic::info(if value["skipWebFetchPreflight"] == true {
        "WebFetch skips Anthropic's domain blocklist check; WebFetch permission rules still apply."
    } else {
        "WebFetch depends on Anthropic's domain check. If domain verification fails, use `aisub2api config claude --webfetch-preflight skip`; this opts out of the domain blocklist check."
    }));

    let cloud_provider = [
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_FOUNDRY",
        "CLAUDE_CODE_USE_ANTHROPIC_AWS",
        "CLAUDE_CODE_USE_ANTHROPIC_GOOGLE_CLOUD",
        "CLAUDE_CODE_USE_MANTLE",
        "CLAUDE_CODE_USE_GATEWAY",
    ]
    .iter()
    .any(|key| get(key).is_some_and(enabled));
    if !cloud_provider && get("ANTHROPIC_BASE_URL").is_some() {
        for key in [
            "ANTHROPIC_DEFAULT_FABLE_MODEL_SUPPORTED_CAPABILITIES",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_SUPPORTED_CAPABILITIES",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_SUPPORTED_CAPABILITIES",
            "ANTHROPIC_CUSTOM_MODEL_OPTION_SUPPORTED_CAPABILITIES",
        ] {
            if env.contains_key(key) {
                diagnostics.push(Diagnostic::warning(format!(
                    "{key} does not override capabilities in ordinary ANTHROPIC_BASE_URL mode (verified on Claude Code 2.1.318)."
                )));
            }
        }
        let model = get("ANTHROPIC_MODEL").unwrap_or("").to_ascii_lowercase();
        if get("CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING").is_some_and(enabled)
            && model.contains("gpt")
            && !model.contains("opus-4-6")
            && !model.contains("sonnet-4-6")
            && value.get("modelOverrides").is_none()
        {
            diagnostics.push(Diagnostic::warning(
                "CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING does not disable adaptive thinking for GPT IDs on Claude Code 2.1.318; do not rely on it to fix rejected request fields.",
            ));
        }
    }
    if get("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS").is_some_and(enabled) {
        diagnostics.push(Diagnostic::warning(
            "CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS removes features such as context management, but may retain thinking, effort, and some beta headers.",
        ));
    }
    if [
        "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
        "DISABLE_TELEMETRY",
    ]
    .iter()
    .any(|key| get(key).is_some_and(|v| !v.is_empty()))
        || ["DISABLE_GROWTHBOOK", "DO_NOT_TRACK"]
            .iter()
            .any(|key| get(key).is_some_and(enabled))
    {
        diagnostics.push(Diagnostic::info(
            "Remote feature-flag fetching is disabled by a configured traffic/telemetry setting; dependent cloud features may be unavailable. This does not disable WebFetch's domain check.",
        ));
    }
    if get("CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY").is_some_and(enabled) {
        diagnostics.push(Diagnostic::info(
            "Gateway model discovery filters IDs without claude/anthropic on Claude Code 2.1.318. Use explicit IDs, role mappings, or a custom modelPicker for GPT models.",
        ));
    }
    Ok(diagnostics)
}

fn enabled(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn diagnoses_ineffective_options_without_disclosing_values() {
        let value = json!({"env":{
            "ANTHROPIC_BASE_URL":"https://private:secret@localhost.invalid",
            "ANTHROPIC_API_KEY":"private-key",
            "ANTHROPIC_AUTH_TOKEN":"private-token",
            "ANTHROPIC_MODEL":"chatgpt/gpt-fixture",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_SUPPORTED_CAPABILITIES":"effort",
            "CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING":"1",
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC":"0",
            "skipWebFetchPreflight":"true"
        }});
        let diagnostics = diagnose(&value, &Settings::default()).unwrap();
        let text = diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for expected in [
            "misplaced",
            "does not override capabilities",
            "does not disable adaptive",
            "Remote feature-flag fetching is disabled",
        ] {
            assert!(text.contains(expected), "missing {expected}");
        }
        for private in ["private", "secret", "localhost.invalid", "gpt-fixture"] {
            assert!(!text.contains(private), "disclosed {private}");
        }
    }

    #[test]
    fn does_not_misdiagnose_cloud_capability_overrides() {
        let value = json!({"env":{
            "ANTHROPIC_BASE_URL":"https://fixture.invalid",
            "CLAUDE_CODE_USE_BEDROCK":"1",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES":"effort"
        }});
        let diagnostics = diagnose(&value, &Settings::default()).unwrap();
        assert!(
            !diagnostics
                .iter()
                .any(|d| d.message.contains("does not override capabilities"))
        );
    }
}
