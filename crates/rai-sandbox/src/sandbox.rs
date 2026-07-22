//! The Sandbox trait + LocalSandbox + E2bSandbox (feature-gated).
//!
//! Clean-room pattern (non-copyrightable): a pluggable `Sandbox` trait with
//! backends for local (subprocess + approval), E2B (Firecracker microVM),
//! and Daytona (Docker/OCI). The trait shape is inspired by OpenHands's
//! Runtime/ActionExecutionClient pattern (github.com/OpenHands/OpenHands, MIT)
//! — RAI Code's own naming, no literal code.
//!
//! T31: Sandbox trait + LocalSandbox (tokio::process, captures stdout/stderr/exit).
//! T32: run_with_timeout (kills after the timeout, returns timed_out).
//! T33: E2bSandbox stub behind feature=e2b (config-only, no network).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// The output of running a command in a sandbox.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SandboxOutput {
    /// The stdout (captured).
    pub stdout: String,
    /// The stderr (captured).
    pub stderr: String,
    /// The exit code (0 = success, non-zero = failure).
    pub exit_code: Option<i32>,
    /// Whether the command was killed by a timeout.
    pub timed_out: bool,
}

impl SandboxOutput {
    /// Whether the command succeeded (exit 0, not timed out).
    pub fn is_success(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }
}

/// The async Sandbox contract.
///
/// `start` provisions the sandbox (returns an id/URL); `run` executes a
/// command; `run_with_timeout` executes with a deadline; `stop` tears down.
#[async_trait]
pub trait Sandbox: Send + Sync {
    /// Start the sandbox (returns an id or public URL).
    async fn start(&self) -> anyhow::Result<String>;
    /// Run a command (returns the captured output).
    async fn run(&self, cmd: &str) -> anyhow::Result<SandboxOutput>;
    /// Run a command with a timeout (kills on expiry).
    async fn run_with_timeout(&self, cmd: &str, timeout: Duration)
        -> anyhow::Result<SandboxOutput>;
    /// Stop the sandbox.
    async fn stop(&self) -> anyhow::Result<()>;
}

/// A local subprocess sandbox (trusted dev — no isolation beyond the OS).
///
/// Uses `sh -c <cmd>` for shell semantics (pipes, redirects, env). The
/// AgentK security kernel mediates in the full impl; here it's a direct
/// subprocess.
pub struct LocalSandbox {
    /// The working directory (defaults to the current dir).
    pub workdir: std::path::PathBuf,
}

impl LocalSandbox {
    /// Construct a LocalSandbox in the given working directory.
    pub fn new(workdir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            workdir: workdir.into(),
        }
    }

    /// Construct a LocalSandbox in the current directory.
    pub fn cwd() -> Self {
        Self::new(std::env::current_dir().unwrap_or_else(|_| ".".into()))
    }

    /// Run `sh -c <cmd>` and capture stdout + stderr + exit code.
    async fn run_inner(&self, cmd: &str) -> anyhow::Result<SandboxOutput> {
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(&self.workdir)
            .output()
            .await?;
        Ok(SandboxOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            timed_out: false,
        })
    }
}

#[async_trait]
impl Sandbox for LocalSandbox {
    async fn start(&self) -> anyhow::Result<String> {
        // Local sandbox is always "started" — return the workdir as the id.
        Ok(self.workdir.display().to_string())
    }

    async fn run(&self, cmd: &str) -> anyhow::Result<SandboxOutput> {
        self.run_inner(cmd).await
    }

    async fn run_with_timeout(
        &self,
        cmd: &str,
        timeout: Duration,
    ) -> anyhow::Result<SandboxOutput> {
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(&self.workdir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        // Take the piped handles before racing (wait_with_output consumes the
        // child, but we need to kill it on timeout — so we use child.wait() +
        // read the pipes manually).
        let mut stdout = child.stdout.take().expect("stdout piped");
        let mut stderr = child.stderr.take().expect("stderr piped");

        // Race: child.wait() vs the timeout.
        let result: SandboxOutput = tokio::select! {
            status = child.wait() => {
                let status = status?;
                // Read the captured stdout/stderr.
                let mut stdout_buf = Vec::new();
                let mut stderr_buf = Vec::new();
                use tokio::io::AsyncReadExt;
                let _ = stdout.read_to_end(&mut stdout_buf).await;
                let _ = stderr.read_to_end(&mut stderr_buf).await;
                SandboxOutput {
                    stdout: String::from_utf8_lossy(&stdout_buf).to_string(),
                    stderr: String::from_utf8_lossy(&stderr_buf).to_string(),
                    exit_code: status.code(),
                    timed_out: false,
                }
            }
            _ = tokio::time::sleep(timeout) => {
                // Timeout elapsed — kill the child.
                let _ = child.kill().await;
                SandboxOutput {
                    stdout: String::new(),
                    stderr: format!("command timed out after {timeout:?}"),
                    exit_code: None,
                    timed_out: true,
                }
            }
        };
        Ok(result)
    }

    async fn stop(&self) -> anyhow::Result<()> {
        // Local sandbox has no persistent state to stop.
        Ok(())
    }
}

/// E2B sandbox config (feature-gated — no network in tests).
#[cfg(feature = "e2b")]
pub mod e2b {
    use super::*;

