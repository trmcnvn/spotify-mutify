use anyhow::{anyhow, Result};
use std::process::{Command, Output};

pub fn execute_applescript(script: &str) -> Result<Output> {
    Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|err| anyhow!("Couldn't spawn osascript: {}", err))
}
