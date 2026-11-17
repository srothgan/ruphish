//! Prevent isolated Cargo resolution from silently testing another product graph.
use std::{collections::BTreeSet, path::Path};

fn read(path: &Path) -> toml::Value {
    toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn product_dependency_versions_and_patches_match() {
    let fuzz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = fuzz.parent().unwrap();
    let product_manifest = read(&root.join("Cargo.toml"));
    let mut fuzz_manifest = read(&fuzz.join("Cargo.toml"));
    for (_, registry) in fuzz_manifest["patch"].as_table_mut().unwrap().iter_mut() {
        for (_, dependency) in registry.as_table_mut().unwrap().iter_mut() {
            if let Some(path) = dependency.get_mut("path") {
                *path = path.as_str().unwrap().strip_prefix("../").unwrap().into();
            }
        }
    }
    assert_eq!(
        product_manifest["patch"], fuzz_manifest["patch"],
        "Mirror product patches in fuzz/Cargo.toml"
    );
    let product_lock = read(&root.join("Cargo.lock"));
    let fuzz_lock = read(&fuzz.join("Cargo.lock"));
    let product_packages = product_lock["package"].as_array().unwrap();
    let product_names: BTreeSet<_> = product_packages
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    for package in fuzz_lock["package"].as_array().unwrap() {
        if product_names.contains(package["name"].as_str().unwrap()) {
            assert!(
                product_packages.iter().any(|p| {
                    ["name", "version", "source", "checksum"]
                        .iter()
                        .all(|key| p.get(key) == package.get(key))
                }),
                "Fuzz dependency differs from product lock: {package}"
            );
        }
    }
}
