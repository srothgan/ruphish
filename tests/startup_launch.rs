//! First-start usability and diagnostic contracts through the actual executable.
use super::*;
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::mpsc,
};

struct Headless {
    child: Child,
    lines: mpsc::Receiver<String>,
    reader: Option<std::thread::JoinHandle<()>>,
    output: String,
}

impl Headless {
    fn start(mut command: Command) -> Self {
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let pipe = child.stderr.take().unwrap();
        let (send, lines) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(pipe).lines() {
                let Ok(line) = line else { break };
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            lines,
            reader: Some(reader),
            output: String::new(),
        }
    }

    fn ready(&mut self) -> Value {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let received = self
                .lines
                .recv_timeout(deadline.saturating_duration_since(Instant::now()));
            assert!(
                received.is_ok(),
                "Startup did not become ready: {}",
                self.output
            );
            let line = received.unwrap();
            self.output.push_str(&line);
            self.output.push('\n');
            if let Some(json) = line.strip_prefix("RUPHISH_READY ") {
                return serde_json::from_str(json).unwrap();
            }
        }
    }
}

impl Drop for Headless {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn command(directory: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ruphish"));
    command
        .current_dir(directory)
        .args([
            "--config",
            "config.json",
            "--mode",
            "admin",
            "--disable-mailer",
        ])
        .env_remove("GOPHISH_INITIAL_ADMIN_PASSWORD")
        .env_remove("GOPHISH_INITIAL_ADMIN_API_TOKEN");
    command
}

#[test]
fn generated_password_reaches_noninteractive_output_and_completes_required_reset() {
    let directory = workspace();
    let mut process = Headless::start(command(directory.path()));
    let base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    let password = process
        .output
        .lines()
        .find_map(|line| line.strip_prefix("Temporary password: "))
        .unwrap()
        .to_owned();
    let browser = auth_process::browser();
    assert_eq!(
        auth_process::sign_in(&browser, &base, "admin", &password).status(),
        200
    );
    assert_eq!(
        auth_process::bootstrap(&browser, &base)["user"]["password_change_required"],
        true
    );
    assert!(
        browser
            .get(format!("{base}/settings"))
            .send()
            .unwrap()
            .headers()["location"]
            .to_str()
            .unwrap()
            .starts_with("/reset_password")
    );
    assert_eq!(
        auth_process::post(
            &browser,
            &base,
            "/reset_password",
            &[
                ("password", "established-generated-bootstrap-password"),
                (
                    "confirm_password",
                    "established-generated-bootstrap-password"
                )
            ]
        )
        .status(),
        200
    );
    drop(process);
    let log = std::fs::read_to_string(directory.path().join("server.log")).unwrap();
    assert!(!log.contains(&password));
    let mut restarted = Headless::start(command(directory.path()));
    let base = format!("http://{}", restarted.ready()["admin"].as_str().unwrap());
    assert!(!restarted.output.contains("Temporary password:"));
    let browser = auth_process::browser();
    assert_eq!(
        auth_process::sign_in(
            &browser,
            &base,
            "admin",
            "established-generated-bootstrap-password"
        )
        .status(),
        200
    );
    assert_eq!(
        auth_process::bootstrap(&browser, &base)["user"]["password_change_required"],
        false
    );
}

#[test]
fn startup_diagnostics_identify_port_tls_assets_and_log_failures_without_echoing_values() {
    let occupied = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    for (case, expected) in [
        ("port", "Bind administration listener"),
        ("tls", "Prepare administration TLS"),
        ("assets", "Load administration assets"),
        ("log", "Open application log"),
    ] {
        let directory = workspace();
        let mut config = configuration();
        match case {
            "port" => {
                config["admin_server"]["listen_url"] =
                    json!(occupied.local_addr().unwrap().to_string())
            }
            "tls" => {
                config["admin_server"]["use_tls"] = json!(true);
                std::fs::write(
                    directory.path().join("admin.crt"),
                    "untrusted-certificate-canary",
                )
                .unwrap();
            }
            "assets" => std::fs::write(
                directory.path().join("dist/admin/asset-manifest.json"),
                "untrusted-manifest-canary",
            )
            .unwrap(),
            "log" => config["logging"]["filename"] = json!("untrusted-path-canary/missing/log"),
            _ => unreachable!(),
        }
        write_config(directory.path(), &config);
        let mut process = Process::start(directory.path(), "admin");
        process.fail();
        let output = process.output();
        assert!(output.contains(expected), "{case}: {output}");
        assert!(!output.contains("canary"), "{case}: {output}");
        if case == "port" {
            assert!(output.contains("AddrInUse"), "{output}");
        }
        if case == "tls" {
            assert!(output.contains("both exist or both be absent"), "{output}");
        }
    }
}
