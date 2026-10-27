//! Size-based file logging with bounded archives and cooperating-process locking.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Retention policy for one active file and its numbered archives.
#[derive(Clone, Copy)]
pub struct Policy {
    /// Rotate before a record would exceed this many bytes. Oversized records stay intact.
    pub max_size_bytes: u64,
    /// Maximum number of archived files, excluding the active file.
    pub max_files: usize,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_size_bytes: 10 * 1024 * 1024,
            max_files: 5,
        }
    }
}

impl Policy {
    /// Reject settings that cannot retain an active file and a bounded history.
    pub fn validate(self) -> io::Result<()> {
        if self.max_size_bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "logging.max_size_bytes must be greater than zero",
            ));
        }
        if self.max_files == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "logging.max_files must be greater than zero",
            ));
        }
        Ok(())
    }
}

/// Append complete records, archive by increasing sequence number, and remove oldest archives.
/// All cooperating writers must use the same policy. Do not externally rotate these files or
/// remove the lock file while a writer is running. No log contents are copied during rotation.
pub struct Writer {
    path: PathBuf,
    policy: Policy,
    lock: File,
}

impl Writer {
    /// Open private log/lock files, preserving existing content and enforcing archive retention.
    pub fn open(path: &Path, policy: Policy) -> io::Result<Self> {
        policy.validate()?;
        regular_file(path)?;
        let lock_path = suffixed(path, ".lock");
        ensure_file(&lock_path)?;
        let lock = OpenOptions::new().read(true).write(true).open(lock_path)?;
        let mut writer = Self {
            path: path.to_owned(),
            policy,
            lock,
        };
        writer.path = writer.locked(|| {
            ensure_file(&writer.path)?;
            // Opening now surfaces permissions before the server reaches readiness.
            OpenOptions::new().append(true).open(&writer.path)?;
            // In particular, differently cased Windows paths must discover the same archives.
            fs::canonicalize(&writer.path)
        })?;
        writer.locked(|| writer.prune(&mut writer.archives()?))?;
        Ok(writer)
    }

    fn locked<T>(&self, operation: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
        self.lock.lock()?;
        let result = operation();
        let unlocked = self.lock.unlock();
        match result {
            Ok(value) => unlocked.map(|()| value),
            Err(error) => Err(error),
        }
    }

    fn archives(&self) -> io::Result<Vec<(u64, PathBuf)>> {
        let name = self.path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Log path must name a file")
        })?;
        let mut prefix = name.to_os_string();
        prefix.push(".");
        let mut archives = Vec::new();
        for entry in fs::read_dir(parent(&self.path))? {
            let entry = entry?;
            let candidate = entry.file_name();
            // Match the original OS filename byte-for-byte; only the suffix is Unicode.
            let Some(suffix) = candidate
                .as_encoded_bytes()
                .strip_prefix(prefix.as_encoded_bytes())
            else {
                continue;
            };
            let Ok(suffix) = std::str::from_utf8(suffix) else {
                continue;
            };
            let Ok(index) = suffix.parse::<u64>() else {
                continue;
            };
            if index == 0 || index.to_string() != suffix {
                continue;
            }
            if !entry.file_type()?.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Log archive must be a regular file",
                ));
            }
            archives.push((index, entry.path()));
        }
        archives.sort_unstable_by_key(|(index, _)| *index);
        Ok(archives)
    }

    fn prune(&self, archives: &mut Vec<(u64, PathBuf)>) -> io::Result<()> {
        let excess = archives.len().saturating_sub(self.policy.max_files);
        for (_, path) in archives.iter().take(excess) {
            fs::remove_file(path)?;
        }
        archives.drain(..excess);
        Ok(())
    }

    fn append(&self, bytes: &[u8]) -> io::Result<()> {
        // Recover an interrupted rollover/cleanup before accepting more file output. This also
        // applies lower retention settings on restart and observes other cooperating writers.
        let mut archives = self.archives()?;
        self.prune(&mut archives)?;
        ensure_file(&self.path)?;
        let mut file = OpenOptions::new().append(true).open(&self.path)?;
        let size = file.metadata()?.len();
        if size > 0 && size.saturating_add(bytes.len() as u64) > self.policy.max_size_bytes {
            drop(file); // Required before rename on Windows; no writer keeps a stale active handle.
            let index = archives
                .last()
                .map_or(Some(1), |(index, _)| index.checked_add(1))
                .ok_or_else(|| io::Error::other("Log archive sequence exhausted"))?;
            let archive = suffixed(&self.path, &format!(".{index}"));
            fs::rename(&self.path, &archive)?;
            archives.push((index, archive));
            // Do not create another active file if cleanup fails. Subsequent records retry
            // cleanup, preventing an endless series of archives on a retention failure.
            self.prune(&mut archives)?;
            ensure_file(&self.path)?;
            file = OpenOptions::new().append(true).open(&self.path)?;
        }
        file.write_all(bytes)
    }
}

impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !bytes.is_empty() {
            self.locked(|| self.append(bytes))?;
        }
        Ok(bytes.len())
    }

    fn write_fmt(&mut self, arguments: std::fmt::Arguments<'_>) -> io::Result<()> {
        // Keep a formatted record within one lock/rollover decision, just like tracing's
        // preformatted write_all calls, rather than interleaving individual format fragments.
        let mut record = String::new();
        std::fmt::write(&mut record, arguments)
            .map_err(|_| io::Error::other("Could not format log record"))?;
        self.write_all(record.as_bytes())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Each write closes its unbuffered file. This is operational logging, not an fsync audit log.
        Ok(())
    }
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn ensure_file(path: &Path) -> io::Result<()> {
    if !regular_file(path)? {
        crate::private_file::publish_new(path, b"")?;
        // A concurrent creator must also have published an ordinary file.
        if !regular_file(path)? {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Log file disappeared",
            ));
        }
    }
    Ok(())
}

fn regular_file(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Log path must be a regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
