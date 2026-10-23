//! Real filesystem publication, reload and platform-specific private-key protection.
use ruphish_adapters::tls;

#[test]
fn generated_certificate_verifies_its_host_and_expires_within_a_year() {
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime, pem::PemObject};
    use std::{sync::Arc, time::Duration};

    let directory = tempfile::tempdir().unwrap();
    let material = tls::load_or_generate(
        &directory.path().join("admin.crt"),
        &directory.path().join("admin.key"),
        "admin.example.test",
    )
    .unwrap();
    let certificate = CertificateDer::from_pem_slice(&material.certificate).unwrap();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(certificate.clone()).unwrap();
    let verifier = rustls::client::WebPkiServerVerifier::builder_with_provider(
        Arc::new(roots),
        Arc::new(rustls::crypto::ring::default_provider()),
    )
    .build()
    .unwrap();
    use rustls::client::danger::ServerCertVerifier;
    let now = UnixTime::now();
    for host in ["localhost", "127.0.0.1", "::1", "admin.example.test"] {
        let name = ServerName::try_from(host).unwrap();
        verifier
            .verify_server_cert(&certificate, &[], &name, &[], now)
            .unwrap();
    }
    let name = ServerName::try_from("admin.example.test").unwrap();
    let later = UnixTime::since_unix_epoch(Duration::from_secs(now.as_secs() + 366 * 86400));
    assert!(matches!(
        verifier.verify_server_cert(&certificate, &[], &name, &[], later),
        Err(rustls::Error::InvalidCertificate(
            rustls::CertificateError::Expired | rustls::CertificateError::ExpiredContext { .. }
        ))
    ));
    assert!(
        verifier
            .verify_server_cert(
                &certificate,
                &[],
                &ServerName::try_from("other.example.test").unwrap(),
                &[],
                now
            )
            .is_err()
    );
}

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
