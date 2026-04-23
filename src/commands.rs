use chrono::{Datelike, Local};

use crate::daemon;
use crate::state::{
    self, fmt_remaining, load_config, load_state, pomodoros_today, save_config, save_state, Config,
    Kind, State,
};

pub enum BreakKind {
    Auto,
    Short,
    Long,
}

pub fn cmd_start(
    work_min: Option<u32>,
    work_seconds: Option<u32>,
    label: Option<String>,
    force: bool,
) -> i32 {
    daemon::cleanup_stale();
    if let Ok(Some(s)) = load_state() {
        if !force && daemon::daemon_running() {
            eprintln!(
                "Timer already running: {} left ({}). Use `tt stop` or `tt start --force`.",
                fmt_remaining(s.remaining_secs()),
                s.kind.label()
            );
            return 1;
        }
        if force {
            stop_running_silently();
        }
    }

    let cfg = load_config();
    let secs = if let Some(s) = work_seconds {
        s as i64
    } else if let Some(m) = work_min {
        (m as i64) * 60
    } else {
        cfg.work_secs
    };
    let done_today = pomodoros_today();
    let state = State::new(Kind::Work, secs, label.clone(), done_today);

    if let Err(e) = save_state(&state) {
        eprintln!("could not write state: {e}");
        return 1;
    }
    if let Err(e) = daemon::spawn_detached() {
        eprintln!("could not spawn daemon: {e}");
        return 1;
    }
    println!(
        "Started work for {}{}.",
        fmt_remaining(secs),
        match &label {
            Some(l) if !l.is_empty() => format!(" ({l})"),
            _ => String::new(),
        }
    );
    0
}

pub fn cmd_break(kind: BreakKind) -> i32 {
    daemon::cleanup_stale();
    if load_state().ok().flatten().is_some() && daemon::daemon_running() {
        eprintln!("A timer is already running. Use `tt stop` first.");
        return 1;
    }

    let cfg = load_config();
    let done_today = pomodoros_today();
    let (kind, secs) = match kind {
        BreakKind::Short => (Kind::Short, cfg.short_secs),
        BreakKind::Long => (Kind::Long, cfg.long_secs),
        BreakKind::Auto => {
            if done_today > 0 && done_today % cfg.long_every == 0 {
                (Kind::Long, cfg.long_secs)
            } else {
                (Kind::Short, cfg.short_secs)
            }
        }
    };

    let state = State::new(kind, secs, None, done_today);
    if let Err(e) = save_state(&state) {
        eprintln!("could not write state: {e}");
        return 1;
    }
    if let Err(e) = daemon::spawn_detached() {
        eprintln!("could not spawn daemon: {e}");
        return 1;
    }
    println!("Started {} for {}.", kind.label(), fmt_remaining(secs));
    0
}

pub fn cmd_stop() -> i32 {
    let mut state = match load_state().ok().flatten() {
        Some(s) => s,
        None => {
            println!("No timer running.");
            return 0;
        }
    };
    state.abort = true;
    if let Err(e) = save_state(&state) {
        eprintln!("could not update state: {e}");
        return 1;
    }
    println!("Stop requested. Timer will end shortly.");
    0
}

fn stop_running_silently() {
    if let Some(mut s) = load_state().ok().flatten() {
        s.abort = true;
        let _ = save_state(&s);
        std::thread::sleep(std::time::Duration::from_millis(1500));
    }
    let _ = state::delete_state();
}

pub fn cmd_pause() -> i32 {
    let mut s = match load_state().ok().flatten() {
        Some(s) => s,
        None => {
            println!("No timer running.");
            return 0;
        }
    };
    if s.is_paused() {
        println!("Already paused.");
        return 0;
    }
    let elapsed_now = (chrono::Utc::now() - s.started_at).num_seconds().max(0);
    s.elapsed_before_pause_secs += elapsed_now;
    s.paused_at = Some(chrono::Utc::now());
    if let Err(e) = save_state(&s) {
        eprintln!("could not update state: {e}");
        return 1;
    }
    println!("Paused at {} remaining.", fmt_remaining(s.remaining_secs()));
    0
}

pub fn cmd_resume() -> i32 {
    let mut s = match load_state().ok().flatten() {
        Some(s) => s,
        None => {
            println!("No timer running.");
            return 0;
        }
    };
    if !s.is_paused() {
        println!("Not paused.");
        return 0;
    }
    s.paused_at = None;
    s.started_at = chrono::Utc::now();
    if let Err(e) = save_state(&s) {
        eprintln!("could not update state: {e}");
        return 1;
    }
    println!("Resumed. {} remaining.", fmt_remaining(s.remaining_secs()));
    0
}

pub fn cmd_skip() -> i32 {
    let mut s = match load_state().ok().flatten() {
        Some(s) => s,
        None => {
            println!("No timer running.");
            return 0;
        }
    };
    s.skip = true;
    if let Err(e) = save_state(&s) {
        eprintln!("could not update state: {e}");
        return 1;
    }
    println!("Marked as completed.");
    0
}

