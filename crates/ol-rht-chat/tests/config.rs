//! Startup configuration fails before opening the database or contacting a provider.

use std::process::Command;

fn startup(args: &[&str], key: Option<&str>) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ol-rht-chat"));
    command
        .env_remove("OL_RHT_DATABASE_URL")
        .env_remove("OL_RHT_CHAT_MODEL")
        .env_remove("OPENROUTER_API_KEY")
        .env_remove("OL_RHT_OPENROUTER_PROVIDER")
        .env_remove("OL_RHT_CHAT_MAX_CONCURRENT")
        .env_remove("OL_RHT_CHAT_REPLIES_PER_MINUTE")
        .env_remove("OL_RHT_CHAT_WRITES_PER_MINUTE")
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
fn public_listener_and_service_dns_require_explicit_demo_mode() {
    let args = [
        "--listen",
        "0.0.0.0:50053",
        "--corpus",
        "http://ol-rht-serve:50052",
    ];
    assert!(startup(&args, Some("test-key")).contains("development listener must be on loopback"));
    assert!(
        startup(&["--corpus", "http://ol-rht-serve:50052"], Some("test-key"))
            .contains("Development service URLs must be HTTP on loopback")
    );
    let demo = [&["--mode", "public-demo"][..], &args].concat();
    assert!(
        startup(&demo, Some("test-key"))
            .starts_with("ol-rht-chat: OL_RHT_DATABASE_URL is required")
    );
    assert!(startup(&["--mode", "production"], Some("test-key")).contains("--mode must be"));
}

#[test]
fn service_configuration_still_rejects_credentials_and_url_extras() {
    for url in [
        "ftp://ol-rht-serve",
        "http://user:pass@ol-rht-serve",
        "http://ol-rht-serve?key=secret",
        "http://ol-rht-serve/#fragment",
    ] {
        assert!(
            ol_rht_chat::Config::local("http://127.0.0.1:8081/v1", "test")
                .unwrap()
                .with_service_corpus(url)
                .is_err()
        );
    }
    for limits in [
        ol_rht_chat::Limits {
            concurrent_replies: 0,
            ..Default::default()
        },
        ol_rht_chat::Limits {
            replies_per_minute: 0,
            ..Default::default()
        },
        ol_rht_chat::Limits {
            history_writes_per_minute: 0,
            ..Default::default()
        },
    ] {
        assert!(
            ol_rht_chat::Config::local("http://127.0.0.1:8081/v1", "test")
                .unwrap()
                .with_limits(limits)
                .is_err()
        );
    }
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
