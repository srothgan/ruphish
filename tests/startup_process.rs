//! End-to-end startup and recovery using the real executable and native terminal signals.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated process/fixture setup in this integration-test harness must fail the test on errors"
)]

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::{Duration, Instant},
};

#[path = "auth_process.rs"]
mod auth_process;
#[path = "campaign_process.rs"]
mod campaign_process;
#[path = "support/database_network.rs"]
mod database_network;
#[path = "support/domain_network.rs"]
mod domain_network;
#[path = "support/mailbox.rs"]
mod mailbox;
#[path = "mysql_process.rs"]
mod mysql_process;
#[path = "overview_process.rs"]
mod overview_process;
#[path = "postgres_process.rs"]
mod postgres_process;
#[path = "support/process_diagnostics.rs"]
mod process_diagnostics;
#[path = "public_process.rs"]
mod public_process;
#[path = "reporting_process.rs"]
mod reporting_process;
#[path = "resource_process.rs"]
mod resource_process;
#[path = "server_database_process.rs"]
mod server_database_process;
#[path = "starter_templates_process.rs"]
mod starter_templates_process;
use domain_network::tls_fixture;
#[path = "support/webhook_receiver.rs"]
mod webhook_receiver;

struct Process {
    directory: PathBuf,
    mode: String,
    started: Instant,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Option<Box<dyn Write + Send>>,
    master: Option<Box<dyn portable_pty::MasterPty + Send>>,
    output: Arc<(Mutex<VecDeque<String>>, Condvar)>,
    reader: Option<std::thread::JoinHandle<()>>,
}

impl Process {
    fn start(directory: &Path, mode: &str) -> Self {
        Self::with_credentials(
            directory,
            mode,
            Some("synthetic-bootstrap-password"),
            Some("synthetic-bootstrap-token"),
        )
    }

    fn with_credentials(
        directory: &Path,
        mode: &str,
        password: Option<&str>,
        token: Option<&str>,
    ) -> Self {
        Self::with_options(directory, mode, password, token, &[])
    }

    fn with_options(
        directory: &Path,
        mode: &str,
        password: Option<&str>,
        token: Option<&str>,
        options: &[&str],
    ) -> Self {
        let started = Instant::now();
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 40,
                cols: 1000,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_ruphish"));
        command.cwd(directory);
        command.args(["--mode", mode]);
        command.args(options);
        for (name, value) in [
            ("GOPHISH_INITIAL_ADMIN_PASSWORD", password),
            ("GOPHISH_INITIAL_ADMIN_API_TOKEN", token),
        ] {
            if let Some(value) = value {
                command.env(name, value);
            } else {
                command.env_remove(name);
            }
        }
        command.env("NO_COLOR", "true");
        if directory.join("mailbox-ca.pem").is_file() {
            command.env("SSL_CERT_FILE", directory.join("mailbox-ca.pem"));
        }
        let child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let writer = pair.master.take_writer().unwrap();
        // ConPTY requests the terminal's inherited cursor position before letting
        // the child write. This is a terminal protocol response, not application input.
        #[cfg(windows)]
        let writer = {
            let mut writer = writer;
            writer.write_all(b"\x1b[1;1R").unwrap();
            writer.flush().unwrap();
            writer
        };
        let output = Arc::new((Mutex::new(VecDeque::new()), Condvar::new()));
        let captured = output.clone();
        let reader = std::thread::spawn(move || {
            let mut bytes = [0; 4096];
            while let Ok(length) = reader.read(&mut bytes) {
                if length == 0 {
                    break;
                }
                let line = String::from_utf8_lossy(&bytes[..length]).into_owned();
                let (buffer, changed) = &*captured;
                let mut buffer = buffer.lock().unwrap_or_else(|error| error.into_inner());
                if buffer.len() >= 1000 {
                    buffer.pop_front();
                }
                buffer.push_back(line);
                changed.notify_all();
            }
        });
        Self {
            directory: directory.to_owned(),
            mode: mode.to_owned(),
            started,
            child,
            writer: Some(writer),
            master: Some(pair.master),
            output,
            reader: Some(reader),
        }
    }

