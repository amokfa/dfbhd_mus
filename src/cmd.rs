use std::process::{Command, Output, Stdio};
use anyhow::{Result, Context};

pub struct CmdResult {
    pub exit_code: u8,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub trait OutputExt {
    fn stdout_string(&self) -> Result<String>;
    fn stderr_string(&self) -> Result<String>;
    fn check_error(self) -> Result<Output>;
}

impl OutputExt for Output {
    fn stdout_string(&self) -> Result<String> {
        String::from_utf8(self.stdout.clone())
            .context("stdout is not a string")
    }

    fn stderr_string(&self) -> Result<String> {
        String::from_utf8(self.stderr.clone())
            .context("stdout is not a string")
    }

    fn check_error(self) -> Result<Output> {
        if self.status.success() {
            Ok(self)
        } else {
            Err(anyhow::anyhow!("Command failed: {:?} {:?} {:?}", self.status, self.stdout_string(), self.stderr_string()))
        }
    }
}

pub fn cmd(argv: &[&str]) -> Result<Output> {
    let cmd = argv.first().ok_or(anyhow::anyhow!("No command given"))?;
    let args = &argv[1..];
    let result = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to run command: {argv:?}"))?;

    result.wait_with_output()
        .context("wait failed on command: {argv}")
}
