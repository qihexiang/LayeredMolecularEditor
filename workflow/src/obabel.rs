use std::{io::Write, process::{Command, Stdio}};

use anyhow::{Ok, Result};

pub fn obabel(input: &str, input_format: &str, output_format: &str) -> Result<String> {
    let mut command = Command::new("obabel")
        .args([format!("-i{}", input_format), format!("-o{}", output_format)])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    command.stdin.take().unwrap().write_all(input.as_bytes())?;
    let output = command.wait_with_output()?;
    Ok(String::from_utf8(output.stdout)?)
}