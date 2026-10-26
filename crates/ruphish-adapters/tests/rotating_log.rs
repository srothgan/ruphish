//! Filesystem outcomes for bounded operational logging, including recovery and concurrent writers.
#![allow(clippy::unwrap_used, reason = "Isolated test fixtures and assertions")]

use ruphish_adapters::rotating_log::{Policy, Writer};
use std::{
    fs,
    io::Write,
    path::Path,
    sync::{Arc, Barrier},
};

fn policy(size: u64, archives: usize) -> Policy {
    Policy {
        max_size_bytes: size,
        max_files: archives,
    }
}

fn read(root: &Path, name: &str) -> String {
    fs::read_to_string(root.join(name)).unwrap()
}

#[test]
fn append_rotate_reopen_and_reduce_retention_preserve_newest_complete_records() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let path = root.join("app.log");
    fs::write(&path, "old\n").unwrap();
    fs::write(root.join("app.log.notes"), "operator notes").unwrap();
    fs::write(root.join("app.log.01"), "unrelated name").unwrap();
    let mut writer = Writer::open(&path, policy(8, 2)).unwrap();
    writer.write_all(b"one\n").unwrap();
    assert_eq!(read(root, "app.log"), "old\none\n");
    for record in [b"two\n", b"tri\n", b"for\n", b"fiv\n", b"six\n"] {
        writer.write_all(record).unwrap();
    }
    assert!(!root.join("app.log.1").exists());
    assert_eq!(read(root, "app.log.2"), "two\ntri\n");
    assert_eq!(read(root, "app.log.3"), "for\nfiv\n");
    assert_eq!(read(root, "app.log"), "six\n");
    drop(writer);
    let mut reopened = Writer::open(&path, policy(8, 1)).unwrap();
    assert!(!root.join("app.log.2").exists());
    reopened.write_all(b"sev\n").unwrap();
    reopened.write_all(b"end\n").unwrap();
    assert!(!root.join("app.log.3").exists());
    assert_eq!(read(root, "app.log.4"), "six\nsev\n");
    assert_eq!(read(root, "app.log"), "end\n");
    assert_eq!(read(root, "app.log.notes"), "operator notes");
    assert_eq!(read(root, "app.log.01"), "unrelated name");
}

#[test]
fn oversized_records_stay_intact_and_rotate_before_the_next_record() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("app.log");
    let mut writer = Writer::open(&path, policy(5, 1)).unwrap();
    writer.write_all(b"large single event\n").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"large single event\n");
    writer.write_all(b"next\n").unwrap();
    assert_eq!(read(directory.path(), "app.log.1"), "large single event\n");
    assert_eq!(fs::read(&path).unwrap(), b"next\n");
}

#[test]
fn rotation_failure_preserves_active_and_operator_files_then_recovers() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let path = root.join("app.log");
    let mut writer = Writer::open(&path, policy(4, 2)).unwrap();
    writer.write_all(b"old\n").unwrap();
    fs::create_dir(root.join("app.log.1")).unwrap();
    fs::write(root.join("app.log.1/keep"), "operator file").unwrap();
    assert!(writer.write_all(b"new\n").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"old\n");
    assert_eq!(read(root, "app.log.1/keep"), "operator file");
    fs::remove_file(root.join("app.log.1/keep")).unwrap();
    fs::remove_dir(root.join("app.log.1")).unwrap();
    writer.write_all(b"new\n").unwrap();
    assert_eq!(read(root, "app.log.1"), "old\n");
    assert_eq!(fs::read(&path).unwrap(), b"new\n");
}

#[test]
fn restart_after_interrupted_rollover_recovers_missing_active_and_excess_archives() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    for (name, bytes) in [("app.log.8", "old\n"), ("app.log.9", "new\n")] {
        fs::write(root.join(name), bytes).unwrap();
    }
    // The previous process renamed active to .9, then stopped before creating active/cleanup.
    let path = root.join("app.log");
    let mut writer = Writer::open(&path, policy(4, 1)).unwrap();
    assert!(!root.join("app.log.8").exists());
    assert_eq!(read(root, "app.log.9"), "new\n");
    writer.write_all(b"now\n").unwrap();
    writer.write_all(b"end\n").unwrap();
    assert!(!root.join("app.log.9").exists());
    assert_eq!(read(root, "app.log.10"), "now\n");
    assert_eq!(fs::read(&path).unwrap(), b"end\n");
}