    fn ready(&mut self) -> Value {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let (buffer, changed) = &*self.output;
            let buffer = buffer.lock().unwrap();
            let output = terminal_text(&buffer.iter().cloned().collect::<String>());
            for line in output.lines() {
                // ConPTY may render a space as CSI cursor movement.
                if let Some((_, rest)) = line.split_once("RUPHISH_READY")
                    && let (Some(start), Some(end)) = (rest.find('{'), rest.find('}'))
                {
                    return serde_json::from_str(&rest[start..=end]).unwrap();
                }
            }
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "Exited before ready: {buffer:?}\n{}",
                self.diagnostics()
            );
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "Readiness deadline: {buffer:?}\n{}",
                self.diagnostics()
            );
            drop(
                changed
                    .wait_timeout(buffer, remaining.min(Duration::from_millis(100)))
                    .unwrap(),
            );
        }
    }

    fn wait(&mut self, success: bool) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                // Child exit can precede ConPTY's final output. Close the terminal
                // and join its reader before inspecting diagnostics or returning.
                self.drain_output();
                assert_eq!(
                    status.success(),
                    success,
                    "Unexpected exit {status:?}: {:?}\n{}",
                    self.output.0.lock().unwrap(),
                    self.diagnostics()
                );
                return;
            }
            assert!(
                Instant::now() < deadline,
                "Process exit deadline: {:?}\n{}",
                self.output.0.lock().unwrap(),
                self.diagnostics()
            );
            let lock = self.output.0.lock().unwrap();
            drop(
                self.output
                    .1
                    .wait_timeout(lock, Duration::from_millis(25))
                    .unwrap(),
            );
        }
    }

    fn stop(&mut self) {
        self.interrupt();
        self.wait(true);
    }

    fn diagnostics(&self) -> String {
        process_diagnostics::snapshot(
            &self.directory,
            self.child.process_id(),
            &self.mode,
            self.started,
        )
    }

    fn drain_output(&mut self) {
        self.writer.take();
        self.master.take();
        if let Some(reader) = self.reader.take() {
            reader.join().unwrap();
        }
    }

    fn interrupt(&mut self) {
        // ConPTY requests win32-input-mode. Encode the actual Ctrl+C key event,
        // as we already do for Ctrl+Break, rather than relying on ETX translation.
        #[cfg(windows)]
        let input = b"\x1b[67;46;3;1;8;1_\x1b[67;46;3;0;8;1_".as_slice();
        #[cfg(unix)]
        let input = &[3];
        self.writer.as_mut().unwrap().write_all(input).unwrap();
        self.writer.as_mut().unwrap().flush().unwrap();
    }

    fn output(&self) -> String {
        terminal_text(
            &self
                .output
                .0
                .lock()
                .unwrap()
                .iter()
                .cloned()
                .collect::<String>(),
        )
    }

    fn fail(&mut self) {
        self.wait(false);
        assert!(!self.output().contains("RUPHISH_READY"));
    }

    fn crash(&mut self) {
        #[cfg(unix)]
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(self.child.process_id().unwrap().try_into().unwrap()),
            nix::sys::signal::Signal::SIGKILL,
        )
        .unwrap();
        #[cfg(windows)]
        self.child.kill().unwrap();
        self.wait(false);
    }
}

// Decode ConPTY's presentation controls for assertions about visible console
// text. Cursor-forward sequences stand for spaces; title/control sequences do not.
fn terminal_text(raw: &str) -> String {
    let mut text = String::new();
    let mut chars = raw.chars();
    while let Some(character) = chars.next() {
        if character != '\x1b' {
            text.push(character);
            continue;
        }
        match chars.next() {
            Some('[') => {
                let mut parameters = String::new();
                for character in chars.by_ref() {
                    if ('@'..='~').contains(&character) {
                        if character == 'C' {
                            text.extend(std::iter::repeat_n(
                                ' ',
                                parameters.parse::<usize>().unwrap_or(1).min(1000),
                            ));
                        }
                        break;
                    }
                    parameters.push(character);
                }
            }
            Some(']') => {
                for character in chars.by_ref() {
                    if character == '\x07' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    text
}

#[test]
fn killing_a_process_during_migration_leaves_the_old_database_recoverable_and_restartable() {
    let directory = workspace_without_assets();
    std::fs::copy(
        fixture("compatibility/20160118194630.sqlite3"),
        directory.path().join("installation.sqlite3"),
    )
    .unwrap();
    database(
        directory.path(),
        "CREATE TRIGGER stall_migration BEFORE INSERT ON goose_db_version BEGIN SELECT count(*) FROM (WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<1000000000) SELECT x FROM n); END",
    );
    let mut process = Process::start(directory.path(), "phish");
    let deadline = Instant::now() + Duration::from_secs(15);
    while backups(directory.path()).is_empty()
        || !std::fs::metadata(directory.path().join("installation.sqlite3-journal"))
            .is_ok_and(|metadata| metadata.len() > 0)
    {
        assert!(
            process.child.try_wait().unwrap().is_none(),
            "{}",
            process.output()
        );
        assert!(
            Instant::now() < deadline,
            "Snapshot was never published: {}",
            process.output()
        );
        let buffer = process.output.0.lock().unwrap();
        drop(
            process
                .output
                .1
                .wait_timeout(buffer, Duration::from_millis(25))
                .unwrap(),
        );
    }
    process.crash();
    assert_eq!(version(directory.path()), 20160118194630);
    database(directory.path(), "DROP TRIGGER stall_migration");
    let mut restarted = Process::start(directory.path(), "phish");
    restarted.ready();
    restarted.stop();
    assert_eq!(
        version(directory.path()),
        ruphish_adapters::database::migrations::LATEST_LEGACY_VERSION
    );
}

impl Drop for Process {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        self.writer.take();
        self.master.take();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

struct Workspace {
    // Drop network threads before removing the directory they serve.
    network: OnceLock<campaign_process::CampaignNetwork>,
    directory: tempfile::TempDir,
}

impl Workspace {
    fn path(&self) -> &Path {
        self.directory.path()
    }
}

impl std::ops::Deref for Workspace {
    type Target = Path;

    fn deref(&self) -> &Path {
        self.path()
    }
}

fn workspace_without_assets() -> Workspace {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let directory = tempfile::Builder::new()
        .prefix("ruphish process é ")
        .tempdir()
        .unwrap();
    write_config(directory.path(), &configuration());
    Workspace {
        network: OnceLock::new(),
        directory,
    }
}

fn workspace() -> Workspace {
    let directory = workspace_without_assets();
    let root = directory.path().join("dist/admin");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("asset-manifest.json"), b"{}").unwrap();
    // These assets exercise the server's real manifest and HTTP handling, not the UI.
    // Tests of the built distribution explicitly use workspace_with_built_assets().
    for (name, bytes) in [
        (
            "index.html",
            b"<!doctype html><title>Ruphish process fixture</title>".as_slice(),
        ),
        (
            "visual-frame.js",
            b"export const fixture = true;".as_slice(),
        ),
        ("visual-frame.css", b"body { margin: 0; }".as_slice()),
        (
            "THIRD-PARTY-NOTICES.txt",
            b"Synthetic process-test assets.".as_slice(),
        ),
        // Valid empty WebAssembly module: magic number followed by version 1.
        ("fixture.wasm", b"\0asm\x01\0\0\0".as_slice()),
    ] {
        update_manifest(directory.path(), name, bytes);
    }
    directory
}

fn workspace_with_built_assets() -> Workspace {
    let directory = workspace_without_assets();
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/admin");
    assert!(
        assets.join("index.html").is_file(),
        "Build the actual admin distribution with trunk build before process tests"
    );
    copy(&assets, &directory.path().join("dist/admin"));
    directory
}

fn copy(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let to = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), to).unwrap();
        }
    }
}
fn configuration() -> Value {
    json!({"db_name":"sqlite3","db_path":"installation.sqlite3","admin_server":{"listen_url":"127.0.0.1:0","allowed_internal_hosts":["127.0.0.1"],"use_tls":false,"cert_path":"admin.crt","key_path":"admin.key"},"phish_server":{"listen_url":"127.0.0.1:0","use_tls":false,"cert_path":"public.crt","key_path":"public.key"},"logging":{"level":"info","filename":"server.log"}})
}
fn write_config(directory: &Path, config: &Value) {
    std::fs::write(
        directory.join("config.json"),
        serde_json::to_vec(config).unwrap(),
    )
    .unwrap();
}
fn client() -> reqwest::blocking::Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap()
}

