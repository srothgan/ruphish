//! Health checks through the actual executable and configured listeners.
use super::*;

pub(super) fn assert_draining(process: &mut Process, admin: &str) {
    process.wait_output("shutdown_started");
    // Listener closure and a not-ready response are both legitimate during drain.
    if let Ok(response) = client().get(format!("http://{admin}/health/ready")).send() {
        assert_eq!(response.status(), 503);
        assert_eq!(
            response.json::<Value>().unwrap(),
            json!({"status":"not_ready"})
        );
    }
}

fn probe(client: &reqwest::blocking::Client, base: &str, ready: bool) {
    for (path, expected, body) in [
        ("live", 200, "live"),
        (
            "ready",
            if ready { 200 } else { 503 },
            if ready { "ready" } else { "not_ready" },
        ),
    ] {
        let response = client
            .get(format!("{base}/health/{path}?rid=health-secret-canary"))
            .header("cookie", "ruphish.sid=invalid; ruphish.csrf=invalid")
            .header("authorization", "Bearer invalid")
            .send()
            .unwrap();
        assert_eq!(response.status(), expected);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(!response.headers().contains_key("set-cookie"));
        assert_eq!(response.json::<Value>().unwrap(), json!({"status":body}));
    }
}

#[test]
fn health_all_listener_modes_work_without_credentials_or_mailer() {
    for mode in ["all", "admin", "phish"] {
        let directory = workspace();
        let mut process = Process::with_options(
            directory.path(),
            mode,
            Some("synthetic-bootstrap-password"),
            Some("synthetic-bootstrap-token"),
            &["--disable-mailer"],
        );
        let addresses = process.ready();
        let client = client();
        for address in addresses.as_object().unwrap().values() {
            let base = format!("http://{}", address.as_str().unwrap());
            probe(&client, &base, true);
            for path in ["/health/live", "/health/ready"] {
                assert_eq!(
                    client
                        .head(format!("{base}{path}"))
                        .send()
                        .unwrap()
                        .status(),
                    200
                );
                assert_eq!(
                    client
                        .post(format!("{base}{path}"))
                        .body("participant-secret-canary")
                        .send()
                        .unwrap()
                        .status(),
                    405
                );
            }
            for path in ["/health", "/api/health", "/health/ready/"] {
                assert_eq!(
                    client.get(format!("{base}{path}")).send().unwrap().status(),
                    404
                );
            }
        }
        process.interrupt();
        process.wait(true);
        let logs = std::fs::read_to_string(directory.path().join("server.log")).unwrap();
        assert!(logs.contains("shutdown_complete"));
        assert!(!logs.contains("health-secret-canary"));
        assert!(!logs.contains("participant-secret-canary"));
    }
}

#[test]
fn health_sqlite_failure_recovers_without_restart() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "all");
    let addresses = process.ready();
    let client = client();
    let bases: Vec<_> = addresses
        .as_object()
        .unwrap()
        .values()
        .map(|a| format!("http://{}", a.as_str().unwrap()))
        .collect();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        use sqlx::Connection;
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(directory.path().join("installation.sqlite3"));
        let mut inspect = sqlx::SqliteConnection::connect_with(&options)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE ruphish_settings RENAME TO unavailable_settings")
            .execute(&mut inspect)
            .await
            .unwrap();
    });
    for base in &bases {
        probe(&client, base, false);
    }
    runtime.block_on(async {
        use sqlx::Connection;
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(directory.path().join("installation.sqlite3"));
        let mut inspect = sqlx::SqliteConnection::connect_with(&options)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE unavailable_settings RENAME TO ruphish_settings")
            .execute(&mut inspect)
            .await
            .unwrap();
    });
    for base in &bases {
        probe(&client, base, true);
    }
    process.interrupt();
    process.wait(true);
}

pub(super) fn server_outage(mut config: Value) {
    let provider = config["db_name"].as_str().unwrap().to_owned();
    let network = database_network::Network::new(
        (
            [127, 0, 0, 1],
            config[&provider]["port"].as_u64().unwrap() as u16,
        )
            .into(),
    );
    config[&provider]["port"] = json!(network.address.port());
    let directory = workspace();
    let mut settings = configuration();
    for (key, value) in config.as_object().unwrap() {
        settings[key] = value.clone();
    }
    write_config(directory.path(), &settings);
    let mut process = Process::start(directory.path(), "all");
    let addresses = process.ready();
    let client = client();
    let bases: Vec<_> = addresses
        .as_object()
        .unwrap()
        .values()
        .map(|a| format!("http://{}", a.as_str().unwrap()))
        .collect();
    for base in &bases {
        probe(&client, base, true);
    }
    network.available(false);
    for base in &bases {
        probe(&client, base, false);
    }
    network.available(true);
    for base in &bases {
        while client
            .get(format!("{base}/health/ready"))
            .send()
            .unwrap()
            .status()
            != 200
        {
            std::thread::yield_now();
        }
        probe(&client, base, true);
    }
    process.interrupt();
    process.wait(true);
}