#[test]
fn independent_writers_serialize_rotation_without_losing_or_splitting_records() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("app.log");
    let barrier = Arc::new(Barrier::new(4));
    let threads: Vec<_> = (0..4)
        .map(|writer_id| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut writer = Writer::open(&path, policy(64, 100)).unwrap();
                barrier.wait();
                for index in 0..20 {
                    writeln!(writer, "{writer_id}:{index:02}").unwrap();
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let mut records = Vec::new();
    for entry in fs::read_dir(directory.path()).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_some_and(|extension| extension == "lock")
        {
            continue;
        }
        assert!(fs::metadata(&path).unwrap().len() <= 64);
        records.extend(
            fs::read_to_string(&path)
                .unwrap()
                .lines()
                .map(str::to_owned),
        );
    }
    records.sort();
    let expected: Vec<_> = (0..4)
        .flat_map(|writer_id| (0..20).map(move |index| format!("{writer_id}:{index:02}")))
        .collect();
    assert_eq!(records, expected);
}

#[test]
fn new_files_are_private_and_invalid_limits_do_not_create_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("app.log");
    assert!(Writer::open(&path, policy(0, 2)).is_err());
    assert!(Writer::open(&path, policy(10, 0)).is_err());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    let mut writer = Writer::open(&path, policy(4, 1)).unwrap();
    writer.write_all(b"old\n").unwrap();
    writer.write_all(b"new\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for name in ["app.log", "app.log.1", "app.log.lock"] {
            assert_eq!(
                fs::metadata(directory.path().join(name))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn symlinks_in_log_or_archive_paths_never_modify_their_targets() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let target = root.join("operator.txt");
    fs::write(&target, "keep").unwrap();
    let path = root.join("app.log");
    symlink(&target, &path).unwrap();
    assert!(Writer::open(&path, policy(4, 1)).is_err());
    fs::remove_file(&path).unwrap();
    let mut writer = Writer::open(&path, policy(4, 1)).unwrap();
    writer.write_all(b"old\n").unwrap();
    symlink(&target, root.join("app.log.1")).unwrap();
    assert!(writer.write_all(b"new\n").is_err());
    assert_eq!(fs::read_to_string(&target).unwrap(), "keep");
    assert_eq!(fs::read(&path).unwrap(), b"old\n");
}

#[cfg(windows)]
#[test]
fn failed_archive_deletion_stops_file_growth_and_resumes_after_reader_closes() {
    use std::os::windows::fs::OpenOptionsExt;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let path = root.join("app.log");
    let mut writer = Writer::open(&path, policy(4, 1)).unwrap();
    writer.write_all(b"old\n").unwrap();
    writer.write_all(b"one\n").unwrap();
    let archive = root.join("app.log.1");
    let reader = fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&archive)
        .unwrap();
    // FILE_SHARE_READ | FILE_SHARE_WRITE, without FILE_SHARE_DELETE: rename of active
    // succeeds, but a reader of the old archive prevents its deletion on Windows.
    for _ in 0..3 {
        assert!(writer.write_all(b"two\n").is_err());
        assert!(!path.exists());
        assert_eq!(read(root, "app.log.1"), "old\n");
        assert_eq!(read(root, "app.log.2"), "one\n");
        assert!(!root.join("app.log.3").exists());
    }
    drop(reader);
    writer.write_all(b"two\n").unwrap();
    assert!(!archive.exists());
    assert_eq!(read(root, "app.log.2"), "one\n");
    assert_eq!(fs::read(path).unwrap(), b"two\n");
}

#[cfg(windows)]
#[test]
fn differently_cased_paths_share_one_archive_sequence_on_windows() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let mut first = Writer::open(&root.join("app.log"), policy(4, 2)).unwrap();
    first.write_all(b"one\n").unwrap();
    let mut second = Writer::open(&root.join("APP.LOG"), policy(4, 2)).unwrap();
    first.write_all(b"two\n").unwrap();
    second.write_all(b"tri\n").unwrap();
    assert_eq!(read(root, "app.log.1"), "one\n");
    assert_eq!(read(root, "app.log.2"), "two\n");
    assert_eq!(read(root, "app.log"), "tri\n");
}
