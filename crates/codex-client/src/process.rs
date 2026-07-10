use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;

use crate::client::AppServerClient;
use crate::error::{Error, Result};
use crate::version::ensure_compatible;

const ENV_ALLOWLIST: &[&str] = &[
    "HOME",
    "PATH",
    "CODEX_HOME",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "SSL_CERT_FILE",
    "CODEX_CA_CERTIFICATE",
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
];
const STDERR_BYTE_LIMIT: u64 = 1024 * 1024;
const STDERR_LINE_LIMIT: u64 = 10_000;
const STDERR_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

pub struct AppServerProcess {
    child: Child,
    client: AppServerClient<BufReader<tokio::process::ChildStdout>, tokio::process::ChildStdin>,
    stderr_drain: JoinHandle<()>,
    stderr_telemetry: Arc<StderrTelemetry>,
    workspace: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessEnd {
    ShutdownSignal,
    ChildExited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StderrSummary {
    /// Number of observed bytes, capped at `byte_limit`.
    pub total_bytes: u64,
    pub byte_limit: u64,
    /// Number of observed lines, capped at `line_limit`.
    pub line_count: u64,
    pub line_limit: u64,
    pub overflowed: bool,
    pub stream_state: StderrStreamState,
    pub child_exited: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StderrStreamState {
    Open,
    Closed,
    ReadError,
    DrainTimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessOutcome {
    pub end: ProcessEnd,
    pub stderr: StderrSummary,
    pub drained_notifications: u64,
}

struct StderrTelemetry {
    total_bytes: AtomicU64,
    line_count: AtomicU64,
    overflowed: AtomicBool,
    unterminated_line: AtomicBool,
    stream_closed: AtomicBool,
    read_error: AtomicBool,
    byte_limit: u64,
    line_limit: u64,
}

impl StderrTelemetry {
    fn new(byte_limit: u64, line_limit: u64) -> Self {
        Self {
            total_bytes: AtomicU64::new(0),
            line_count: AtomicU64::new(0),
            overflowed: AtomicBool::new(false),
            unterminated_line: AtomicBool::new(false),
            stream_closed: AtomicBool::new(false),
            read_error: AtomicBool::new(false),
            byte_limit,
            line_limit,
        }
    }

    fn observe_bytes(&self, amount: u64) {
        self.add_capped(&self.total_bytes, amount, self.byte_limit);
    }

    fn observe_lines(&self, amount: u64) {
        self.add_capped(&self.line_count, amount, self.line_limit);
    }

    fn add_capped(&self, counter: &AtomicU64, amount: u64, limit: u64) {
        let mut current = counter.load(Ordering::Relaxed);
        loop {
            let uncapped = current.saturating_add(amount);
            let next = uncapped.min(limit);
            if uncapped > limit {
                self.overflowed.store(true, Ordering::Relaxed);
            }
            match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
    }

    fn snapshot(&self, child_exited: bool, drain_timed_out: bool) -> StderrSummary {
        let mut line_count = self.line_count.load(Ordering::Relaxed);
        let mut overflowed = self.overflowed.load(Ordering::Relaxed);
        if self.unterminated_line.load(Ordering::Relaxed) {
            if line_count < self.line_limit {
                line_count += 1;
            } else {
                overflowed = true;
            }
        }
        let stream_state = if drain_timed_out {
            StderrStreamState::DrainTimedOut
        } else if self.read_error.load(Ordering::Relaxed) {
            StderrStreamState::ReadError
        } else if self.stream_closed.load(Ordering::Relaxed) {
            StderrStreamState::Closed
        } else {
            StderrStreamState::Open
        };
        StderrSummary {
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
            byte_limit: self.byte_limit,
            line_count,
            line_limit: self.line_limit,
            overflowed,
            stream_state,
            child_exited,
        }
    }
}

impl AppServerProcess {
    /// Validates the pinned CLI version and starts the App Server over stdio.
    ///
    /// # Errors
    ///
    /// Returns a typed version, spawn, pipe, or protocol setup error. Server
    /// stderr is drained without exposing its contents.
    pub async fn spawn(codex_binary: &Path) -> Result<Self> {
        ensure_compatible(codex_binary).await?;
        let workspace = resolve_workspace()?;

        let mut command = Command::new(codex_binary);
        command
            .arg("app-server")
            .arg("--listen")
            .arg("stdio://")
            .current_dir(&workspace)
            .env_clear()
            .kill_on_drop(true)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        for key in ENV_ALLOWLIST {
            if let Some(value) = env::var_os(key) {
                command.env(key, value);
            }
        }

        let mut child = command.spawn().map_err(Error::Spawn)?;
        let stdin = child.stdin.take().ok_or_else(|| {
            Error::Spawn(std::io::Error::other("Codex stdin pipe was not created"))
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            Error::Spawn(std::io::Error::other("Codex stdout pipe was not created"))
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            Error::Spawn(std::io::Error::other("Codex stderr pipe was not created"))
        })?;
        let (stderr_drain, stderr_telemetry) =
            spawn_stderr_drain(stderr, STDERR_BYTE_LIMIT, STDERR_LINE_LIMIT);

        Ok(Self {
            child,
            client: AppServerClient::new(BufReader::new(stdout), stdin),
            stderr_drain,
            stderr_telemetry,
            workspace,
        })
    }

    pub fn client_mut(
        &mut self,
    ) -> &mut AppServerClient<BufReader<tokio::process::ChildStdout>, tokio::process::ChildStdin>
    {
        &mut self.client
    }

    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    /// Stops the child process and returns bounded, content-free stderr telemetry.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Shutdown`] if process state cannot be read or the
    /// running child cannot be terminated. No stderr text is retained or returned.
    pub async fn shutdown(mut self) -> Result<StderrSummary> {
        match self.child.try_wait().map_err(Error::Shutdown)? {
            Some(_) => {}
            None => {
                self.child.kill().await.map_err(Error::Shutdown)?;
            }
        }
        Ok(finalize_stderr(self.stderr_drain, &self.stderr_telemetry, true).await)
    }

    /// Keeps the child alive until it exits or receives an interrupt or terminate signal.
    ///
    /// # Errors
    ///
    /// Returns a typed signal, child wait, or shutdown error. Child stderr content is never
    /// returned or logged.
    pub async fn run_until_shutdown(mut self) -> Result<ProcessOutcome> {
        let shutdown_signal = wait_for_shutdown_signal();
        tokio::pin!(shutdown_signal);
        let mut drained_notifications = 0_u64;

        let end = loop {
            tokio::select! {
                signal_result = &mut shutdown_signal => {
                    signal_result.map_err(Error::Signal)?;
                    match self.child.try_wait().map_err(Error::Shutdown)? {
                        Some(_) => {}
                        None => self.child.kill().await.map_err(Error::Shutdown)?,
                    }
                    break ProcessEnd::ShutdownSignal;
                }
                status_result = self.child.wait() => {
                    let _status = status_result.map_err(|source| Error::Io {
                        operation: "wait_for_app_server",
                        source,
                    })?;
                    break ProcessEnd::ChildExited;
                }
                notification_result = self.client.discard_next_notification() => {
                    if let Err(error) = notification_result {
                        match self.child.try_wait().map_err(Error::Shutdown)? {
                            Some(_) => {}
                            None => self.child.kill().await.map_err(Error::Shutdown)?,
                        }
                        let _stderr = finalize_stderr(
                            self.stderr_drain,
                            &self.stderr_telemetry,
                            true,
                        ).await;
                        return Err(error);
                    }
                    drained_notifications = drained_notifications.saturating_add(1);
                }
            }
        };

        let stderr = finalize_stderr(self.stderr_drain, &self.stderr_telemetry, true).await;
        Ok(ProcessOutcome {
            end,
            stderr,
            drained_notifications,
        })
    }
}

fn spawn_stderr_drain<R>(
    mut stderr: R,
    byte_limit: u64,
    line_limit: u64,
) -> (JoinHandle<()>, Arc<StderrTelemetry>)
where
    R: AsyncRead + Send + Unpin + 'static,
{
    let telemetry = Arc::new(StderrTelemetry::new(byte_limit, line_limit));
    let task_telemetry = Arc::clone(&telemetry);
    let task = tokio::spawn(async move {
        let mut buffer = [0_u8; 4096];
        loop {
            match stderr.read(&mut buffer).await {
                Ok(0) => {
                    task_telemetry.stream_closed.store(true, Ordering::Relaxed);
                    break;
                }
                Ok(read) => {
                    let read_u64 = u64::try_from(read).expect("stderr chunk length fits u64");
                    task_telemetry.observe_bytes(read_u64);
                    let mut line_count = 0_u64;
                    for byte in &buffer[..read] {
                        if *byte == b'\n' {
                            line_count = line_count.saturating_add(1);
                        }
                    }
                    task_telemetry.observe_lines(line_count);
                    task_telemetry
                        .unterminated_line
                        .store(buffer[read - 1] != b'\n', Ordering::Relaxed);
                }
                Err(_) => {
                    task_telemetry.read_error.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }
    });
    (task, telemetry)
}

async fn finalize_stderr(
    mut task: JoinHandle<()>,
    telemetry: &StderrTelemetry,
    child_exited: bool,
) -> StderrSummary {
    let drain_timed_out = match tokio::time::timeout(STDERR_DRAIN_TIMEOUT, &mut task).await {
        Ok(Ok(())) => false,
        Ok(Err(_)) => {
            telemetry.read_error.store(true, Ordering::Relaxed);
            false
        }
        Err(_) => true,
    };
    if drain_timed_out {
        task.abort();
    }
    telemetry.snapshot(child_exited, drain_timed_out)
}

fn resolve_workspace() -> Result<PathBuf> {
    let configured = env::var_os("JIMIN_AGENT_WORKSPACE").map(PathBuf::from);
    let workspace = match configured {
        Some(workspace) => workspace,
        None => env::current_dir().map_err(Error::Workspace)?,
    };
    let canonical = workspace.canonicalize().map_err(Error::Workspace)?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(Error::InvalidWorkspace)
    }
}

#[cfg(unix)]
async fn wait_for_shutdown_signal() -> std::io::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    tokio::select! {
        _ = terminate.recv() => Ok(()),
        _ = interrupt.recv() => Ok(()),
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() -> std::io::Result<()> {
    tokio::signal::ctrl_c().await
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncWriteExt;

    use super::{StderrStreamState, spawn_stderr_drain};

    #[tokio::test]
    async fn stderr_flood_is_bounded_and_never_retains_content() {
        let (reader, mut writer) = tokio::io::duplex(256);
        let (drain, telemetry) = spawn_stderr_drain(reader, 64, 4);
        let writer_task = tokio::spawn(async move {
            for _ in 0..4096 {
                writer
                    .write_all(b"SECRET_TOKEN=must-not-escape\n")
                    .await
                    .expect("stderr fixture write");
            }
        });

        writer_task.await.expect("stderr writer task");
        drain.await.expect("stderr drain task");
        let summary = telemetry.snapshot(false, false);

        assert_eq!(summary.total_bytes, 64);
        assert_eq!(summary.byte_limit, 64);
        assert_eq!(summary.line_count, 4);
        assert_eq!(summary.line_limit, 4);
        assert!(summary.overflowed);
        assert_eq!(summary.stream_state, StderrStreamState::Closed);
        assert!(!summary.child_exited);
        assert!(!format!("{summary:?}").contains("must-not-escape"));
    }
}
