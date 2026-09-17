use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

/// Output from a completed process.
#[derive(Debug)]
pub struct ProcessOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Builder for spawning a child process.
pub struct ProcessSpawnOptions {
    command: Command,
    stdin_piped: bool,
    stdout_piped: bool,
    stderr_piped: bool,
}

impl ProcessSpawnOptions {
    pub fn new(program: &str) -> Self {
        Self {
            command: Command::new(program),
            stdin_piped: false,
            stdout_piped: false,
            stderr_piped: false,
        }
    }

    pub fn arg(mut self, arg: &str) -> Self {
        self.command.arg(arg);
        self
    }

    pub fn args(mut self, args: &[&str]) -> Self {
        self.command.args(args);
        self
    }

    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.command.env(key, value);
        self
    }

    pub fn current_dir(mut self, dir: &str) -> Self {
        self.command.current_dir(dir);
        self
    }

    pub fn stdin_piped(mut self) -> Self {
        self.stdin_piped = true;
        self
    }

    pub fn stdout_piped(mut self) -> Self {
        self.stdout_piped = true;
        self
    }

    pub fn stderr_piped(mut self) -> Self {
        self.stderr_piped = true;
        self
    }

    pub fn spawn(self) -> Result<ProcessHandle, std::io::Error> {
        let mut cmd = self.command;

        cmd.stdin(if self.stdin_piped {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        Ok(ProcessHandle {
            child: Some(child),
            stdin,
            stdout_reader: stdout.map(BufReader::new),
            stderr,
        })
    }
}

/// Handle to a running child process.
pub struct ProcessHandle {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout_reader: Option<BufReader<ChildStdout>>,
    stderr: Option<ChildStderr>,
}

impl ProcessHandle {
    /// Write data to the process stdin.
    pub fn write_stdin(&mut self, data: &[u8]) -> Result<(), std::io::Error> {
        if let Some(ref mut stdin) = self.stdin {
            stdin.write_all(data)?;
            stdin.flush()?;
        }
        Ok(())
    }

    /// Close stdin to signal EOF.
    pub fn close_stdin(&mut self) -> Result<(), std::io::Error> {
        self.stdin = None;
        Ok(())
    }

    /// Read a line from stdout (blocking). Returns `None` at EOF.
    pub fn read_stdout_line(&mut self) -> Result<Option<String>, std::io::Error> {
        if let Some(ref mut reader) = self.stdout_reader {
            let mut line = String::new();
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                return Ok(None);
            }
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }
            return Ok(Some(line));
        }
        Ok(None)
    }

    /// Kill the process.
    pub fn kill(&mut self) -> Result<(), std::io::Error> {
        if let Some(ref mut child) = self.child {
            child.kill()?;
        }
        Ok(())
    }

    /// Wait for the process to exit.
    pub fn wait(mut self) -> Result<ProcessOutput, std::io::Error> {
        let mut stdout = String::new();
        let mut stderr = String::new();

        if let Some(ref mut reader) = self.stdout_reader {
            reader.read_to_string(&mut stdout)?;
        }
        if let Some(ref mut err) = self.stderr {
            err.read_to_string(&mut stderr)?;
        }

        self.stdin = None;
        self.stdout_reader = None;
        self.stderr = None;

        if let Some(mut child) = self.child.take() {
            let status = child.wait()?;
            Ok(ProcessOutput {
                exit_code: status.code(),
                stdout,
                stderr,
            })
        } else {
            Ok(ProcessOutput {
                exit_code: None,
                stdout,
                stderr,
            })
        }
    }

    /// Wait with a timeout. Kills the process on timeout.
    pub fn wait_timeout(mut self, timeout: Duration) -> Result<ProcessOutput, std::io::Error> {
        let child = self
            .child
            .as_mut()
            .ok_or_else(|| std::io::Error::other("no child process"))?;

        let start = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let mut stdout = String::new();
                    let mut stderr = String::new();
                    if let Some(ref mut reader) = self.stdout_reader {
                        reader.read_to_string(&mut stdout)?;
                    }
                    if let Some(ref mut err) = self.stderr {
                        err.read_to_string(&mut stderr)?;
                    }
                    self.stdin = None;
                    self.stdout_reader = None;
                    self.stderr = None;
                    return Ok(ProcessOutput {
                        exit_code: status.code(),
                        stdout,
                        stderr,
                    });
                }
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        self.kill()?;
                        let mut stdout = String::new();
                        let mut stderr = String::new();
                        if let Some(ref mut reader) = self.stdout_reader {
                            reader.read_to_string(&mut stdout)?;
                        }
                        if let Some(ref mut err) = self.stderr {
                            err.read_to_string(&mut stderr)?;
                        }
                        self.stdin = None;
                        self.stdout_reader = None;
                        self.stderr = None;
                        if let Some(mut child) = self.child.take() {
                            let _ = child.wait();
                        }
                        return Ok(ProcessOutput {
                            exit_code: None,
                            stdout,
                            stderr,
                        });
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => return Err(e),
            }
        }
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
