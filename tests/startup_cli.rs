//! CLI compatibility exercised through the real executable in isolated working directories.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "Isolated CLI fixture/process setup is confined to this integration-test binary"
)]

use std::{
    io::Read,
    process::{Command, Output, Stdio},
};

fn capture(mut reader: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut output = Vec::new();
        let mut buffer = [0; 4096];
        while let Ok(length) = reader.read(&mut buffer) {
            if length == 0 {
                break;
            }
            let available = 65536usize.saturating_sub(output.len());
            output.extend_from_slice(&buffer[..length.min(available)]);
        }
        output
    })
}

fn run(arguments: &[&str]) -> (tempfile::TempDir, Output) {
    let directory = tempfile::Builder::new()
        .prefix("ruphish cli ")
        .tempdir()
        .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ruphish"))
        .current_dir(directory.path())
        .env_remove("GOPHISH_INITIAL_ADMIN_PASSWORD")
        .env_remove("GOPHISH_INITIAL_ADMIN_API_TOKEN")
        .env("LOCALAPPDATA", directory.path().join("os-local"))
        .env("HOME", directory.path().join("os-home"))
        .env("XDG_CONFIG_HOME", directory.path().join("os-config"))
        .env("XDG_DATA_HOME", directory.path().join("os-data"))
        .env("XDG_STATE_HOME", directory.path().join("os-state"))
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = capture(child.stdout.take().unwrap());
    let stderr = capture(child.stderr.take().unwrap());
    let status = child.wait().unwrap();
    let output = Output {
        status,
        stdout: stdout.join().unwrap(),
        stderr: stderr.join().unwrap(),
    };
    (directory, output)
}

#[test]
fn informational_cli_preserves_streams_and_does_not_require_or_create_runtime_files() {
    for (arguments, stdout) in [
        (vec!["--help"], false),
        (vec!["-h"], false),
        (vec!["--help-long"], true),
        (vec!["--help-man"], true),
        (vec!["--version"], false),
    ] {
        let (directory, output) = run(&arguments);
        assert!(output.status.success(), "{arguments:?}: {output:?}");
        let text = if stdout {
            &output.stdout
        } else {
            &output.stderr
        };
        let unused = if stdout {
            &output.stderr
        } else {
            &output.stdout
        };
        assert!(unused.is_empty(), "Unexpected output stream: {arguments:?}");
        let text = String::from_utf8_lossy(text);
        if arguments == ["--version"] {
            assert!(text.contains(env!("CARGO_PKG_VERSION")));
        } else {
            for option in ["--config", "--mode", "--disable-mailer"] {
                assert!(text.contains(option), "Missing {option}: {arguments:?}");
            }
        }
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}

#[test]
fn invalid_cli_and_missing_configuration_fail_without_starting_the_application() {
    for arguments in [
        vec!["--mode", "invalid"],
        vec!["--unknown-flag"],
        vec!["--config"],
        vec!["--config", "missing config.json"],
    ] {
        let (directory, output) = run(&arguments);
        assert_eq!(output.status.code(), Some(1), "{arguments:?}: {output:?}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}

#[test]
fn independently_captured_go_boolean_aliases_and_option_boundaries_match() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/startup/cli.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let args: Vec<&str> = case["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap())
            .collect();
        let (directory, output) = run(&args);
        assert_eq!(
            output.status.code(),
            case["exit_code"]
                .as_i64()
                .map(|code| i32::try_from(code).unwrap()),
            "{args:?}: {output:?}"
        );
        assert_eq!(
            output.stdout.is_empty(),
            case["stdout_empty"].as_bool().unwrap(),
            "{args:?}"
        );
        assert_eq!(
            output.stderr.is_empty(),
            case["stderr_empty"].as_bool().unwrap(),
            "{args:?}"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).contains("Could not read configuration"),
            case["configuration_read_attempted"].as_bool().unwrap(),
            "{args:?}: {output:?}"
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}
