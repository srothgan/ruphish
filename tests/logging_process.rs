//! Real process logging: rotation, concurrent ownership, stderr fallback and recovery.
use super::*;

fn logs(directory: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_str().unwrap();
            name == "server.log"
                || name
                    .strip_prefix("server.log.")
                    .is_some_and(|suffix| suffix.parse::<u64>().is_ok())
        })
        .collect()
}

fn wait_output(process: &mut Process, needle: &str) -> String {
    process.wait_output(needle)
}

#[test]
fn rotation_bounds_files_across_two_processes_and_restart_without_logging_query_secrets() {
    let directory = workspace();
    let mut config = configuration();
    config["logging"]["max_size_bytes"] = json!(1024);
    config["logging"]["max_files"] = json!(2);
    write_config(directory.path(), &config);
    let mut first = Process::start(directory.path(), "phish");
    let first_address = first.ready()["phish"].as_str().unwrap().to_owned();
    let mut second = Process::start(directory.path(), "phish");
    let second_address = second.ready()["phish"].as_str().unwrap().to_owned();
    for _ in 0..20 {
        for address in [&first_address, &second_address] {
            assert_eq!(
                client()
                    .get(format!(
                        "http://{address}/missing?password=rotation-secret-canary"
                    ))
                    .send()
                    .unwrap()
                    .status(),
                404
            );
        }
    }
    first.stop();
    second.stop();
    let files = logs(directory.path());
    assert_eq!(files.len(), 3);
    for path in &files {
        assert!(std::fs::metadata(path).unwrap().len() <= 1024);
        let text = std::fs::read_to_string(path).unwrap();
        assert!(!text.contains("rotation-secret-canary"));
        assert!(!text.contains("synthetic-bootstrap"));
        assert!(text.ends_with('\n'));
    }
    let mut restarted = Process::start(directory.path(), "phish");
    restarted.ready();
    restarted.stop();
    assert_eq!(logs(directory.path()).len(), 3);
    assert!(
        std::fs::read_to_string(directory.path().join("server.log"))
            .unwrap()
            .contains("shutdown_complete")
    );
}

#[test]
fn runtime_file_failure_keeps_http_and_stderr_working_then_recovers_file_output() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "phish");
    let address = process.ready()["phish"].as_str().unwrap().to_owned();
    let obstruction = directory.path().join("server.log.999999");
    std::fs::create_dir(&obstruction).unwrap();
    std::fs::write(obstruction.join("private-error-canary"), "operator file").unwrap();
    for _ in 0..3 {
        assert_eq!(
            client()
                .get(format!("http://{address}/missing"))
                .send()
                .unwrap()
                .status(),
            404
        );
    }
    let output = wait_output(&mut process, "Application file logging unavailable");
    assert!(!output.contains("private-error-canary"));
    assert!(output.contains("stderr logging remains active"));
    assert_eq!(
        std::fs::read_to_string(obstruction.join("private-error-canary")).unwrap(),
        "operator file"
    );
    std::fs::remove_file(obstruction.join("private-error-canary")).unwrap();
    std::fs::remove_dir(obstruction).unwrap();
    assert_eq!(
        client()
            .get(format!("http://{address}/missing"))
            .send()
            .unwrap()
            .status(),
        404
    );
    wait_output(&mut process, "Application file logging resumed");
    process.stop();
    let output = process.output();
    assert_eq!(
        output
            .matches("Application file logging unavailable")
            .count(),
        1
    );
    assert_eq!(
        output.matches("Application file logging resumed").count(),
        1
    );
    assert!(
        std::fs::read_to_string(directory.path().join("server.log"))
            .unwrap()
            .contains("shutdown_complete")
    );
}

#[test]
fn stderr_only_logging_creates_no_file_or_lock_and_preserves_request_diagnostics() {
    let directory = workspace();
    let mut config = configuration();
    config["logging"]["filename"] = json!("");
    write_config(directory.path(), &config);
    let mut process = Process::start(directory.path(), "phish");
    let address = process.ready()["phish"].as_str().unwrap().to_owned();
    assert_eq!(
        client()
            .get(format!("http://{address}/missing"))
            .send()
            .unwrap()
            .status(),
        404
    );
    process.stop();
    assert!(logs(directory.path()).is_empty());
    assert!(!directory.path().join("server.log.lock").exists());
    let output = process.output();
    assert!(output.contains("http_request") && output.contains("shutdown_complete"));
}
