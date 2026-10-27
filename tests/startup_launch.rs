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

fn default_command(directory: &Path, profile: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ruphish"));
    command
        .current_dir(directory)
        .args(["--mode", "admin", "--disable-mailer"])
        .env_remove("GOPHISH_INITIAL_ADMIN_PASSWORD")
        .env_remove("GOPHISH_INITIAL_ADMIN_API_TOKEN")
        .env("LOCALAPPDATA", profile.join("local"))
        .env("HOME", profile.join("home"))
        .env("XDG_CONFIG_HOME", profile.join("config"))
        .env("XDG_DATA_HOME", profile.join("data"))
        .env("XDG_STATE_HOME", profile.join("state"));
    command
}

fn default_config(profile: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        profile.join("local/ruphish/config.json")
    }
    #[cfg(target_os = "linux")]
    {
        profile.join("config/ruphish/config.json")
    }
    #[cfg(target_os = "macos")]
    {
        profile.join("home/Library/Application Support/ruphish/config.json")
    }
}

fn failed(command: Command) -> String {
    let mut process = Headless::start(command);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = process.child.try_wait().unwrap() {
            assert!(!status.success());
            process.reader.take().unwrap().join().unwrap();
            for line in process.lines.try_iter() {
                process.output.push_str(&line);
                process.output.push('\n');
            }
            return process.output.clone();
        }
        assert!(
            Instant::now() < deadline,
            "Expected startup to fail: {}",
            process.output
        );
        if let Ok(line) = process.lines.recv_timeout(Duration::from_millis(20)) {
            process.output.push_str(&line);
            process.output.push('\n');
        }
    }
}

#[test]
fn os_default_config_creates_sqlite_logs_and_preserves_login_across_working_directories() {
    let profile = tempfile::tempdir().unwrap();
    let initial = tempfile::tempdir().unwrap();
    // A loose example file must never control the built-in defaults.
    std::fs::write(
        initial.path().join("config.example.json"),
        b"malicious loose example",
    )
    .unwrap();
    let output = failed(default_command(initial.path(), profile.path()));
    // Development builds still require assets until package 13 embeds them.
    assert!(output.contains("Load administration assets"), "{output}");
    let path = default_config(profile.path());
    let mut config: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(config["db_name"], "sqlite3");
    let database = PathBuf::from(config["db_path"].as_str().unwrap());
    let log = PathBuf::from(config["logging"]["filename"].as_str().unwrap());
    assert!(database.is_absolute() && log.is_absolute());
    assert!(database.starts_with(profile.path()) && log.starts_with(profile.path()));
    assert!(!database.exists());
    config["admin_server"]["listen_url"] = json!("127.0.0.1:0");
    assert_eq!(config["admin_server"]["use_tls"], true);
    let certificate = PathBuf::from(config["admin_server"]["cert_path"].as_str().unwrap());
    let key = PathBuf::from(config["admin_server"]["key_path"].as_str().unwrap());
    let bytes = serde_json::to_vec(&config).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let first_directory = workspace();
    let mut first = Headless::start(default_command(first_directory.path(), profile.path()));
    let base = format!("https://{}", first.ready()["admin"].as_str().unwrap());
    let password = first
        .output
        .lines()
        .find_map(|line| line.strip_prefix("Temporary password: "))
        .unwrap()
        .to_owned();
    let original_certificate = std::fs::read(&certificate).unwrap();
    let original_key = std::fs::read(&key).unwrap();
    let browser = || {
        reqwest::blocking::Client::builder()
            .no_proxy()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(20))
            .add_root_certificate(reqwest::Certificate::from_pem(&original_certificate).unwrap())
            .build()
            .unwrap()
    };
    let first_browser = browser();
    assert_eq!(
        auth_process::sign_in(&first_browser, &base, "admin", &password).status(),
        200
    );
    assert_eq!(
        auth_process::post(
            &first_browser,
            &base,
            "/auth/password/reset",
            &[
                ("password", "os-default-established-password"),
                ("confirm_password", "os-default-established-password")
            ]
        )
        .status(),
        200
    );
    drop(first);
    assert!(database.is_file() && log.is_file());
    assert!(!first_directory.path().join("installation.sqlite3").exists());
    let before = std::fs::metadata(&log).unwrap().len();
    assert!(before > 0);
    let second_directory = workspace();
    let mut second = Headless::start(default_command(second_directory.path(), profile.path()));
    let base = format!("https://{}", second.ready()["admin"].as_str().unwrap());
    assert!(!second.output.contains("Temporary password:"));
    assert_eq!(
        auth_process::sign_in(
            &browser(),
            &base,
            "admin",
            "os-default-established-password"
        )
        .status(),
        200
    );
    drop(second);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(std::fs::read(&certificate).unwrap(), original_certificate);
    assert_eq!(std::fs::read(&key).unwrap(), original_key);
    assert!(std::fs::metadata(&log).unwrap().len() > before);
    assert!(!std::fs::read_to_string(&log).unwrap().contains(&password));
    assert!(
        !second_directory
            .path()
            .join("installation.sqlite3")
            .exists()
    );

    // A bad edited path must fail instead of recreating a default database/config.
    config["db_path"] = json!(profile.path());
    let broken = serde_json::to_vec(&config).unwrap();
    std::fs::write(&path, &broken).unwrap();
    let output = failed(default_command(second_directory.path(), profile.path()));
    assert!(output.contains("Open or upgrade database"), "{output}");
    assert_eq!(std::fs::read(&path).unwrap(), broken);
    assert!(database.is_file());

    let malformed = b"{private-malformed-config-canary";
    std::fs::write(&path, malformed).unwrap();
    let output = failed(default_command(second_directory.path(), profile.path()));
    assert!(output.contains("Invalid configuration JSON"), "{output}");
    assert!(
        output.contains("Configuration: ") && output.contains("config.json"),
        "{output}"
    );
    assert!(output.contains(profile.path().file_name().unwrap().to_str().unwrap()));
    assert!(!output.contains("private-malformed-config-canary"));
    assert_eq!(std::fs::read(&path).unwrap(), malformed);
}

#[test]
fn explicit_missing_config_does_not_initialize_the_os_installation() {
    let directory = tempfile::tempdir().unwrap();
    let profile = directory.path().join("unused-profile");
    let mut command = default_command(directory.path(), &profile);
    command.args(["--config", "missing/config.json"]);
    let output = failed(command);
    assert!(output.contains("Could not read configuration"));
    assert!(!profile.exists());
    assert!(!directory.path().join("missing").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn absent_empty_and_relative_xdg_variables_use_home_directories() {
    for value in [None, Some(""), Some("relative-directory")] {
        let profile = tempfile::tempdir().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let mut command = default_command(directory.path(), profile.path());
        for variable in ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME"] {
            if let Some(value) = value {
                command.env(variable, value);
            } else {
                command.env_remove(variable);
            }
        }
        let output = failed(command);
        assert!(output.contains("Load administration assets"), "{output}");
        let home = profile.path().join("home");
        let config: Value = serde_json::from_slice(
            &std::fs::read(home.join(".config/ruphish/config.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            Path::new(config["db_path"].as_str().unwrap()).parent(),
            Some(home.join(".local/share/ruphish").as_path())
        );
        assert_eq!(
            Path::new(config["logging"]["filename"].as_str().unwrap()).parent(),
            Some(home.join(".local/state/ruphish").as_path())
        );
        assert!(!directory.path().join("relative-directory").exists());
    }
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
            "/auth/password/reset",
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