fn fixture(path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(path)
}

fn database(directory: &Path, sql: &'static str) -> Vec<sqlx::sqlite::SqliteRow> {
    use sqlx::Connection;
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut connection = sqlx::SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(directory.join("installation.sqlite3")),
        )
        .await
        .unwrap();
        let result = sqlx::raw_sql(sql).fetch_all(&mut connection).await.unwrap();
        connection.close().await.unwrap();
        result
    })
}

fn version(directory: &Path) -> i64 {
    use sqlx::Row;
    database(
        directory,
        "SELECT max(version_id) AS version FROM goose_db_version WHERE is_applied=1",
    )[0]
    .get("version")
}

fn backups(directory: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("ruphish-pre-upgrade-")
        })
        .collect()
}

#[test]
fn process_diagnostics_observe_the_child_without_disclosing_workspace_contents() {
    let directory = workspace_without_assets();
    let mut process = Process::start(directory.path(), "phish");
    process.ready();
    let secret = "synthetic-diagnostic-secret-canary";
    std::fs::write(directory.path().join("admin.key"), secret).unwrap();
    let diagnostics = process.diagnostics();
    // OS metrics are best-effort diagnostics, not a process correctness requirement.
    // Keep the observation in CI logs even when the optional command times out.
    eprintln!("{diagnostics}");
    assert!(diagnostics.contains(&format!("pid={:?}", process.child.process_id())));
    assert!(diagnostics.contains("mode=phish"));
    assert!(diagnostics.contains("installation.sqlite3: "));
    assert!(diagnostics.contains(&format!("admin.key: {} bytes", secret.len())));
    for value in [
        secret,
        "synthetic-bootstrap-password",
        "synthetic-bootstrap-token",
    ] {
        assert!(!diagnostics.contains(value));
    }
    process.stop();
}

#[test]
fn actual_http_servers_serve_built_assets_isolate_public_routes_and_shutdown_on_real_ctrl_c() {
    let directory = workspace_with_built_assets();
    let mut process = Process::start(directory.path(), "all");
    let ready = process.ready();
    let admin = ready["admin"].as_str().unwrap();
    let public = ready["phish"].as_str().unwrap();
    let client = client();
    let response = client.get(format!("http://{admin}/")).send().unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.text().unwrap().contains("Ruphish"));
    let asset_root = directory.path().join("dist/admin");
    let manifest: std::collections::BTreeMap<String, String> =
        serde_json::from_slice(&std::fs::read(asset_root.join("asset-manifest.json")).unwrap())
            .unwrap();
    // ES module imports live under snippets/, so testing only root files misses
    // missing modules and incorrect MIME types that prevent WASM from mounting.
    for name in manifest
        .keys()
        .map(String::as_str)
        .chain(["asset-manifest.json"])
    {
        let route = if name == "index.html" { "" } else { name };
        let response = client
            .get(format!("http://{admin}/{route}"))
            .send()
            .unwrap();
        assert_eq!(response.status(), 200, "{name}");
        if name.ends_with(".wasm") {
            assert_eq!(response.headers()["content-type"], "application/wasm");
        }
        if name.ends_with(".js") || name.ends_with(".mjs") {
            assert!(
                response.headers()["content-type"]
                    .to_str()
                    .unwrap()
                    .contains("javascript"),
                "{name}"
            );
        }
        assert_eq!(
            response.bytes().unwrap().as_ref(),
            std::fs::read(asset_root.join(name)).unwrap(),
            "{name}"
        );
    }
    for path in [
        "/api/no-such-resource",
        "/missing.wasm",
        "/.git/config",
        "/config.json",
    ] {
        let response = client.get(format!("http://{admin}{path}")).send().unwrap();
        assert_eq!(response.status(), 404, "{path}");
        assert_eq!(response.text().unwrap(), "404 page not found\n");
    }
    let response = client.get(format!("http://{public}/")).send().unwrap();
    assert_eq!(response.status(), 404);
    process.stop();
    let logs = std::fs::read_to_string(directory.path().join("server.log")).unwrap();
    assert!(logs.contains("shutdown_complete"));
    assert!(!logs.contains("synthetic-bootstrap-password"));
    assert!(!logs.contains("synthetic-bootstrap-token"));
    assert!(std::net::TcpListener::bind(admin).is_ok());
    assert!(std::net::TcpListener::bind(public).is_ok());
}

