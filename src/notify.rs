use std::process::{Command, Stdio};

use libc::geteuid;

/// UID of the user at the physical console (Notification Center / GUI session).
fn console_uid() -> Option<u32> {
    let output = Command::new("/usr/bin/stat")
        .args(["-f", "%u", "/dev/console"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()
}

/// After `setsid` the daemon is reparented under `launchd` and sits outside the user's
/// GUI bootstrap; `display notification` then fails silently. `launchctl asuser` runs the
/// child in the console user's session (only allowed when that UID matches our euid).
fn run_in_user_gui_session(program: &str, args: &[&str]) {
    let euid = unsafe { geteuid() };
    let mut cmd = if console_uid().is_some_and(|c| c == euid) {
        let mut c = Command::new("/bin/launchctl");
        c.args(["asuser", &euid.to_string(), program]);
        c.args(args);
        c
    } else {
        let mut c = Command::new(program);
        c.args(args);
        c
    };
    let _ = cmd
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// AppleScript: argv avoids embedding user text in source (quotes, Unicode). We still try a
/// Notification Center banner, then show a short auto-dismissing dialog — banners are often
/// disabled for `osascript` in System Settings, but the dialog is hard to miss.
const NOTIFY_SCRIPT: &str = r#"
on run argv
	if (count of argv) < 2 then return
	set msg to item 1 of argv
	set ttl to item 2 of argv
	try
		display notification msg with title ttl
	end try
	try
		display dialog msg with title ttl buttons {"OK"} default button "OK" giving up after 6 with icon note
	end try
end run
"#;

fn clip_for_ui(s: &str, max_chars: usize) -> String {
    let n = s.chars().count();
    if n <= max_chars {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    t.push('…');
    t
}

pub fn notify(title: &str, message: &str) {
    let msg = clip_for_ui(message, 480);
    let ttl = clip_for_ui(title, 120);
    run_in_user_gui_session(
        "/usr/bin/osascript",
        &["-e", NOTIFY_SCRIPT.trim(), "--", &msg, &ttl],
    );
}

pub fn play_sound() {
    run_in_user_gui_session(
        "/usr/bin/afplay",
        &["/System/Library/Sounds/Glass.aiff"],
    );
}
