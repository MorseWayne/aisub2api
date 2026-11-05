#![cfg(unix)]

use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

struct Fixture {
    root: std::path::PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "aisub2api-claude-sync-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("claude")).unwrap();
        std::fs::write(
            root.join("input.toml"),
            r#"
[providers.first]
provider_type = "openai"
api_key = "fixture-upstream-key"
base_url = "http://127.0.0.1:1/v1"
[providers.second]
provider_type = "openai"
api_key = "fixture-upstream-key"
base_url = "http://127.0.0.1:1/v1"
[model]
default = "first/fixture-model"
[server]
auth_token = "fixture-client-key"
"#,
        )
        .unwrap();
        Self { root }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_aisub2api"))
            .args(args)
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("XDG_CACHE_HOME", self.root.join("cache"))
            .env("CLAUDE_CONFIG_DIR", self.root.join("claude"))
            .current_dir(&self.root)
            .output()
            .unwrap()
    }

    fn settings_path(&self) -> std::path::PathBuf {
        self.root.join("claude/settings.json")
    }

    fn settings(&self) -> Value {
        serde_json::from_slice(&std::fs::read(self.settings_path()).unwrap()).unwrap()
    }

    fn import(&self) -> Output {
        let output = self.run(&["config", "import", "input.toml"]);
        assert_success(&output);
        output
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_import_and_provider_switch_sync_claude_without_starting_server() {
    let fixture = Fixture::new();
    fixture.import();
    let settings = fixture.settings();
    assert_eq!(settings["env"]["ANTHROPIC_MODEL"], "first/fixture-model");
    assert_eq!(settings["env"]["ENABLE_TOOL_SEARCH"], "auto");
    assert_eq!(settings["env"]["CLAUDE_CODE_ENABLE_TODO_TOOLS"], "1");
    assert_eq!(settings["env"]["CLAUDE_CODE_MAX_OUTPUT_TOKENS"], "128000");

    assert_success(&fixture.run(&["provider", "switch", "second"]));
    let saved =
        aisub2api_config::Settings::load(&fixture.root.join("config/aisub2api/config.toml"))
            .unwrap();
    assert!(saved.model.default.name.starts_with("second/"));
    let settings = fixture.settings();
    for key in [
        "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
    ] {
        assert_eq!(settings["env"][key], saved.model.default.name);
    }
    assert!(!fixture.root.join("config/aisub2api/aisub2api.pid").exists());
}

#[test]
fn cli_sync_preserves_explicit_compatibility_options_and_picker() {
    let fixture = Fixture::new();
    let initial = json!({
        "theme": "dark",
        "env": {
            "ENABLE_TOOL_SEARCH": "true",
            "CLAUDE_CODE_ENABLE_TODO_TOOLS": "0",
            "CLAUDE_CODE_ENABLE_TASKS": "0",
            "CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING": "0",
            "CLAUDE_CODE_MAX_OUTPUT_TOKENS": "8192"
        },
        "modelPicker": {"options": [{"model": "first/fixture-model"}]}
    });
    std::fs::write(fixture.settings_path(), initial.to_string()).unwrap();
    fixture.import();
    let settings = fixture.settings();
    assert_eq!(settings["theme"], initial["theme"]);
    assert_eq!(settings["modelPicker"], initial["modelPicker"]);
    for (key, value) in initial["env"].as_object().unwrap() {
        assert_eq!(&settings["env"][key], value, "overwrote {key}");
    }
}

#[test]
fn cli_sync_reports_invalid_claude_json_without_overwriting_it() {
    for invalid in ["{invalid-json", "[]", r#"{"env":true}"#] {
        let fixture = Fixture::new();
        std::fs::write(fixture.settings_path(), invalid).unwrap();
        let output = fixture.import();
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Claude Code settings sync failed")
        );
        assert_eq!(
            std::fs::read_to_string(fixture.settings_path()).unwrap(),
            invalid
        );
        assert!(fixture.root.join("config/aisub2api/config.toml").exists());
    }
}

#[test]
fn cli_sync_keeps_existing_client_route_during_incomplete_provider_setup() {
    let fixture = Fixture::new();
    let input = fixture.root.join("input.toml");
    let incomplete = std::fs::read_to_string(&input)
        .unwrap()
        .replace("first/fixture-model", "missing/fixture-model");
    std::fs::write(input, incomplete).unwrap();
    let existing = r#"{"env":{"ANTHROPIC_MODEL":"existing/model"}}"#;
    std::fs::write(fixture.settings_path(), existing).unwrap();
    fixture.import();
    assert_eq!(
        std::fs::read_to_string(fixture.settings_path()).unwrap(),
        existing
    );
}
