use std::{path::Path, process::Command};

pub fn list_devices() -> Vec<String> {
    let output = Command::new("adb")
        .arg("devices")
        .output()
        .unwrap_or_else(|_| panic!("failed to execute adb"));
    let stdout = String::from_utf8_lossy(&output.stdout);

    // skip first line "List of devices attached"
    stdout
        .lines()
        .skip(1)
        .filter_map(|line| {
            let parts: Vec<_> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[1] == "device" {
                Some(parts[0].to_string())
            } else {
                None
            }
        })
        .collect()
}

pub fn shell_run(device: &str, cmd: &str, args: Vec<String>) -> Result<(), String> {
    let status = Command::new("adb")
        .arg("-s")
        .arg(device)
        .arg("shell")
        .arg(cmd)
        .args(&args)
        .status()
        .map_err(|e| format!("failed to execute adb shell: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "adb pull failed with code {}",
            status.code().unwrap_or(-1)
        ))
    }
}

pub fn push(device: &str, local_path: &Path, remote_path: &str) -> Result<(), String> {
    let status = Command::new("adb")
        .arg("-s")
        .arg(device)
        .arg("push")
        .arg(local_path)
        .arg(remote_path)
        .status()
        .map_err(|e| format!("failed to execute adb pull: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "adb pull failed with code {}",
            status.code().unwrap_or(-1)
        ))
    }
}

pub fn pull(device: &str, remote_path: &str, local_path: &Path) -> Result<(), String> {
    let status = Command::new("adb")
        .arg("-s")
        .arg(device)
        .arg("pull")
        .arg(remote_path)
        .arg(local_path)
        .status()
        .map_err(|e| format!("failed to execute adb pull: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "adb pull failed with code {}",
            status.code().unwrap_or(-1)
        ))
    }
}
