use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Work,
    Short,
    Long,
}

impl Kind {
    pub fn label(&self) -> &'static str {
        match self {
            Kind::Work => "work",
            Kind::Short => "short break",
            Kind::Long => "long break",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub kind: Kind,
    pub started_at: DateTime<Utc>,
    pub duration_secs: i64,
    #[serde(default)]
    pub elapsed_before_pause_secs: i64,
    #[serde(default)]
    pub paused_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub abort: bool,
    #[serde(default)]
    pub skip: bool,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub pomodoros_completed: u32,
}

impl State {
    pub fn new(kind: Kind, duration_secs: i64, label: Option<String>, pomodoros_completed: u32) -> Self {
        Self {
            kind,
            started_at: Utc::now(),
            duration_secs,
            elapsed_before_pause_secs: 0,
            paused_at: None,
            abort: false,
            skip: false,
            label,
            pomodoros_completed,
        }
    }

    pub fn elapsed_secs(&self) -> i64 {
        let live = match self.paused_at {
            Some(_) => 0,
            None => (Utc::now() - self.started_at).num_seconds(),
        };
        self.elapsed_before_pause_secs + live.max(0)
    }

    pub fn remaining_secs(&self) -> i64 {
        (self.duration_secs - self.elapsed_secs()).max(0)
    }

    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub work_secs: i64,
    pub short_secs: i64,
    pub long_secs: i64,
    pub long_every: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            work_secs: 25 * 60,
            short_secs: 5 * 60,
            long_secs: 15 * 60,
            long_every: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub kind: Kind,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub duration_secs: i64,
    pub completed: bool,
    #[serde(default)]
    pub label: Option<String>,
}

pub fn home_dir() -> PathBuf {
    let mut p = dirs::home_dir().expect("could not resolve home directory");
    p.push(".timer-tiger");
    p
}

pub fn ensure_home() -> std::io::Result<PathBuf> {
    let p = home_dir();
    fs::create_dir_all(&p)?;
    Ok(p)
}

pub fn state_path() -> PathBuf {
    let mut p = home_dir();
    p.push("state.json");
    p
}

pub fn pid_path() -> PathBuf {
    let mut p = home_dir();
    p.push("tt.pid");
    p
}

pub fn log_path() -> PathBuf {
    let mut p = home_dir();
    p.push("tt.log");
    p
}

pub fn history_path() -> PathBuf {
    let mut p = home_dir();
    p.push("history.jsonl");
    p
}

pub fn config_path() -> PathBuf {
    let mut p = home_dir();
    p.push("config.json");
    p
}

pub fn load_state() -> std::io::Result<Option<State>> {
    let p = state_path();
    if !p.exists() {
        return Ok(None);
    }
    let mut f = File::open(&p)?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    if s.trim().is_empty() {
        return Ok(None);
    }
    match serde_json::from_str(&s) {
        Ok(state) => Ok(Some(state)),
        Err(e) => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
    }
}

pub fn save_state(state: &State) -> std::io::Result<()> {
    ensure_home()?;
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let p = state_path();
    let tmp = p.with_extension("json.tmp");
    {
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)?;
        f.write_all(json.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(tmp, p)?;
    Ok(())
}

pub fn delete_state() -> std::io::Result<()> {
    let p = state_path();
    if p.exists() {
        fs::remove_file(p)?;
    }
    Ok(())
}

pub fn load_config() -> Config {
    let p = config_path();
    if !p.exists() {
        return Config::default();
    }
    fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_config(cfg: &Config) -> std::io::Result<()> {
    ensure_home()?;
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(config_path(), json)
}

pub fn append_history(entry: &HistoryEntry) -> std::io::Result<()> {
    ensure_home()?;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_path())?;
    let line = serde_json::to_string(entry)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    f.write_all(line.as_bytes())?;
    f.write_all(b"\n")?;
    Ok(())
}

pub fn read_history() -> std::io::Result<Vec<HistoryEntry>> {
    let p = history_path();
    if !p.exists() {
        return Ok(Vec::new());
    }
    let s = fs::read_to_string(p)?;
    let mut out = Vec::new();
    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(e) = serde_json::from_str::<HistoryEntry>(line) {
            out.push(e);
        }
    }
    Ok(out)
}

pub fn fmt_remaining(secs: i64) -> String {
    let s = secs.max(0);
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// Count completed work pomodoros in history for "today" (local date).
pub fn pomodoros_today() -> u32 {
    let today = chrono::Local::now().date_naive();
    read_history()
        .unwrap_or_default()
        .iter()
        .filter(|e| {
            e.completed
                && e.kind == Kind::Work
                && e.ended_at.with_timezone(&chrono::Local).date_naive() == today
        })
        .count() as u32
}
