#[cfg(unix)]
#[test]
fn config_path_succeeds_before_config_exists() {
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "aisub2api-config-path-{}-{suffix}",
        std::process::id()
    ));
    let config_home = root.join("config");
    let cache_home = root.join("cache");

    let output = Command::new(env!("CARGO_BIN_EXE_aisub2api"))
        .args(["config", "path"])
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_CACHE_HOME", &cache_home)
        .output()
        .expect("config path command runs");

    assert!(
        output.status.success(),
        "config path failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        config_home
            .join("aisub2api/config.toml")
            .display()
            .to_string()
    );

    let _ = std::fs::remove_dir_all(root);
}
