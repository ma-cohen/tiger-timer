use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use chrono::Utc;
use fs2::FileExt;

use crate::notify;
use crate::state::{
    self, append_history, delete_state, load_state, log_path, pid_path, HistoryEntry, Kind,
};

/// Spawn the current binary as a detached daemon and return.
pub fn spawn_detached() -> std::io::Result<()> {
    state::ensure_home()?;
    let exe = std::env::current_exe()?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())?;
    let log_err = log.try_clone()?;

    unsafe {
        Command::new(exe)
            .arg("--__daemon")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err))
            .pre_exec(|| {
                // Detach from controlling terminal / process group.
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            })
            .spawn()?;
    }
    Ok(())
}

/// Daemon loop. Holds an exclusive lock on the pid file; polls state.json each second.
pub fn run_daemon() {
    let pid_file = match OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(true)
        .open(pid_path())
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("daemon: cannot open pid file: {e}");
            return;
        }
    };

    if let Err(e) = pid_file.try_lock_exclusive() {
        eprintln!("daemon: another instance is running ({e})");
        return;
    }

    {
        let mut f = &pid_file;
        let _ = writeln!(f, "{}", std::process::id());
        let _ = f.sync_all();
    }

    let result = loop_until_done();

    if let Err(e) = result {
        eprintln!("daemon: error: {e}");
    }

    let _ = delete_state();
    let _ = FileExt::unlock(&pid_file);
    let _ = std::fs::remove_file(pid_path());
}

fn loop_until_done() -> std::io::Result<()> {
    loop {
        let state = match load_state()? {
            Some(s) => s,
            None => return Ok(()),
        };

        if state.abort {
            let now = Utc::now();
            let _ = append_history(&HistoryEntry {
                kind: state.kind,
                started_at: state.started_at,
                ended_at: now,
                duration_secs: state.elapsed_secs(),
                completed: false,
                label: state.label.clone(),
            });
            return Ok(());
        }

        if state.skip || state.remaining_secs() == 0 {
            let now = Utc::now();
            let _ = append_history(&HistoryEntry {
                kind: state.kind,
                started_at: state.started_at,
                ended_at: now,
                duration_secs: state.duration_secs,
                completed: true,
                label: state.label.clone(),
            });
            fire_completion(state.kind, state.label.as_deref());
            return Ok(());
        }

        thread::sleep(Duration::from_millis(1000));
    }
}

fn fire_completion(kind: Kind, label: Option<&str>) {
    let (title, msg) = match kind {
        Kind::Work => (
            "Timer Tiger - work done",
            match label {
                Some(l) if !l.is_empty() => format!("Nice. \"{l}\" complete. Time for a break."),
                _ => "Pomodoro complete. Time for a break.".to_string(),
            },
        ),
        Kind::Short => (
            "Timer Tiger - break over",
            "Short break done. Back to work.".to_string(),
        ),
        Kind::Long => (
            "Timer Tiger - break over",
            "Long break done. Back to work.".to_string(),
        ),
    };
    notify::notify(title, &msg);
    notify::play_sound();
}

/// Best-effort: is a daemon currently running (pid file locked)?
pub fn daemon_running() -> bool {
    let p = pid_path();
    if !p.exists() {
        return false;
    }
    let Ok(f) = OpenOptions::new().read(true).write(true).open(&p) else {
        return false;
    };
    match f.try_lock_exclusive() {
        Ok(_) => {
            let _ = FileExt::unlock(&f);
            false
        }
        Err(_) => true,
    }
}

/// If the pid file exists but no daemon holds the lock, remove stale files.
pub fn cleanup_stale() {
    if daemon_running() {
        return;
    }
    let _ = std::fs::remove_file(pid_path());
    let _ = delete_state();
}