fn update_manifest(directory: &Path, name: &str, bytes: &[u8]) {
    use sha2::{Digest, Sha256};
    let root = directory.join("dist/admin");
    std::fs::write(root.join(name), bytes).unwrap();
    let path = root.join("asset-manifest.json");
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest[name] = json!(hex::encode(Sha256::digest(bytes)));
    std::fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
}

#[test]
fn graceful_shutdown_finishes_an_in_progress_asset_response_and_bounds_idle_connections() {
    let directory = workspace();
    let content = vec![b'x'; 32 * 1024 * 1024];
    update_manifest(directory.path(), "large.js", &content);
    let mut process = Process::start(directory.path(), "all");
    let ready = process.ready();
    let admin = ready["admin"].as_str().unwrap();
    let mut idle = std::net::TcpStream::connect(admin).unwrap();
    idle.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nX-Incomplete:")
        .unwrap();
    let response = client()
        .get(format!("http://{admin}/large.js"))
        .timeout(Duration::from_secs(14))
        .send()
        .unwrap();
    assert_eq!(response.content_length(), Some(content.len() as u64));
    process.interrupt();
    assert_eq!(response.bytes().unwrap().as_ref(), content);
    process.wait(true);
    assert!(std::net::TcpListener::bind(admin).is_ok());
}

#[test]
fn incomplete_http_headers_and_tls_handshakes_have_real_deadlines() {
    let directory = workspace();
    let mut config = configuration();
    config["phish_server"]["use_tls"] = json!(true);
    write_config(directory.path(), &config);
    let mut process = Process::start(directory.path(), "all");
    let ready = process.ready();
    let mut http = std::net::TcpStream::connect(ready["admin"].as_str().unwrap()).unwrap();
    let mut tls = std::net::TcpStream::connect(ready["phish"].as_str().unwrap()).unwrap();
    for stream in [&http, &tls] {
        stream
            .set_read_timeout(Some(Duration::from_secs(13)))
            .unwrap();
    }
    http.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nX-Incomplete:")
        .unwrap();
    tls.write_all(&[0x16]).unwrap();
    let start = Instant::now();
    let mut buffer = [0; 1024];
    let length = http.read(&mut buffer).unwrap();
    assert!(length == 0 || buffer[..length].starts_with(b"HTTP/1.1 408"));
    match tls.read(&mut buffer) {
        Ok(length) => assert_eq!(length, 0),
        Err(error) => assert!(
            matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
            ),
            "{error}"
        ),
    }
    assert!(start.elapsed() < Duration::from_secs(13));
    process.stop();
}

#[test]
fn slow_anonymous_bodies_are_closed_and_validated_assets_remain_immutable() {
    let directory = workspace_with_built_assets();
    let root = directory.path().join("dist/admin");
    let original = std::fs::read(root.join("visual-frame.js")).unwrap();
    let mut process = Process::start(directory.path(), "admin");
    let ready = process.ready();
    let address = ready["admin"].as_str().unwrap();
    let base = format!("http://{address}");
    std::fs::write(root.join("visual-frame.js"), b"changed-after-startup").unwrap();
    assert_eq!(
        client()
            .get(format!("{base}/visual-frame.js"))
            .send()
            .unwrap()
            .bytes()
            .unwrap()
            .as_ref(),
        original
    );
    let partial = client()
        .get(format!("{base}/visual-frame.js"))
        .header("Range", "bytes=0-7")
        .send()
        .unwrap();
    assert_eq!(partial.status(), 206);
    assert_eq!(partial.bytes().unwrap().as_ref(), &original[..8]);
    let mut socket = std::net::TcpStream::connect(address).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(35)))
        .unwrap();
    socket.write_all(b"POST /api/import/group/review HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{").unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 408"), "{response}");
    assert_eq!(
        client()
            .get(format!("{base}/auth/session"))
            .send()
            .unwrap()
            .status(),
        200
    );
    process.stop();
}

#[test]
fn raw_traversal_requests_cannot_escape_assets_and_modified_distributions_fail_preflight() {
    let directory = workspace_with_built_assets();
    std::fs::write(directory.path().join("private.txt"), "outside-asset-secret").unwrap();
    let mut process = Process::start(directory.path(), "admin");
    let ready = process.ready();
    for path in [
        "/../private.txt",
        "/%2e%2e/private.txt",
        "/%2e%2e%5cprivate.txt",
        "/dist/admin/",
        "/api/",
        "/missing.js",
    ] {
        let mut socket = std::net::TcpStream::connect(ready["admin"].as_str().unwrap()).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        write!(
            socket,
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 404"), "{path}: {response}");
        assert!(!response.contains("outside-asset-secret"));
    }
    process.stop();
    std::fs::write(
        directory.path().join("dist/admin/visual-frame.js"),
        "stale-generation",
    )
    .unwrap();
    let mut stale = Process::start(directory.path(), "admin");
    stale.fail();
    assert!(stale.output().contains("integrity"));
}

#[cfg(unix)]
#[test]
fn asset_symlinks_outside_the_distribution_are_rejected_before_startup() {
    let directory = workspace_with_built_assets();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.js"), "outside-secret").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.js"),
        directory.path().join("dist/admin/escape.js"),
    )
    .unwrap();
    let mut process = Process::start(directory.path(), "admin");
    process.fail();
    assert!(!directory.path().join("installation.sqlite3").exists());
}