pub fn cmd_status() -> i32 {
    daemon::cleanup_stale();
    match load_state().ok().flatten() {
        None => {
            println!("idle | today: {} pomodoros", pomodoros_today());
        }
        Some(s) => {
            let pause_tag = if s.is_paused() { " [paused]" } else { "" };
            let label = match &s.label {
                Some(l) if !l.is_empty() => format!(" \"{l}\""),
                _ => String::new(),
            };
            println!(
                "{}{}{} | {} left | today: {} pomodoros",
                s.kind.label(),
                label,
                pause_tag,
                fmt_remaining(s.remaining_secs()),
                pomodoros_today()
            );
        }
    }
    0
}

pub enum LogRange {
    Today,
    Week,
    All,
}

pub fn cmd_log(range: LogRange) -> i32 {
    let entries = state::read_history().unwrap_or_default();
    let now = Local::now();
    let today = now.date_naive();
    let iso_week_now = now.iso_week();

    let filtered: Vec<_> = entries
        .into_iter()
        .filter(|e| {
            let local = e.ended_at.with_timezone(&Local);
            match range {
                LogRange::All => true,
                LogRange::Today => local.date_naive() == today,
                LogRange::Week => {
                    let w = local.iso_week();
                    w.year() == iso_week_now.year() && w.week() == iso_week_now.week()
                }
            }
        })
        .collect();

    if filtered.is_empty() {
        println!("No sessions in range.");
        return 0;
    }

    let mut work_done = 0u32;
    let mut focus_secs = 0i64;
    for e in &filtered {
        let local = e.ended_at.with_timezone(&Local);
        let status = if e.completed { "done" } else { "abort" };
        let label = e.label.as_deref().unwrap_or("");
        println!(
            "{}  {:>5}  {:>5}  {:<12}  {}",
            local.format("%Y-%m-%d %H:%M"),
            e.kind.label(),
            fmt_remaining(e.duration_secs),
            status,
            label
        );
        if e.completed && e.kind == Kind::Work {
            work_done += 1;
            focus_secs += e.duration_secs;
        }
    }
    println!(
        "----\nwork pomodoros: {} | focus time: {}m",
        work_done,
        focus_secs / 60
    );
    0
}

pub enum ConfigOp {
    Show,
    Get(String),
    Set(String, String),
}

pub fn cmd_config(op: ConfigOp) -> i32 {
    let mut cfg = load_config();
    match op {
        ConfigOp::Show => {
            print_config(&cfg);
        }
        ConfigOp::Get(k) => match get_field(&cfg, &k) {
            Some(v) => println!("{v}"),
            None => {
                eprintln!("unknown key: {k}");
                return 1;
            }
        },
        ConfigOp::Set(k, v) => {
            if let Err(e) = set_field(&mut cfg, &k, &v) {
                eprintln!("{e}");
                return 1;
            }
            if let Err(e) = save_config(&cfg) {
                eprintln!("could not save config: {e}");
                return 1;
            }
            print_config(&cfg);
        }
    }
    0
}

fn print_config(cfg: &Config) {
    println!("work_secs   = {}", cfg.work_secs);
    println!("short_secs  = {}", cfg.short_secs);
    println!("long_secs   = {}", cfg.long_secs);
    println!("long_every  = {}", cfg.long_every);
}

fn get_field(cfg: &Config, key: &str) -> Option<String> {
    match key {
        "work_secs" => Some(cfg.work_secs.to_string()),
        "short_secs" => Some(cfg.short_secs.to_string()),
        "long_secs" => Some(cfg.long_secs.to_string()),
        "long_every" => Some(cfg.long_every.to_string()),
        _ => None,
    }
}

fn set_field(cfg: &mut Config, key: &str, val: &str) -> Result<(), String> {
    match key {
        "work_secs" => cfg.work_secs = val.parse().map_err(|_| "expected integer seconds".to_string())?,
        "short_secs" => cfg.short_secs = val.parse().map_err(|_| "expected integer seconds".to_string())?,
        "long_secs" => cfg.long_secs = val.parse().map_err(|_| "expected integer seconds".to_string())?,
        "long_every" => cfg.long_every = val.parse().map_err(|_| "expected positive integer".to_string())?,
        other => return Err(format!("unknown key: {other}")),
    }
    Ok(())
}

pub fn cmd_help_overview() {
    println!(
        "Timer Tiger - Pomodoro CLI\n\n\
USAGE\n  tt <command> [options]\n\n\
COMMANDS\n\
  start     Start a work pomodoro (default 25m)\n\
  break     Start a short or long break\n\
  stop      Abort the current session\n\
  pause     Pause the running timer\n\
  resume    Resume a paused timer\n\
  skip      Mark current session done immediately\n\
  status    Show what's running and time left\n\
  log       Show today's / week's / all history\n\
  config    Get or set defaults (durations, cycle)\n\
  help      Show this help (or `tt help <command>`)\n\n\
EXAMPLES\n\
  tt start --label \"write report\"\n\
  tt status\n\
  tt pause && tt resume\n\
  tt break --long\n\
  tt log --today\n\
  tt config set work_secs 1800\n"
    );
}
