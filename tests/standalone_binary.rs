//! Run the actual standalone executable outside the checkout with isolated state.
use super::*;
use sha2::{Digest, Sha256};

fn distribution() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/admin")
}

pub(super) fn wasm() -> (String, Vec<u8>) {
    let manifest: std::collections::BTreeMap<String, String> =
        serde_json::from_slice(&std::fs::read(distribution().join("asset-manifest.json")).unwrap())
            .unwrap();
    let name = manifest
        .keys()
        .find(|name| name.ends_with(".wasm"))
        .unwrap()
        .clone();
    let bytes = std::fs::read(distribution().join(&name)).unwrap();
    (name, bytes)
}

#[test]
#[ignore = "Requires RUPHISH_TEST_BINARY pointing to the exact standalone executable to verify"]
fn copied_binary_serves_verified_embedded_ui_and_preserves_state_after_restart() {
    let directory = workspace_without_assets();
    let executable = directory.path().join(if cfg!(windows) {
        "ruphish.exe"
    } else {
        "ruphish"
    });
    let source = PathBuf::from(
        std::env::var_os("RUPHISH_TEST_BINARY")
            .ok_or("Set RUPHISH_TEST_BINARY to the standalone executable")
            .unwrap(),
    );
    assert!(source.is_absolute(), "Use an absolute artifact path");
    std::fs::copy(&source, &executable).unwrap();
    assert_eq!(
        Sha256::digest(std::fs::read(&source).unwrap()),
        Sha256::digest(std::fs::read(&executable).unwrap())
    );
    let version = std::process::Command::new(&executable)
        .arg("--version")
        .current_dir(directory.path())
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(version.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&version.stderr).trim(),
        format!("Ruphish {}", env!("CARGO_PKG_VERSION"))
    );
    assert!(!directory.path().join("dist").exists());
    let start = || {
        Process::with_executable(
            directory.path(),
            "all",
            Some("synthetic-bootstrap-password"),
            Some("synthetic-bootstrap-token"),
            &["--disable-mailer"],
            &executable,
        )
    };
    let mut process = start();
    let ready = process.ready();
    let base = format!("http://{}", ready["admin"].as_str().unwrap());
    let browser = auth_process::browser();
    let user = auth_process::setup(&browser, &base)["user"].clone();
    assert!(user["id"].is_number());
    let http = client();
    let expected_manifest = std::fs::read(distribution().join("asset-manifest.json")).unwrap();
    let manifest: std::collections::BTreeMap<String, String> =
        serde_json::from_slice(&expected_manifest).unwrap();
    let response = http
        .get(format!("{base}/asset-manifest.json"))
        .send()
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.bytes().unwrap().as_ref(), expected_manifest);
    for (name, digest) in &manifest {
        let route = if name == "index.html" { "login" } else { name };
        let response = http
            .get(format!("{base}/{route}"))
            .header("Accept-Encoding", "identity")
            .send()
            .unwrap();
        assert_eq!(response.status(), 200, "{name}");
        assert_eq!(
            hex::encode(Sha256::digest(response.bytes().unwrap())),
            *digest,
            "{name}"
        );
    }
    let (name, expected) = wasm();
    let url = format!("{base}/{name}");
    for encoding in ["gzip", "br"] {
        let started = Instant::now();
        let response = http
            .get(&url)
            .header("Accept-Encoding", format!("{encoding}, identity;q=0"))
            .send()
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["content-encoding"], encoding);
        assert_eq!(response.headers()["content-type"], "application/wasm");
        assert_eq!(
            response.headers()["cache-control"],
            "public, max-age=31536000, immutable"
        );
        let etag = response.headers()["etag"].clone();
        let transferred = response.bytes().unwrap();
        println!(
            "Standalone WASM {encoding}: identity={} bytes, transferred={} bytes, full body={:?}",
            expected.len(),
            transferred.len(),
            started.elapsed()
        );
        let mut decoded = Vec::new();
        if encoding == "gzip" {
            flate2::read::GzDecoder::new(transferred.as_ref())
                .read_to_end(&mut decoded)
                .unwrap();
        } else {
            brotli::Decompressor::new(transferred.as_ref(), 4096)
                .read_to_end(&mut decoded)
                .unwrap();
        }
        assert_eq!(decoded, expected);
        let repeated = http
            .get(&url)
            .header("Accept-Encoding", encoding)
            .send()
            .unwrap();
        assert_eq!(repeated.headers()["etag"], etag);
        assert_eq!(repeated.bytes().unwrap(), transferred);
        let conditional = http
            .get(&url)
            .header("Accept-Encoding", encoding)
            .header("If-None-Match", etag.clone())
            .send()
            .unwrap();
        assert_eq!(conditional.status(), 304);
        assert_eq!(conditional.headers()["vary"], "Accept-Encoding");
        assert!(conditional.bytes().unwrap().is_empty());
        let range = http
            .get(&url)
            .header("Accept-Encoding", encoding)
            .header("Range", "bytes=0-15")
            .header("If-Range", etag)
            .send()
            .unwrap();
        assert_eq!(range.status(), 206);
        assert_eq!(range.bytes().unwrap().as_ref(), &transferred[..16]);
        let head = http
            .head(&url)
            .header("Accept-Encoding", encoding)
            .send()
            .unwrap();
        assert_eq!(head.status(), 200);
        assert_eq!(
            head.headers()["content-length"]
                .to_str()
                .unwrap()
                .parse::<usize>()
                .unwrap(),
            transferred.len()
        );
        assert!(head.bytes().unwrap().is_empty());
        let page = browser
            .get(format!("{base}/"))
            .header("Accept-Encoding", format!("{encoding}, identity;q=0"))
            .send()
            .unwrap();
        assert_eq!(page.status(), 200);
        assert_eq!(page.headers()["content-encoding"], encoding);
        assert_eq!(page.headers()["cache-control"], "no-store");
    }
    assert_eq!(
        http.get(&url)
            .header("Accept-Encoding", "*;q=0")
            .send()
            .unwrap()
            .status(),
        406
    );
    assert_eq!(
        http.get(format!(
            "http://{}/{name}",
            ready["phish"].as_str().unwrap()
        ))
        .send()
        .unwrap()
        .status(),
        404
    );
    process.stop();
    assert!(
        std::fs::read_to_string(directory.path().join("server.log"))
            .unwrap()
            .contains("shutdown_complete")
    );
    assert!(!directory.path().join("dist").exists());
    // A corrupt loose distribution must never override the embedded application.
    std::fs::create_dir_all(directory.path().join("dist/admin")).unwrap();
    std::fs::write(
        directory.path().join("dist/admin/index.html"),
        b"untrusted replacement",
    )
    .unwrap();
    let mut restarted = start();
    let base = format!("http://{}", restarted.ready()["admin"].as_str().unwrap());
    let browser = auth_process::browser();
    assert_eq!(
        auth_process::sign_in(&browser, &base, "admin", "established-admin-password").status(),
        200
    );
    assert_eq!(auth_process::bootstrap(&browser, &base)["user"], user);
    assert!(!restarted.output().contains("Temporary password:"));
    assert_eq!(
        http.get(format!("{base}/{name}"))
            .send()
            .unwrap()
            .bytes()
            .unwrap()
            .as_ref(),
        expected
    );
    restarted.stop();
}