#[test]
fn every_populated_goose_version_starts_in_place_with_a_verified_recoverable_snapshot() {
    use sqlx::Row;
    let versions: Vec<i64> =
        serde_json::from_str(include_str!("fixtures/compatibility/versions.json")).unwrap();
    for old in versions {
        let directory = workspace_without_assets();
        let source = fixture(&format!("compatibility/{old}.sqlite3"));
        let original = std::fs::read(&source).unwrap();
        std::fs::copy(&source, directory.path().join("installation.sqlite3")).unwrap();
        let mut process = Process::start(directory.path(), "phish");
        process.ready();
        // Preservation is an offline assertion. Background workers may write after
        // readiness; concurrent access is covered by the dedicated shared-DB tests.
        process.stop();
        assert_eq!(
            version(directory.path()),
            ruphish_adapters::database::migrations::LATEST_LEGACY_VERSION,
            "{old}"
        );
        let rows = database(
            directory.path(),
            "SELECT username,hash,api_key FROM users WHERE id=17",
        );
        assert_eq!(rows[0].get::<String, _>("username"), "historical-admin");
        assert_eq!(rows[0].get::<String, _>("api_key"), "historical-key");
        assert!(ruphish_adapters::password_worker::verify(
            "FixtureOnlyPassword42!",
            &rows[0].get::<String, _>("hash")
        ));
        assert_eq!(
            database(
                directory.path(),
                "SELECT r_id FROM results WHERE id=59 AND campaign_id=53"
            )[0]
            .get::<String, _>("r_id"),
            "preserved-rid"
        );
        let snapshots = backups(directory.path());
        assert_eq!(
            snapshots.len(),
            1, // Every Go schema receives the independently versioned authentication upgrade.
        );
        if let Some(snapshot) = snapshots.first() {
            let restore = tempfile::tempdir().unwrap();
            std::fs::copy(snapshot, restore.path().join("installation.sqlite3")).unwrap();
            assert_eq!(version(restore.path()), old);
            assert_eq!(
                database(restore.path(), "PRAGMA integrity_check")[0].get::<String, _>(0),
                "ok"
            );
            assert_eq!(
                database(restore.path(), "SELECT api_key FROM users WHERE id=17")[0]
                    .get::<String, _>(0),
                "historical-key"
            );
        }
        let mut restart = Process::start(directory.path(), "phish");
        restart.ready();
        restart.stop();
        assert_eq!(backups(directory.path()).len(), snapshots.len());
        assert_eq!(std::fs::read(source).unwrap(), original);
    }
}

#[test]
fn bootstrap_is_atomic_refreshes_only_temporary_credentials_and_preserves_established_accounts() {
    use sqlx::Row;
    let directory = workspace();
    let mut first = Process::start(directory.path(), "admin");
    first.ready();
    first.stop();
    let row = database(
        directory.path(),
        "SELECT users.*,roles.slug FROM users JOIN roles ON role_id=roles.id",
    );
    assert_eq!(row.len(), 1);
    assert_eq!(row[0].get::<String, _>("slug"), "admin");
    assert!(row[0].get::<bool, _>("password_change_required"));
    assert!(ruphish_adapters::password_worker::verify(
        "synthetic-bootstrap-password",
        &row[0].get::<String, _>("hash")
    ));
    let mut refresh = Process::with_credentials(
        directory.path(),
        "admin",
        Some("new-temporary-password"),
        Some("must-not-replace-token"),
    );
    refresh.ready();
    refresh.stop();
    let row = database(directory.path(), "SELECT hash,api_key FROM users");
    let hash: String = row[0].get("hash");
    assert!(ruphish_adapters::password_worker::verify(
        "new-temporary-password",
        &hash
    ));
    assert_eq!(
        row[0].get::<String, _>("api_key"),
        "synthetic-bootstrap-token"
    );
    database(
        directory.path(),
        "UPDATE users SET password_change_required=0,username='renamed-admin'",
    );
    let mut established = Process::start(directory.path(), "admin");
    established.ready();
    established.stop();
    assert_eq!(
        database(directory.path(), "SELECT hash FROM users")[0].get::<String, _>(0),
        hash
    );
    assert!(!established.output().contains("Temporary password:"));
    assert_eq!(
        database(directory.path(), "SELECT count(*) FROM users")[0].get::<i64, _>(0),
        1
    );
}

#[test]
fn failed_bootstrap_rolls_back_then_forced_process_death_allows_clean_restart() {
    use sqlx::Row;
    let directory = workspace();
    let mut rejected =
        Process::with_credentials(directory.path(), "phish", Some(&"x".repeat(1025)), None);
    rejected.fail();
    assert_eq!(
        database(directory.path(), "SELECT count(*) FROM users")[0].get::<i64, _>(0),
        0
    );
    let mut generated = Process::with_credentials(directory.path(), "phish", None, None);
    generated.ready();
    let output = generated.output();
    assert!(!output.contains("Temporary password:"));
    assert!(output.contains("GOPHISH_INITIAL_ADMIN_PASSWORD"));
    let row = database(directory.path(), "SELECT hash,api_key FROM users");
    assert!(row[0].get::<String, _>("hash").starts_with("$argon2id$"));
    assert_eq!(row[0].get::<String, _>("api_key").len(), 64);
    database(
        directory.path(),
        "UPDATE users SET password_change_required=0",
    );
    generated.crash();
    let mut restarted = Process::start(directory.path(), "all");
    restarted.ready();
    restarted.stop();
    assert_eq!(
        database(directory.path(), "PRAGMA integrity_check")[0].get::<String, _>(0),
        "ok"
    );
    assert_eq!(
        database(directory.path(), "SELECT hash FROM users")[0].get::<String, _>(0),
        row[0].get::<String, _>("hash")
    );
    assert!(
        !std::fs::read_to_string(directory.path().join("server.log"))
            .unwrap()
            .contains("Temporary password:")
    );
}

