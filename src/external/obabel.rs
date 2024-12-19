use std::{
    io::Write,
    process::{Command, Stdio},
};

use anyhow::{anyhow, Context, Ok, Result};

pub fn obabel(
    input: &str,
    input_format: &str,
    output_format: &str,
    error_output: bool,
    gen3d: bool
) -> Result<String> {
    let args = if gen3d {
        vec![
            format!("-i{}", input_format),
            format!("-o{}", output_format),
            "--gen3d".to_string()
        ]
    } else {
        vec![
            format!("-i{}", input_format),
            format!("-o{}", output_format),
        ]
    };
    let mut command = Command::new("obabel")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(if error_output {
            Stdio::inherit()
        } else {
            Stdio::null()
        })
        .spawn()
        .with_context(|| "Failed to start openbabel")?;
    command.stdin.take().unwrap().write_all(input.as_bytes())?;
    let output = command.wait_with_output()?;
    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(anyhow!("Failed to convert with openbabel, {:?}", output.status.code()))
    }
}
