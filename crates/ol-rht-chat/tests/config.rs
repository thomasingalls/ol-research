//! Startup configuration fails before opening the database or contacting a provider.

use std::process::Command;

fn startup(args: &[&str], key: Option<&str>) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ol-rht-chat"));
    command
        .env_remove("OL_RHT_DATABASE_URL")
        .env_remove("OL_RHT_CHAT_MODEL")
        .env_remove("OPENROUTER_API_KEY")
        .env("OL_RHT_CHAT_PROVIDER", "openrouter")
        .args(args);
    if let Some(key) = key {
        command.env("OPENROUTER_API_KEY", key);
    }
    let output = command.output().unwrap();
    assert!(!output.status.success());
    String::from_utf8(output.stderr).unwrap()
}

#[test]
fn openrouter_fails_early_without_a_key() {
    for key in [None, Some(""), Some("   ")] {
        assert!(startup(&[], key).starts_with("ol-rht-chat: Set OPENROUTER_API_KEY"));
    }
    let error = startup(&[], Some("test-startup-secret"));
    assert!(error.starts_with("ol-rht-chat: OL_RHT_DATABASE_URL is required"));
    assert!(!error.contains("test-startup-secret"));
}

#[test]
fn provider_selection_is_explicit_and_cli_overrides_environment() {
    assert!(
        startup(&["--provider", "local"], None)
            .starts_with("ol-rht-chat: OL_RHT_DATABASE_URL is required")
    );
    assert!(
        startup(&["--provider", "unknown"], None).starts_with(
            "ol-rht-chat: OL_RHT_CHAT_PROVIDER / --provider must be local or openrouter"
        )
    );
    assert!(
        startup(
            &["--upstream", "http://127.0.0.1:8081/v1"],
            Some("test-key")
        )
        .starts_with("ol-rht-chat: --upstream is only supported with --provider local")
    );
}
