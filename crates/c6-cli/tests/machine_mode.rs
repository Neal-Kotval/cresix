#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, process::Command};

use c6_cli::config::{Config, Paths, Server};

#[test]
fn json_clone_without_token_never_starts_git_and_exits_unauthenticated() {
    let temp = tempfile::tempdir().unwrap();
    let paths = Paths {
        directory: temp.path().join("state"),
    };
    let mut config = Config {
        default_server: Some("work".into()),
        ..Config::default()
    };
    config.servers.insert(
        "work".into(),
        Server {
            base_url: "https://c6.example".into(),
            server_id: "00000000-0000-0000-0000-000000000001".into(),
            allow_http_localhost: false,
        },
    );
    config.save(&paths).unwrap();

    let fake_bin = temp.path().join("bin");
    fs::create_dir(&fake_bin).unwrap();
    let git_marker = temp.path().join("git-was-started");
    let fake_git = fake_bin.join("git");
    fs::write(
        &fake_git,
        format!("#!/bin/sh\n: > '{}'\nexit 99\n", git_marker.display()),
    )
    .unwrap();
    fs::set_permissions(&fake_git, fs::Permissions::from_mode(0o700)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_c6"))
        .args(["--json", "clone", "paper-street/weeknote"])
        .env("C6_CONFIG_DIR", &paths.directory)
        .env("PATH", &fake_bin)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(10));
    assert!(
        !git_marker.exists(),
        "Git was started before authentication failed"
    );
    let failure: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(failure["version"], 1);
    assert_eq!(failure["ok"], false);
    assert_eq!(failure["error"]["code"], "unauthenticated");
}
