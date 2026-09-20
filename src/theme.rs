//! System dark-mode detection.
//!
//! Deepin V23 exposes the desktop color scheme through `gsettings`. We detect it
//! with a one-shot query and, when available, a `gsettings monitor` background
//! thread that reports changes in near real time.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

/// One-shot check of the current dark-mode preference.
pub fn is_dark() -> bool {
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output();
    output
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).contains("prefer-dark"))
        .unwrap_or(false)
}

/// Starts a background `gsettings monitor` thread that sends `true`/`false` on
/// the returned channel whenever the color scheme changes. Returns `None` when
/// `gsettings` is unavailable or the monitor cannot be spawned.
pub fn monitor_dark_mode() -> Option<mpsc::Receiver<bool>> {
    let mut child = Command::new("gsettings")
        .args(["monitor", "org.gnome.desktop.interface", "color-scheme"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line.contains("prefer-dark")).is_err() {
                break;
            }
        }
        let _ = child.wait();
    });
    Some(rx)
}