    /// Config for an E2B sandbox (Firecracker microVM).
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct E2bConfig {
        /// The E2B API key.
        pub api_key: String,
        /// The template id (e.g. "rai-browser" for app + headless Chromium).
        pub template: String,
        /// An optional timeout for the sandbox.
        pub timeout_secs: Option<u64>,
    }

    impl E2bConfig {
        /// Construct an E2B config.
        pub fn new(api_key: impl Into<String>, template: impl Into<String>) -> Self {
            Self {
                api_key: api_key.into(),
                template: template.into(),
                timeout_secs: None,
            }
        }

        /// Set the timeout.
        pub fn with_timeout(mut self, secs: u64) -> Self {
            self.timeout_secs = Some(secs);
            self
        }
    }

    /// An E2B sandbox (stub — the live REST API wiring lands in the loop phase).
    pub struct E2bSandbox {
        /// The config.
        pub config: E2bConfig,
    }

    impl E2bSandbox {
        /// Construct from a config.
        pub fn new(config: E2bConfig) -> Self {
            Self { config }
        }
    }

    #[async_trait]
    impl Sandbox for E2bSandbox {
        async fn start(&self) -> anyhow::Result<String> {
            // TODO(loop-wiring): POST to E2B REST API to provision a sandbox.
            Err(anyhow::anyhow!(
                "E2bSandbox::start not yet implemented (REST wiring)"
            ))
        }

        async fn run(&self, _cmd: &str) -> anyhow::Result<SandboxOutput> {
            Err(anyhow::anyhow!(
                "E2bSandbox::run not yet implemented (REST wiring)"
            ))
        }

        async fn run_with_timeout(
            &self,
            _cmd: &str,
            _timeout: Duration,
        ) -> anyhow::Result<SandboxOutput> {
            Err(anyhow::anyhow!(
                "E2bSandbox::run_with_timeout not yet implemented (REST wiring)"
            ))
        }

        async fn stop(&self) -> anyhow::Result<()> {
            Err(anyhow::anyhow!(
                "E2bSandbox::stop not yet implemented (REST wiring)"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T31: LocalSandbox runs `echo hi` -> stdout contains "hi", exit 0.
    #[tokio::test]
    async fn local_sandbox_runs_echo() {
        let sb = LocalSandbox::cwd();
        let out = sb.run("echo hi").await.expect("run echo");
        assert!(
            out.stdout.trim().contains("hi"),
            "stdout should contain 'hi': {}",
            out.stdout
        );
        assert_eq!(out.exit_code, Some(0));
        assert!(!out.timed_out);
        assert!(out.is_success());
    }

    /// T31: LocalSandbox::start returns the workdir; stop is a no-op.
    #[tokio::test]
    async fn local_sandbox_start_and_stop() {
        let sb = LocalSandbox::cwd();
        let id = sb.start().await.expect("start");
        assert!(!id.is_empty());
        sb.stop().await.expect("stop");
    }

    /// T32: run_with_timeout kills `sleep 5` after 1s -> timed_out=true.
    #[tokio::test]
    async fn local_sandbox_timeout() {
        let sb = LocalSandbox::cwd();
        let out = sb
            .run_with_timeout("sleep 5", Duration::from_secs(1))
            .await
            .expect("run with timeout");
        assert!(out.timed_out, "should be timed out: {out:?}");
        assert!(!out.is_success());
    }

    /// T32: a failing command returns non-zero exit + stderr.
    #[tokio::test]
    async fn local_sandbox_failure_exit_code() {
        let sb = LocalSandbox::cwd();
        let out = sb.run("false").await.expect("run false");
        assert!(!out.is_success());
        assert_eq!(out.exit_code, Some(1));
    }

    /// T32: a command that writes to stderr captures it.
    #[tokio::test]
    async fn local_sandbox_captures_stderr() {
        let sb = LocalSandbox::cwd();
        let out = sb.run("echo error_msg >&2").await.expect("run");
        assert!(
            out.stderr.trim().contains("error_msg"),
            "stderr should contain 'error_msg': {}",
            out.stderr
        );
    }

    /// T32: a multi-step command (pipe) works via sh -c.
    #[tokio::test]
    async fn local_sandbox_pipe() {
        let sb = LocalSandbox::cwd();
        let out = sb.run("echo hello | tr a-z A-Z").await.expect("run pipe");
        assert_eq!(out.stdout.trim(), "HELLO");
    }

    /// T33: E2bConfig builds (behind feature=e2b, no network).
    #[cfg(feature = "e2b")]
    #[test]
    fn e2b_config_builds() {
        let cfg = e2b::E2bConfig::new("e2b-key", "rai-browser");
        assert_eq!(cfg.api_key, "e2b-key");
        assert_eq!(cfg.template, "rai-browser");
        assert!(cfg.timeout_secs.is_none());

        let cfg2 = e2b::E2bConfig::new("k", "t").with_timeout(300);
        assert_eq!(cfg2.timeout_secs, Some(300));
    }
}
