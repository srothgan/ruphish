//! Failure-only process observations; never dump configuration or database contents.
use std::{
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn snapshot(directory: &Path, pid: Option<u32>, mode: &str, started: Instant) -> String {
    let files: Vec<_> = [
        "server.log",
        "admin.crt",
        "admin.key",
        "public.crt",
        "public.key",
        "installation.sqlite3",
        "installation.sqlite3-journal",
        "installation.sqlite3-wal",
        "installation.sqlite3.ruphish-startup.lock",
    ]
    .into_iter()
    .map(|name| match std::fs::metadata(directory.join(name)) {
        Ok(metadata) => format!(
            "{name}: {} bytes, modified {:?}",
            metadata.len(),
            metadata.modified()
        ),
        Err(error) => format!("{name}: {:?}", error.kind()),
    })
    .collect();
    format!(
        "pid={pid:?}, mode={mode}, elapsed={:?}, workspace={}\nprocess status: {}\nworkspace metadata:\n{}",
        started.elapsed(),
        directory.display(),
        pid.map(process_status)
            .unwrap_or_else(|| "PID unavailable".into()),
        files.join("\n"),
    )
}

fn process_status(pid: u32) -> String {
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new("powershell.exe");
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "Get-Process -Id {pid} -ErrorAction Stop | Select-Object Id,CPU,WorkingSet64,@{{Name='Threads';Expression={{$_.Threads.Count}}}} | ConvertTo-Json -Compress"
            ),
        ]);
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        command
    };
    #[cfg(unix)]
    let mut command = {
        let mut command = Command::new("ps");
        command.args([
            "-p",
            &pid.to_string(),
            "-o",
            "pid,etime,time,pcpu,rss,state",
        ]);
        command
    };
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return format!("status command unavailable: {error}"),
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return match child.wait_with_output() {
                    Ok(output) => format!(
                        "success={}; stdout={}; stderr={}",
                        output.status.success(),
                        String::from_utf8_lossy(&output.stdout).trim(),
                        String::from_utf8_lossy(&output.stderr).trim(),
                    ),
                    Err(error) => format!("could not read status command: {error}"),
                };
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return format!("status command did not finish within five seconds: {result:?}");
            }
        }
    }
}
