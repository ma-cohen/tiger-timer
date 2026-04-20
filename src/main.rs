mod commands;
mod daemon;
mod notify;
mod state;

use clap::{Args, Parser, Subcommand};

use commands::{BreakKind, ConfigOp, LogRange};

#[derive(Parser, Debug)]
#[command(name = "tt", version, about = "Timer Tiger - Pomodoro CLI", disable_help_subcommand = true)]
struct Cli {
    /// Internal: run the background daemon loop. Not for direct use.
    #[arg(long = "__daemon", hide = true)]
    daemon: bool,

    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Start a work pomodoro (default 25 minutes).
    Start(StartArgs),
    /// Start a short or long break.
    Break(BreakArgs),
    /// Abort the current session (no notification).
    Stop,
    /// Pause the current timer.
    Pause,
    /// Resume a paused timer.
    Resume,
    /// Mark the current session as completed immediately.
    Skip,
    /// Show what's running and how much time is left.
    Status,
    /// Show session history.
    Log(LogArgs),
    /// View or change defaults.
    Config(ConfigArgs),
    /// Show overview help, or details for a command.
    Help(HelpArgs),
}

#[derive(Args, Debug)]
struct StartArgs {
    /// Override work duration in minutes for this session.
    #[arg(short, long, value_name = "MIN")]
    work: Option<u32>,
    /// Optional label for this pomodoro.
    #[arg(short, long)]
    label: Option<String>,
    /// Stop any running timer first.
    #[arg(long)]
    force: bool,
}

#[derive(Args, Debug)]
struct BreakArgs {
    /// Force a short break.
    #[arg(long, conflicts_with = "long")]
    short: bool,
    /// Force a long break.
    #[arg(long)]
    long: bool,
}

#[derive(Args, Debug)]
struct LogArgs {
    /// Only entries from today (default).
    #[arg(long, conflicts_with_all = ["week", "all"])]
    today: bool,
    /// Only entries from the current ISO week.
    #[arg(long, conflicts_with = "all")]
    week: bool,
    /// All recorded entries.
    #[arg(long)]
    all: bool,
}

#[derive(Args, Debug)]
struct ConfigArgs {
    #[command(subcommand)]
    op: Option<ConfigCmd>,
}

#[derive(Subcommand, Debug)]
enum ConfigCmd {
    /// Print one config value.
    Get { key: String },
    /// Set a config value.
    Set { key: String, value: String },
}

#[derive(Args, Debug)]
struct HelpArgs {
    /// Optional command to get detailed help for.
    command: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    if cli.daemon {
        daemon::run_daemon();
        return;
    }

    let code = match cli.cmd {
        None => {
            commands::cmd_help_overview();
            0
        }
        Some(Cmd::Start(a)) => commands::cmd_start(a.work, a.label, a.force),
        Some(Cmd::Break(a)) => {
            let kind = if a.long {
                BreakKind::Long
            } else if a.short {
                BreakKind::Short
            } else {
                BreakKind::Auto
            };
            commands::cmd_break(kind)
        }
        Some(Cmd::Stop) => commands::cmd_stop(),
        Some(Cmd::Pause) => commands::cmd_pause(),
        Some(Cmd::Resume) => commands::cmd_resume(),
        Some(Cmd::Skip) => commands::cmd_skip(),
        Some(Cmd::Status) => commands::cmd_status(),
        Some(Cmd::Log(a)) => {
            let range = if a.all {
                LogRange::All
            } else if a.week {
                LogRange::Week
            } else {
                LogRange::Today
            };
            commands::cmd_log(range)
        }
        Some(Cmd::Config(a)) => {
            let op = match a.op {
                None => ConfigOp::Show,
                Some(ConfigCmd::Get { key }) => ConfigOp::Get(key),
                Some(ConfigCmd::Set { key, value }) => ConfigOp::Set(key, value),
            };
            commands::cmd_config(op)
        }
        Some(Cmd::Help(a)) => match a.command {
            None => {
                commands::cmd_help_overview();
                0
            }
            Some(name) => {
                use clap::CommandFactory;
                let mut app = Cli::command();
                if let Some(sub) = app.find_subcommand_mut(&name) {
                    let _ = sub.print_long_help();
                    println!();
                    0
                } else {
                    eprintln!("unknown command: {name}");
                    1
                }
            }
        },
    };

    std::process::exit(code);
}
