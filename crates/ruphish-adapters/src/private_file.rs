//! Restrictive publication permissions for generated keys, databases and backups.
use std::{io, path::Path};

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
