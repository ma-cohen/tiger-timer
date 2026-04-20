use std::process::{Command, Stdio};

pub fn notify(title: &str, message: &str) {
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        escape(message),
        escape(title)
    );
    let _ = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

pub fn play_sound() {
    let _ = Command::new("afplay")
        .arg("/System/Library/Sounds/Glass.aiff")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