#[test]
fn migration_failures_roll_back_and_retain_a_snapshot_while_invalid_databases_are_never_reinitialized()
 {
    use sqlx::Row;
    for mutation in [
        "UPDATE goose_db_version SET version_id=99999999999999 WHERE version_id=(SELECT max(version_id) FROM goose_db_version)",
        "ALTER TABLE templates DROP COLUMN subject",
        "CREATE TRIGGER fail_upgrade BEFORE INSERT ON goose_db_version BEGIN SELECT RAISE(ABORT,'injected migration failure'); END",
    ] {
        let directory = workspace();
        std::fs::copy(
            fixture("compatibility/20160118194630.sqlite3"),
            directory.path().join("installation.sqlite3"),
        )
        .unwrap();
        database(directory.path(), mutation);
        let before = std::fs::read(directory.path().join("installation.sqlite3")).unwrap();
        let mut rejected = Process::start(directory.path(), "phish");
        rejected.fail();
        assert_eq!(
            std::fs::read(directory.path().join("installation.sqlite3")).unwrap(),
            before
        );
        assert_eq!(
            database(directory.path(), "SELECT api_key FROM users WHERE id=17")[0]
                .get::<String, _>(0),
            "historical-key"
        );
        if mutation.starts_with("CREATE TRIGGER") {
            assert_eq!(backups(directory.path()).len(), 1);
            database(directory.path(), "DROP TRIGGER fail_upgrade");
            let mut fixed = Process::start(directory.path(), "phish");
            fixed.ready();
            fixed.stop();
            assert_eq!(
                version(directory.path()),
                ruphish_adapters::database::migrations::LATEST_LEGACY_VERSION
            );
        }
    }
    let directory = workspace();
    std::fs::write(
        directory.path().join("installation.sqlite3"),
        b"corrupt synthetic sqlite",
    )
    .unwrap();
    let mut corrupt = Process::start(directory.path(), "phish");
    corrupt.fail();
    assert_eq!(
        std::fs::read(directory.path().join("installation.sqlite3")).unwrap(),
        b"corrupt synthetic sqlite"
    );
}

#[test]
fn mode_selection_and_short_startup_locks_allow_separate_servers_to_share_one_database() {
    let directory = workspace();
    let mut admin = Process::start(directory.path(), "admin");
    let a = admin.ready();
    assert!(a.get("phish").is_none());
    let mut phish = Process::start(directory.path(), "phish");
    let p = phish.ready();
    assert!(p.get("admin").is_none());
    assert!(
        client()
            .get(format!("http://{}/", a["admin"].as_str().unwrap()))
            .send()
            .unwrap()
            .status()
            .is_success()
    );
    assert_eq!(
        client()
            .get(format!("http://{}/", p["phish"].as_str().unwrap()))
            .send()
            .unwrap()
            .status(),
        404
    );
    phish.stop();
    admin.stop();
}

#[test]
fn preflight_failures_never_publish_readiness_or_create_a_database_and_release_reserved_ports() {
    for case in [
        "second-port",
        "assets",
        "logging",
        "mysql",
        "postgres-missing-settings",
        "postgres-conflicting-settings",
        "postgres-password-sources",
        "postgres-unknown-field",
        "postgres-ca-without-tls",
        "custom-migrations",
        "domain-ca",
        "public-asset-size",
    ] {
        let directory = workspace();
        let mut config = configuration();
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = reserved.local_addr().unwrap();
        // Keep the admin port reserved until the conflicting listener has its own port.
        let occupied = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        config["admin_server"]["listen_url"] = json!(port.to_string());
        match case {
            "second-port" => {
                config["phish_server"]["listen_url"] =
                    json!(occupied.local_addr().unwrap().to_string())
            }
            "assets" => {
                std::fs::remove_file(directory.path().join("dist/admin/index.html")).unwrap()
            }
            "logging" => config["logging"]["filename"] = json!("dist"),
            "mysql" => config["db_name"] = json!("mysql"),
            name if name.starts_with("postgres-") => {
                config["db_name"] = json!("postgres");
                config["db_path"] = json!("");
                config["postgres"] = json!({"host":"127.0.0.1","database":"fixture","username":"fixture","password":"private-config-canary"});
                match name {
                    "postgres-missing-settings" => {
                        config.as_object_mut().unwrap().remove("postgres");
                    }
                    "postgres-conflicting-settings" => {
                        config["db_path"] = json!("installation.sqlite3")
                    }
                    "postgres-password-sources" => {
                        config["postgres"]["password_env"] = json!("PRIVATE_FIXTURE_PASSWORD")
                    }
                    "postgres-unknown-field" => config["postgres"]["sslmode"] = json!("disable"),
                    "postgres-ca-without-tls" => {
                        config["postgres"]["tls"] = json!("disabled");
                        config["postgres"]["ca_path"] = json!("unused.pem");
                    }
                    _ => unreachable!(),
                }
            }
            "custom-migrations" => config["migrations_prefix"] = json!("missing-custom-"),
            "domain-ca" => {
                config["domain_verification"] = json!({"trusted_ca_path":"missing-domain-ca.pem"})
            }
            "public-asset-size" => {
                let root = directory.path().join("static/endpoint");
                std::fs::create_dir_all(&root).unwrap();
                std::fs::File::create(root.join("large.bin"))
                    .unwrap()
                    .set_len(8 * 1024 * 1024 + 1)
                    .unwrap();
            }
            _ => unreachable!(),
        }
        write_config(directory.path(), &config);
        drop(reserved);
        let mut process = Process::start(directory.path(), "all");
        process.fail();
        assert!(
            !directory.path().join("installation.sqlite3").exists(),
            "{case}"
        );
        let rebound = std::net::TcpListener::bind(port);
        assert!(
            rebound.is_ok(),
            "{case}: could not rebind {port}: {rebound:?}"
        );
    }
}

