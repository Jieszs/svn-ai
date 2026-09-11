use std::{ffi::OsString, io, path::Path, process::Command};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub trait CommandRunner {
    fn run(&self, executable: &Path, args: &[OsString]) -> io::Result<CommandOutput>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessRunner;

impl CommandRunner for ProcessRunner {
    fn run(&self, executable: &Path, args: &[OsString]) -> io::Result<CommandOutput> {
        let output = Command::new(executable).args(args).output()?;
        Ok(CommandOutput {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}
