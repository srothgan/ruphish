//! Restrictive publication permissions for generated keys, databases and backups.
use std::{io, path::Path};

/// Atomically publish a private file if absent. Existing files are never replaced.
/// Returns false if another process or the operator already created the destination.
pub fn publish_new(path: &Path, bytes: &[u8]) -> Result<bool, io::Error> {
    use io::Write;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    protect(temporary.path())?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(path) {
        Ok(_) => Ok(true),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error.error),
    }
}

#[cfg(unix)]
pub(crate) fn protect(path: &Path) -> Result<(), io::Error> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(windows)]
pub(crate) fn protect(path: &Path) -> Result<(), io::Error> {
    use windows_permissions::{
        LocalBox, SecurityDescriptor,
        constants::{SeObjectType, SecurityInformation},
        utilities::current_process_sid,
        wrappers::SetNamedSecurityInfo,
    };
    let sid = current_process_sid()?;
    let descriptor: LocalBox<SecurityDescriptor> = format!("D:P(A;;FA;;;{sid})").parse()?;
    SetNamedSecurityInfo(
        path,
        SeObjectType::SE_FILE_OBJECT,
        SecurityInformation::Dacl | SecurityInformation::ProtectedDacl,
        None,
        None,
        descriptor.dacl(),
        None,
    )
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn protect(_path: &Path) -> Result<(), io::Error> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Private file permissions are not implemented on this OS",
    ))
}
