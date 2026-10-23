//! Certificate material with no-clobber publication and OS-specific private-key protection.

use std::{
    io::{self, Write},
    path::Path,
};

/// Certificate preparation failed without overwriting supplied material.
#[derive(Debug, thiserror::Error)]
pub enum CertificateError {
    /// Filesystem or permissions failure.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Certificate generation failed.
    #[error(transparent)]
    Generate(#[from] rcgen::Error),
    /// An incomplete or ambiguous pair requires operator repair.
    #[error(
        "TLS certificate and key must be distinct files and either both exist or both be absent; existing files were kept"
    )]
    Incomplete,
}

/// PEM bytes to be parsed by the selected TLS server implementation.
pub struct Material {
    /// Certificate chain, including a generated self-signed leaf where needed.
    pub certificate: Vec<u8>,
    /// Private key. Never log or serialize this field.
    pub key: Vec<u8>,
}

/// Load supplied files, or generate both only when neither exists.
pub fn load_or_generate(
    certificate: &Path,
    key: &Path,
    hostname: &str,
) -> Result<Material, CertificateError> {
    if certificate == key {
        return Err(CertificateError::Incomplete);
    }
    match (certificate.try_exists()?, key.try_exists()?) {
        (true, true) => {
            return Ok(Material {
                certificate: std::fs::read(certificate)?,
                key: std::fs::read(key)?,
            });
        }
        (true, false) | (false, true) => return Err(CertificateError::Incomplete),
        (false, false) => (),
    }
    let mut names = vec!["localhost".into(), "127.0.0.1".into(), "::1".into()];
    if !hostname.is_empty()
        && !matches!(hostname, "0.0.0.0" | "::")
        && !names.iter().any(|name| name == hostname)
    {
        names.push(hostname.to_owned());
    }
    let mut params = rcgen::CertificateParams::new(names)?;
    params.not_before = time::OffsetDateTime::now_utc() - time::Duration::minutes(5);
    // Development certificates still need to satisfy native TLS verifiers.
    // Keep validity comfortably below Apple's server-certificate lifetime limit.
    params.not_after = time::OffsetDateTime::now_utc() + time::Duration::days(365);
    params
        .distinguished_name
        .push(rcgen::DnType::OrganizationName, "Ruphish");
    params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    let signing_key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P384_SHA384)?;
    let material = Material {
        certificate: params.self_signed(&signing_key)?.pem().into_bytes(),
        key: signing_key.serialize_pem().into_bytes(),
    };
    let mut certificate_file = tempfile::NamedTempFile::new_in(parent(certificate))?;
    let mut key_file = tempfile::NamedTempFile::new_in(parent(key))?;
    crate::private_file::protect(key_file.path())?;
    certificate_file.write_all(&material.certificate)?;
    key_file.write_all(&material.key)?;
    certificate_file.as_file().sync_all()?;
    key_file.as_file().sync_all()?;
    // Two filesystem paths cannot be published atomically. If interrupted after
    // one publication, the next startup refuses the incomplete pair explicitly.
    key_file
        .persist_noclobber(key)
        .map_err(|error| error.error)?;
    if let Err(error) = certificate_file.persist_noclobber(certificate) {
        // Remove only the private key this operation just published.
        if std::fs::read(key).is_ok_and(|bytes| bytes == material.key) {
            std::fs::remove_file(key)?;
        }
        return Err(error.error.into());
    }
    Ok(material)
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}
