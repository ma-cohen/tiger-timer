# Timer Tiger (`tt`)

A tiny Pomodoro CLI for macOS. Start a 25-minute work timer with `tt start`, get a native notification + sound when it's up, and check progress from any terminal with `tt status`.

## Install on a new computer

You need **macOS** (notifications use `osascript` and `afplay`) and a **Rust toolchain** ([install rustup](https://rustup.rs/)).

### Option A: Install from GitHub without cloning (fastest)

This compiles from the default branch and installs the `tt` binary into `~/.cargo/bin`:

```bash
cargo install --git https://github.com/ma-cohen/tiger-timer --locked
```

Use `--locked` so dependency versions match this repo’s `Cargo.lock` (reproducible builds).

### Option B: Clone, then install

```bash
git clone https://github.com/ma-cohen/tiger-timer.git
cd tiger-timer
cargo install --path . --locked
```

### Put `tt` on your `PATH`

`cargo install` places binaries in `~/.cargo/bin`. Add it to your shell (zsh example):

```bash
# in ~/.zshrc — rustup usually adds this line when you install Rust:
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
```

Open a new terminal (or `source ~/.zshrc`), then run `tt --version` or `tt help`.

### Option C: Developers working from a local checkout

If you already have this repository on disk:

```bash
cd /path/to/timer-tiger   # or tiger-timer after clone
cargo install --path . --force
```

Omit `--locked` only if you intentionally want Cargo to resolve newer dependency versions.

## Usage

```bash
tt start                       # 25-minute work pomodoro
tt start --work 50 --label "write report"
tt status                      # what's running, how long left, today's count
tt pause && tt resume
tt skip                        # finish now (fires the notification)
tt stop                        # abort, no notification
tt break                       # short break, or long every 4th
tt break --long
tt log --today                 # today's sessions + totals
tt log --week
tt log --all
tt config                      # show defaults
tt config set work_secs 1800   # 30-minute work sessions
tt help                        # friendly overview
tt help start                  # detailed help for a subcommand
tt --help                      # clap-generated help
```

Only one timer runs at a time across all terminals. When the timer ends, you get a macOS banner ("Timer Tiger - work done") and the Glass system sound.

## Config keys

Stored at `~/.timer-tiger/config.json`. Edit via `tt config set <key> <value>`.

| key          | default | meaning                                 |
| ------------ | ------- | --------------------------------------- |
| `work_secs`  | `1500`  | work pomodoro length (seconds)          |
| `short_secs` | `300`   | short break length                      |
| `long_secs`  | `900`   | long break length                       |
| `long_every` | `4`     | every Nth pomodoro gets a long break    |

## Files

Everything lives under `~/.timer-tiger/`:

- `state.json` — current session (created on start, removed on end).
- `tt.pid` — daemon PID, `flock`-locked for single-instance.
- `tt.log` — daemon stdout/stderr.
- `history.jsonl` — one line per finished/aborted session.
- `config.json` — your overrides.

## How it works

`tt start` writes `state.json` and forks a detached daemon (`setsid`) that takes an exclusive lock on `tt.pid`, then sleeps in 1-second ticks until the timer is done, aborted, or skipped. Pause/resume/stop/skip mutate `state.json`; the daemon picks them up on the next tick. On completion, the daemon shells out to `osascript` for the banner and `afplay /System/Library/Sounds/Glass.aiff` for sound, appends to `history.jsonl`, then exits and cleans up.

## Not yet supported

- Linux / Windows notifications.
- Live TUI countdown (`tt watch`).
- Slack / calendar integrations.
