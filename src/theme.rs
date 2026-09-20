use std::process::Command;

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