#[test]
fn generated_tls_is_trusted_with_its_certificate_and_preserved_across_restart() {
    let directory = workspace();
    let mut config = configuration();
    config["admin_server"]["use_tls"] = json!(true);
    config["phish_server"]["use_tls"] = json!(true);
    write_config(directory.path(), &config);
    let mut process = Process::start(directory.path(), "all");
    let ready = process.ready();
    for (name, file, status) in [("admin", "admin.crt", 200), ("phish", "public.crt", 404)] {
        let certificate =
            reqwest::Certificate::from_pem(&std::fs::read(directory.path().join(file)).unwrap())
                .unwrap();
        let trusted = reqwest::blocking::Client::builder()
            .no_proxy()
            .add_root_certificate(certificate)
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        let url = format!("https://{}/", ready[name].as_str().unwrap());
        assert!(client().get(&url).send().is_err());
        assert_eq!(trusted.get(url).send().unwrap().status().as_u16(), status);
    }
    process.stop();
    let key = std::fs::read(directory.path().join("admin.key")).unwrap();
    let cert = std::fs::read(directory.path().join("admin.crt")).unwrap();
    let mut restarted = Process::start(directory.path(), "all");
    restarted.ready();
    restarted.stop();
    assert_eq!(
        std::fs::read(directory.path().join("admin.key")).unwrap(),
        key
    );
    assert_eq!(
        std::fs::read(directory.path().join("admin.crt")).unwrap(),
        cert
    );
}

#[test]
fn legacy_go_sec1_certificates_work_and_incomplete_or_mismatched_pairs_are_not_overwritten() {
    let directory = workspace();
    let mut config = configuration();
    config["admin_server"]["use_tls"] = json!(true);
    write_config(directory.path(), &config);
    let cert = std::fs::read(fixture("startup/go-admin.crt")).unwrap();
    let key = std::fs::read(fixture("startup/go-admin.key")).unwrap();
    std::fs::write(directory.path().join("admin.crt"), &cert).unwrap();
    let mut incomplete = Process::start(directory.path(), "admin");
    incomplete.fail();
    assert!(!directory.path().join("admin.key").exists());
    assert!(!directory.path().join("installation.sqlite3").exists());
    std::fs::write(directory.path().join("admin.key"), &key).unwrap();
    let mut legacy = Process::start(directory.path(), "admin");
    let ready = legacy.ready();
    // Legacy generated certificates have no SAN. This one synthetic compatibility
    // request intentionally skips identity validation; modern generated TLS above does not.
    let legacy_client = reqwest::blocking::Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    assert_eq!(
        legacy_client
            .get(format!("https://{}/", ready["admin"].as_str().unwrap()))
            .send()
            .unwrap()
            .status(),
        200
    );
    legacy.stop();
    assert_eq!(
        std::fs::read(directory.path().join("admin.crt")).unwrap(),
        cert
    );
    assert_eq!(
        std::fs::read(directory.path().join("admin.key")).unwrap(),
        key
    );
    let other = tempfile::tempdir().unwrap();
    ruphish_adapters::tls::load_or_generate(
        &other.path().join("c"),
        &other.path().join("k"),
        "localhost",
    )
    .unwrap();
    let mismatched = std::fs::read(other.path().join("k")).unwrap();
    std::fs::write(directory.path().join("admin.key"), &mismatched).unwrap();
    let mut rejected = Process::start(directory.path(), "admin");
    rejected.fail();
    assert_eq!(
        std::fs::read(directory.path().join("admin.key")).unwrap(),
        mismatched
    );
    assert_eq!(
        std::fs::read(directory.path().join("admin.crt")).unwrap(),
        cert
    );
}

#[test]
fn log_levels_append_and_never_record_query_credentials() {
    for level in [
        "", "trace", "debug", "info", "warn", "warning", "error", "fatal", "panic", "INFO",
    ] {
        let directory = workspace();
        let mut config = configuration();
        config["logging"]["level"] = json!(level);
        write_config(directory.path(), &config);
        std::fs::write(directory.path().join("server.log"), "existing-log-marker\n").unwrap();
        let mut process = Process::start(directory.path(), "phish");
        let ready = process.ready();
        client()
            .get(format!(
                "http://{}/missing?api_key=synthetic-query-secret",
                ready["phish"].as_str().unwrap()
            ))
            .send()
            .unwrap();
        process.stop();
        let logs = std::fs::read_to_string(directory.path().join("server.log")).unwrap();
        assert!(logs.starts_with("existing-log-marker\n"));
        assert!(!logs.contains("synthetic-query-secret"));
        assert_eq!(
            logs.contains("http_request"),
            matches!(level, "" | "trace" | "debug" | "info" | "INFO"),
            "{level}"
        );
        assert!(!logs.contains("synthetic-bootstrap"));
    }
    let directory = workspace();
    let mut config = configuration();
    config["logging"]["level"] = json!("invalid");
    write_config(directory.path(), &config);
    let mut process = Process::start(directory.path(), "phish");
    process.fail();
    assert!(!directory.path().join("installation.sqlite3").exists());
}

