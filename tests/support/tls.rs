//! Short-lived server certificates for local protocol peers, including native verifiers.
pub fn server_certificate(names: &[&str]) -> rcgen::CertifiedKey<rcgen::KeyPair> {
    let mut params = rcgen::CertificateParams::new(
        names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    params.not_before = time::OffsetDateTime::now_utc() - time::Duration::minutes(5);
    params.not_after = time::OffsetDateTime::now_utc() + time::Duration::days(1);
    params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    let signing_key = rcgen::KeyPair::generate().unwrap();
    let cert = params.self_signed(&signing_key).unwrap();
    rcgen::CertifiedKey { cert, signing_key }
}
