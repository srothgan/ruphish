//! Real filesystem publication, reload and platform-specific private-key protection.
use ruphish_adapters::tls;

#[test]
fn generated_private_key_is_restricted_and_existing_pair_is_loaded_byte_for_byte() {
    let directory = tempfile::tempdir().unwrap();
    let certificate = directory.path().join("admin.crt");
    let key = directory.path().join("admin.key");
    let first = tls::load_or_generate(&certificate, &key, "localhost").unwrap();
    let loaded = tls::load_or_generate(&certificate, &key, "another.example.test").unwrap();
    assert_eq!(loaded.key, first.key);
    assert_eq!(loaded.certificate, first.certificate);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&key).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    #[cfg(windows)]
    {
        use windows_permissions::{
            constants::{SeObjectType, SecurityInformation},
            utilities::current_process_sid,
            wrappers::GetNamedSecurityInfo,
        };
        let descriptor = GetNamedSecurityInfo(
            &key,
            SeObjectType::SE_FILE_OBJECT,
            SecurityInformation::Dacl,
        )
        .unwrap();
        let acl = descriptor.dacl().unwrap();
        assert_eq!(acl.len(), 1);
        assert_eq!(
            acl.get_ace(0).unwrap().sid(),
            Some(current_process_sid().unwrap().as_ref())
        );
        let sddl = descriptor.as_sddl().unwrap();
        let sddl = sddl.to_string_lossy();
        let flags = sddl.split_once("D:").unwrap().1.split('(').next().unwrap();
        assert!(
            flags.contains('P'),
            "DACL must be protected from inherited grants: {sddl}"
        );
    }
    std::fs::remove_file(&certificate).unwrap();
    assert!(tls::load_or_generate(&certificate, &key, "localhost").is_err());
    assert_eq!(std::fs::read(&key).unwrap(), first.key);
    assert!(!certificate.exists());
}

#[test]
fn failed_generation_and_ambiguous_paths_do_not_publish_or_overwrite_material() {
    let directory = tempfile::tempdir().unwrap();
    let key = directory.path().join("key.pem");
    assert!(tls::load_or_generate(&key, &key, "localhost").is_err());
    assert!(!key.exists());
    assert!(
        tls::load_or_generate(
            &directory.path().join("missing/cert.pem"),
            &key,
            "localhost"
        )
        .is_err()
    );
    assert!(!key.exists());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}