#[cfg(unix)]
#[test]
fn unix_sigterm_uses_the_same_graceful_shutdown_path() {
    use nix::{
        sys::signal::{Signal, kill},
        unistd::Pid,
    };
    let directory = workspace();
    let mut process = Process::start(directory.path(), "all");
    process.ready();
    kill(
        Pid::from_raw(process.child.process_id().unwrap().try_into().unwrap()),
        Signal::SIGTERM,
    )
    .unwrap();
    process.wait(true);
    assert!(
        std::fs::read_to_string(directory.path().join("server.log"))
            .unwrap()
            .contains("shutdown_complete")
    );
}

#[test]
fn custom_config_paths_mailer_switches_and_inactive_listener_settings_keep_their_cli_semantics() {
    for (options, requested) in [
        (vec!["--disable-mailer"], false),
        (vec!["--no-disable-mailer"], true),
    ] {
        let directory = workspace();
        let mut config = configuration();
        config["phish_server"]["listen_url"] = json!("inactive-invalid-address");
        config["phish_server"]["use_tls"] = json!(true);
        config["phish_server"]["cert_path"] = json!("");
        std::fs::create_dir(directory.path().join("settings")).unwrap();
        std::fs::remove_file(directory.path().join("config.json")).unwrap();
        std::fs::write(
            directory.path().join("settings/alternate config.json"),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        let mut args = vec!["--config", "settings/alternate config.json"];
        args.extend(options);
        let mut process = Process::with_options(
            directory.path(),
            "admin",
            Some("synthetic-bootstrap-password"),
            None,
            &args,
        );
        let ready = process.ready();
        process.stop();
        assert!(ready.get("phish").is_none());
        assert!(directory.path().join("installation.sqlite3").exists());
        assert!(
            !directory
                .path()
                .join("settings/installation.sqlite3")
                .exists()
        );
        assert!(
            std::fs::read_to_string(directory.path().join("server.log"))
                .unwrap()
                .contains(&format!("mailer_requested={requested}"))
        );
    }
}

#[test]
fn wildcard_listeners_support_ipv4_and_ipv6_on_hosts_that_offer_dual_stack() {
    let directory = workspace();
    let mut config = configuration();
    config["phish_server"]["listen_url"] = json!(":0");
    write_config(directory.path(), &config);
    let ipv6 = std::net::TcpListener::bind("[::1]:0").is_ok();
    let mut process = Process::start(directory.path(), "phish");
    let ready = process.ready();
    let address: std::net::SocketAddr = ready["phish"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        client()
            .get(format!("http://127.0.0.1:{}/", address.port()))
            .send()
            .unwrap()
            .status(),
        404
    );
    if ipv6 {
        assert!(address.is_ipv6());
        assert_eq!(
            client()
                .get(format!("http://[::1]:{}/", address.port()))
                .send()
                .unwrap()
                .status(),
            404
        );
    }
    process.stop();
}

#[test]
fn existing_go_rsa_pkcs1_material_serves_verified_tls_without_rewriting_files() {
    let directory = workspace();
    let mut config = configuration();
    config["admin_server"]["use_tls"] = json!(true);
    write_config(directory.path(), &config);
    let cert = std::fs::read(fixture("startup/go-rsa.crt")).unwrap();
    let key = std::fs::read(fixture("startup/go-rsa.key")).unwrap();
    std::fs::write(directory.path().join("admin.crt"), &cert).unwrap();
    std::fs::write(directory.path().join("admin.key"), &key).unwrap();
    let mut process = Process::start(directory.path(), "admin");
    let ready = process.ready();
    let trusted = reqwest::blocking::Client::builder()
        .no_proxy()
        .tls_certs_only([reqwest::Certificate::from_pem(&cert).unwrap()])
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    assert_eq!(
        trusted
            .get(format!("https://{}/", ready["admin"].as_str().unwrap()))
            .send()
            .unwrap()
            .status(),
        200
    );
    process.stop();
    assert_eq!(
        std::fs::read(directory.path().join("admin.crt")).unwrap(),
        cert
    );
    assert_eq!(
        std::fs::read(directory.path().join("admin.key")).unwrap(),
        key
    );
}

#[cfg(windows)]
#[test]
fn windows_ctrl_break_uses_the_same_graceful_shutdown_path() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "all");
    process.ready();
    // ConPTY win32-input-mode: VK_CANCEL, scan 70, key down, LEFT_CTRL.
    // https://github.com/microsoft/terminal/blob/main/src/host/input.cpp
    process
        .writer
        .as_mut()
        .unwrap()
        .write_all(b"\x1b[3;70;0;1;8;1_")
        .unwrap();
    process.writer.as_mut().unwrap().flush().unwrap();
    process.wait(true);
    assert!(
        std::fs::read_to_string(directory.path().join("server.log"))
            .unwrap()
            .contains("shutdown_complete")
    );
}
