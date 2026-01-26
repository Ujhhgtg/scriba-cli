use std::io::{self, Write};
use std::process::{Command, ExitStatus, Stdio};

use anyhow::bail;

pub fn run_with_output(cmd: &str, args: &[&str]) -> anyhow::Result<ExitStatus> {
    let child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Ok(output) = child.wait_with_output() {
        let _ = io::stdout().write_all(&output.stdout);
        let _ = io::stderr().write_all(&output.stderr);

        return Ok(output.status);
    }

    bail!("failed to run process");
}
