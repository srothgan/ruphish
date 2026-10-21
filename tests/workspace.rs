//! Structural guarantees for the seven product crate boundaries.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// Authoritative allowed direct workspace edges, including dev/build/target sections.
const BOUNDARIES: &[(&str, &[&str])] = &[
    ("ruphish-domain", &[]),
    ("ruphish-contracts", &["ruphish-domain"]),
    ("ruphish-application", &["ruphish-domain"]),
    (
        "ruphish-adapters",
        &["ruphish-domain", "ruphish-application"],
    ),
    (
        "ruphish-http",
        &["ruphish-application", "ruphish-contracts"],
    ),
    ("ruphish-ui", &["ruphish-contracts", "ruphish-domain"]),
    (
        "ruphish",
        &[
            "ruphish-domain",
            "ruphish-contracts",
            "ruphish-application",
            "ruphish-adapters",
            "ruphish-http",
        ],
    ),
];

const DATABASE_DRIVERS: &[&str] = &[
    "sqlx",
    "sqlx-core",
    "sqlx-sqlite",
    "sqlx-mysql",
    "sqlx-postgres",
    "rusqlite",
    "mysql",
    "mysql_async",
    "postgres",
    "tokio-postgres",
    "diesel",
    "sea-orm",
];

fn product_root() -> Result<PathBuf, std::io::Error> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
}

fn metadata() -> Result<Value, Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO"))
        .current_dir(product_root()?)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--locked",
            "--offline",
        ])
        .output()?;
    assert!(
        output.status.success(),
        "Cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

#[test]
fn workspace_dependencies_follow_boundaries_and_stay_inside_product() {
    let metadata = metadata().expect("workspace metadata must load");
    let root = product_root().expect("product root exists");
    let policy: BTreeMap<_, _> = BOUNDARIES.iter().copied().collect();
    let packages = metadata["packages"].as_array().expect("workspace packages");
    let names: BTreeSet<_> = packages
        .iter()
        .map(|p| p["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        policy.keys().copied().collect(),
        "new boundaries require an explicit policy decision"
    );
    for package in packages {
        let name = package["name"].as_str().expect("package name");
        let allowed = policy[name];
        for target in package["targets"].as_array().expect("targets") {
            let path = Path::new(target["src_path"].as_str().expect("target source"));
            assert!(
                path.canonicalize()
                    .expect("target exists")
                    .starts_with(&root),
                "{name} source escapes product: {path:?}"
            );
        }
        for dependency in package["dependencies"].as_array().expect("dependencies") {
            let dependency_name = dependency["name"].as_str().expect("dependency name");
            if policy.contains_key(dependency_name) {
                assert!(
                    allowed.contains(&dependency_name),
                    "forbidden dependency: {name} -> {dependency_name}"
                );
            }
            if let Some(path) = dependency["path"].as_str() {
                assert!(
                    Path::new(path)
                        .canonicalize()
                        .expect("local dependency exists")
                        .starts_with(&root),
                    "{name} depends on a parent/external source path: {path}"
                );
            }
            // Production consumers cannot acquire a driver or bypass the facade.
            // Dev dependencies are reserved for independent fixtures/fault injection.
            if name != "ruphish-adapters" && dependency["kind"] != "dev" {
                assert!(
                    !DATABASE_DRIVERS.contains(&dependency_name),
                    "database driver leaked into {name}: {dependency_name}"
                );
            }
            if matches!(
                name,
                "ruphish-domain" | "ruphish-contracts" | "ruphish-application" | "ruphish-ui"
            ) {
                assert!(
                    !DATABASE_DRIVERS.contains(&dependency_name)
                        && !["axum", "hyper", "lettre", "async-imap"].contains(&dependency_name),
                    "backend dependency leaked into {name}: {dependency_name}"
                );
            }
        }
    }
    let ui = packages
        .iter()
        .find(|p| p["name"] == "ruphish-ui")
        .expect("UI member");
    let ui_id = ui["id"].as_str().expect("UI ID");
    assert!(
        !metadata["workspace_default_members"]
            .as_array()
            .expect("default members")
            .iter()
            .any(|id| id == ui_id),
        "browser binary must not silently pass native-only checks"
    );
}

#[test]
fn member_manifests_inherit_supported_toolchain_and_production_lints() {
    let metadata = metadata().expect("workspace metadata must load");
    let root = product_root().expect("product root exists");
    let toolchain: toml::Value = toml::from_str(
        &fs::read_to_string(root.join("rust-toolchain.toml")).expect("toolchain file"),
    )
    .expect("valid toolchain TOML");
    let channel = toolchain["toolchain"]["channel"]
        .as_str()
        .expect("exact toolchain");
    let version_parts: Vec<_> = channel.split('.').collect();
    assert_eq!(
        version_parts.len(),
        3,
        "use a patch-pinned toolchain rather than floating stable"
    );
    assert!(
        version_parts.iter().all(|p| p.parse::<u32>().is_ok()),
        "stable numeric toolchain required"
    );
    let supported = format!("{}.{}", version_parts[0], version_parts[1]);
    for package in metadata["packages"].as_array().expect("packages") {
        let name = package["name"].as_str().expect("package name");
        assert_eq!(
            package["rust_version"], supported,
            "{name} supported compiler must match the tested toolchain series"
        );
        let manifest: toml::Value = toml::from_str(
            &fs::read_to_string(package["manifest_path"].as_str().expect("manifest path"))
                .expect("manifest exists"),
        )
        .expect("valid manifest TOML");
        assert_eq!(
            manifest["lints"]["workspace"].as_bool(),
            Some(true),
            "{name} must inherit production lint policy"
        );
        assert_eq!(
            manifest["lints"].as_table().expect("lint table").len(),
            1,
            "{name} must not weaken inherited lints locally"
        );
        for field in ["version", "edition", "rust-version", "license", "publish"] {
            assert_eq!(
                manifest["package"][field]["workspace"].as_bool(),
                Some(true),
                "{name} must inherit {field}"
            );
        }
    }
}
