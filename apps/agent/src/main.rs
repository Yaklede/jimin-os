use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use jimin_codex_client::{
    AccountSummary, AppServerProcess, Error, ProcessEnd, StderrStreamState, StderrSummary,
    probe_compatibility,
};
use serde::Serialize;
use serde_json::Value;
use tokio::io::AsyncReadExt;

mod health;

use health::{HealthMarker, HealthMarkerError, HealthState};

const MAX_PROMPT_BYTES: u64 = 64 * 1024;
const MAX_MODEL_ID_BYTES: usize = 128;
const ACCOUNT_PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const TURN_PROBE_TIMEOUT: Duration = Duration::from_mins(3);
const SERVE_START_TIMEOUT: Duration = Duration::from_secs(30);
const SERVE_FAILURE_BUDGET: u8 = 3;
const SERVE_MAX_BACKOFF: Duration = Duration::from_secs(2);
const SERVE_STABLE_WINDOW: Duration = Duration::from_mins(5);

#[derive(Debug, Parser)]
#[command(name = "jimin-agent", disable_help_subcommand = true)]
struct Cli {
    #[arg(
        long,
        env = "JIMIN_AGENT_CODEX_BIN",
        default_value = "codex",
        global = true
    )]
    codex_bin: PathBuf,
    #[arg(
        long,
        env = "JIMIN_AGENT_HEALTH_MARKER",
        default_value = "/tmp/jimin-agent-health",
        global = true
    )]
    health_marker: PathBuf,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Serve,
    Health,
    Probe {
        #[command(subcommand)]
        probe: Probe,
    },
}

#[derive(Debug, Subcommand)]
enum Probe {
    Compatibility,
    Account,
    Turn {
        #[arg(long)]
        prompt_file: PathBuf,
        #[arg(long, env = "JIMIN_AGENT_PROBE_MODEL")]
        model: Option<String>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SuccessEnvelope<T> {
    ok: bool,
    probe: &'static str,
    result: T,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FailureEnvelope {
    ok: bool,
    probe: &'static str,
    error: SafeError,
}

#[derive(Serialize)]
struct SafeError {
    code: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountProbeResult {
    runtime_state: &'static str,
    #[serde(flatten)]
    account: jimin_codex_client::AccountSummary,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TurnProbeResult {
    prompt_bytes: u64,
    #[serde(flatten)]
    turn: jimin_codex_client::TurnSummary,
}

enum ProbeOutput {
    Success {
        probe: &'static str,
        result: Value,
    },
    Failure {
        probe: &'static str,
        code: &'static str,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let Ok(cli) = Cli::try_parse() else {
        return emit_probe_output(ProbeOutput::Failure {
            probe: "unknown",
            code: "invalid_arguments",
        });
    };

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => run_serve(&cli.codex_bin, &cli.health_marker).await,
        Command::Health => run_health(&cli.health_marker),
        Command::Probe { probe } => emit_probe_output(execute_probe(&cli.codex_bin, probe).await),
    }
}

fn emit_probe_output(output: ProbeOutput) -> ExitCode {
    let (value, success) = match output {
        ProbeOutput::Success { probe, result } => (
            serde_json::to_value(SuccessEnvelope {
                ok: true,
                probe,
                result,
            })
            .unwrap_or_else(|_| fallback_serialization_error()),
            true,
        ),
        ProbeOutput::Failure { probe, code } => (
            serde_json::to_value(FailureEnvelope {
                ok: false,
                probe,
                error: SafeError { code },
            })
            .unwrap_or_else(|_| fallback_serialization_error()),
            false,
        ),
    };

    write_single_json_line(&value);
    if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

async fn execute_probe(codex_binary: &Path, probe: Probe) -> ProbeOutput {
    match probe {
        Probe::Compatibility => match probe_compatibility(codex_binary).await {
            Ok(summary) if summary.compatible => success("compatibility", summary),
            Ok(_) => failure_code("compatibility", "codex_version_incompatible"),
            Err(error) => failure("compatibility", &error),
        },
        Probe::Account => {
            match tokio::time::timeout(ACCOUNT_PROBE_TIMEOUT, run_account_probe(codex_binary)).await
            {
                Ok(Ok(result)) => success("account", result),
                Ok(Err(error)) => failure("account", &error),
                Err(_) => failure_code("account", "probe_timeout"),
            }
        }
        Probe::Turn { prompt_file, model } => {
            match tokio::time::timeout(
                TURN_PROBE_TIMEOUT,
                run_turn_probe(codex_binary, &prompt_file, model.as_deref()),
            )
            .await
            {
                Ok(Ok(result)) => success("turn", result),
                Ok(Err(error)) => failure("turn", &error),
                Err(_) => failure_code("turn", "probe_timeout"),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FailureDecision {
    attempt: u8,
    budget: u8,
    exhausted: bool,
    backoff: Duration,
    reset_after_stable_run: bool,
}

struct CrashBudget {
    failures: u8,
    maximum: u8,
    stable_window: Duration,
}

struct ServeStartFailure {
    error: Error,
    process: Option<AppServerProcess>,
}

impl CrashBudget {
    fn new(maximum: u8, stable_window: Duration) -> Self {
        Self {
            failures: 0,
            maximum,
            stable_window,
        }
    }

    fn record_failure(&mut self, uptime: Option<Duration>) -> FailureDecision {
        let reset_after_stable_run = uptime.is_some_and(|value| value >= self.stable_window);
        if reset_after_stable_run {
            self.failures = 0;
        }
        self.failures = self.failures.saturating_add(1).min(self.maximum);
        let exhausted = self.failures >= self.maximum;
        let backoff = if exhausted {
            Duration::ZERO
        } else {
            Duration::from_secs(u64::from(self.failures)).min(SERVE_MAX_BACKOFF)
        };
        FailureDecision {
            attempt: self.failures,
            budget: self.maximum,
            exhausted,
            backoff,
            reset_after_stable_run,
        }
    }
}

fn run_health(marker_path: &Path) -> ExitCode {
    let result = HealthMarker::resolve(marker_path).and_then(|marker| marker.read());
    match result {
        Ok(state) => {
            let acceptable = state.is_acceptable_container_health();
            write_single_json_line(&serde_json::json!({
                "ok": acceptable,
                "mode": "health",
                "state": state.as_str()
            }));
            if acceptable {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            write_single_json_line(&serde_json::json!({
                "ok": false,
                "mode": "health",
                "error": { "code": error.code() }
            }));
            ExitCode::FAILURE
        }
    }
}

async fn run_serve(codex_binary: &Path, marker_path: &Path) -> ExitCode {
    let marker = match initialize_health_marker(marker_path) {
        Ok(marker) => marker,
        Err(error) => {
            write_serve_failure(error.code(), None, None);
            return ExitCode::FAILURE;
        }
    };

    let mut budget = CrashBudget::new(SERVE_FAILURE_BUDGET, SERVE_STABLE_WINDOW);
    loop {
        let startup = tokio::select! {
            signal_result = wait_for_shutdown_signal() => {
                let _ = marker.remove();
                if signal_result.is_ok() {
                    return ExitCode::SUCCESS;
                }
                write_serve_failure("agent_signal_failed", None, None);
                return ExitCode::FAILURE;
            }
            startup = tokio::time::timeout(SERVE_START_TIMEOUT, start_serve(codex_binary)) => startup,
        };

        let (process, state) = match startup {
            Ok(Ok(startup)) => startup,
            Ok(Err(failure)) => {
                let ServeStartFailure { error, process } = failure;
                let stderr = match process {
                    Some(process) => process.shutdown().await.ok(),
                    None => None,
                };
                if let Some(state) = terminal_state_for_error(&error) {
                    return hold_terminal_state(&marker, state).await;
                }
                if let Some(exit_code) =
                    handle_serve_failure(&marker, &mut budget, error.code(), stderr.as_ref(), None)
                        .await
                {
                    return exit_code;
                }
                continue;
            }
            Err(_) => {
                if let Some(exit_code) =
                    handle_serve_failure(&marker, &mut budget, "serve_start_timeout", None, None)
                        .await
                {
                    return exit_code;
                }
                continue;
            }
        };

        if let Err(error) = marker.write(state) {
            let stderr = process.shutdown().await.ok();
            if let Some(exit_code) =
                handle_serve_failure(&marker, &mut budget, error.code(), stderr.as_ref(), None)
                    .await
            {
                return exit_code;
            }
            continue;
        }

        write_single_json_line(&serde_json::json!({
            "ok": true,
            "mode": "serve",
            "state": state.as_str()
        }));

        let run_started_at = Instant::now();
        match process.run_until_shutdown().await {
            Ok(outcome) if outcome.end == ProcessEnd::ShutdownSignal => {
                let _ = marker.remove();
                return ExitCode::SUCCESS;
            }
            Ok(outcome) => {
                if let Some(exit_code) = handle_serve_failure(
                    &marker,
                    &mut budget,
                    Error::AppServerExited.code(),
                    Some(&outcome.stderr),
                    Some(run_started_at.elapsed()),
                )
                .await
                {
                    return exit_code;
                }
            }
            Err(error) => {
                if let Some(exit_code) = handle_serve_failure(
                    &marker,
                    &mut budget,
                    error.code(),
                    None,
                    Some(run_started_at.elapsed()),
                )
                .await
                {
                    return exit_code;
                }
            }
        }
    }
}

fn initialize_health_marker(marker_path: &Path) -> Result<HealthMarker, HealthMarkerError> {
    let marker = HealthMarker::resolve(marker_path)?;
    marker.remove()?;
    Ok(marker)
}

fn terminal_state_for_error(error: &Error) -> Option<HealthState> {
    match error {
        Error::IncompatibleVersion { .. } => Some(HealthState::Incompatible),
        Error::UnsupportedAccountType => Some(HealthState::UnsupportedAccount),
        _ => None,
    }
}

async fn hold_terminal_state(marker: &HealthMarker, state: HealthState) -> ExitCode {
    if let Err(error) = marker.write(state) {
        write_serve_failure(error.code(), None, None);
        return ExitCode::FAILURE;
    }
    write_single_json_line(&serde_json::json!({
        "ok": true,
        "mode": "serve",
        "state": state.as_str()
    }));
    let signal_result = wait_for_shutdown_signal().await;
    let _ = marker.remove();
    if signal_result.is_ok() {
        ExitCode::SUCCESS
    } else {
        write_serve_failure("agent_signal_failed", None, None);
        ExitCode::FAILURE
    }
}

async fn handle_serve_failure(
    marker: &HealthMarker,
    budget: &mut CrashBudget,
    code: &'static str,
    stderr: Option<&StderrSummary>,
    uptime: Option<Duration>,
) -> Option<ExitCode> {
    let _ = marker.remove();
    let decision = budget.record_failure(uptime);
    write_serve_failure(code, stderr, Some(&decision));
    if decision.exhausted {
        return Some(ExitCode::FAILURE);
    }
    match wait_for_shutdown_or_delay(decision.backoff).await {
        Ok(false) => None,
        Ok(true) => {
            let _ = marker.remove();
            Some(ExitCode::SUCCESS)
        }
        Err(_) => {
            write_serve_failure("agent_signal_failed", None, None);
            Some(ExitCode::FAILURE)
        }
    }
}

async fn start_serve(
    codex_binary: &Path,
) -> Result<(AppServerProcess, HealthState), ServeStartFailure> {
    let mut process = AppServerProcess::spawn(codex_binary)
        .await
        .map_err(|error| ServeStartFailure {
            error,
            process: None,
        })?;
    let account_result = async {
        let client = process.client_mut();
        client.initialize().await?;
        let account = client.read_account().await?;
        Ok::<_, Error>(account)
    }
    .await;
    let account = match account_result {
        Ok(account) => account,
        Err(error) => {
            return Err(ServeStartFailure {
                error,
                process: Some(process),
            });
        }
    };
    let state = match health_state_for_account(&account) {
        Ok(state) => state,
        Err(error) => {
            return Err(ServeStartFailure {
                error,
                process: Some(process),
            });
        }
    };
    Ok((process, state))
}

fn health_state_for_account(account: &AccountSummary) -> Result<HealthState, Error> {
    if !account.authenticated {
        return Ok(HealthState::AuthRequired);
    }
    if account.account_type == "chatgpt" {
        Ok(HealthState::Ready)
    } else {
        Err(Error::UnsupportedAccountType)
    }
}

fn write_serve_failure(
    code: &'static str,
    stderr: Option<&StderrSummary>,
    decision: Option<&FailureDecision>,
) {
    write_single_json_line(&serve_failure_value(code, stderr, decision));
}

fn serve_failure_value(
    code: &'static str,
    stderr: Option<&StderrSummary>,
    decision: Option<&FailureDecision>,
) -> Value {
    let mut value = serde_json::json!({
        "ok": false,
        "mode": "serve",
        "error": { "code": code }
    });
    let object = value
        .as_object_mut()
        .expect("serve failure JSON is always an object");
    if let Some(stderr) = stderr {
        object.insert("stderrBytes".to_owned(), stderr.total_bytes.into());
        object.insert("stderrLines".to_owned(), stderr.line_count.into());
        object.insert("stderrOverflowed".to_owned(), stderr.overflowed.into());
        object.insert(
            "stderrReadError".to_owned(),
            (stderr.stream_state == StderrStreamState::ReadError).into(),
        );
        object.insert(
            "stderrDrainTimedOut".to_owned(),
            (stderr.stream_state == StderrStreamState::DrainTimedOut).into(),
        );
        object.insert("childExited".to_owned(), stderr.child_exited.into());
    }
    if let Some(decision) = decision {
        object.insert("failureAttempt".to_owned(), decision.attempt.into());
        object.insert("failureBudget".to_owned(), decision.budget.into());
        object.insert("budgetExhausted".to_owned(), decision.exhausted.into());
        object.insert(
            "failureSequenceReset".to_owned(),
            decision.reset_after_stable_run.into(),
        );
        let retry_millis = u64::try_from(decision.backoff.as_millis()).unwrap_or(u64::MAX);
        object.insert("retryInMs".to_owned(), retry_millis.into());
    }
    value
}

async fn run_account_probe(codex_binary: &Path) -> Result<AccountProbeResult, Error> {
    let mut process = AppServerProcess::spawn(codex_binary).await?;
    let result = async {
        let client = process.client_mut();
        client.initialize().await?;
        let account = client.read_account().await?;
        let runtime_state = if account.authenticated {
            "ready"
        } else {
            "authRequired"
        };
        Ok(AccountProbeResult {
            runtime_state,
            account,
        })
    }
    .await;
    let shutdown_result = process.shutdown().await;
    finish_probe(result, shutdown_result)
}

async fn run_turn_probe(
    codex_binary: &Path,
    prompt_file: &Path,
    model: Option<&str>,
) -> Result<TurnProbeResult, Error> {
    let model = validate_model(model)?;
    let prompt = read_prompt(prompt_file).await.map_err(|source| Error::Io {
        operation: "read_prompt",
        source,
    })?;
    let prompt_bytes = prompt.len() as u64;
    let mut process = AppServerProcess::spawn(codex_binary).await?;
    let workspace = process.workspace().to_path_buf();
    let result = async {
        let client = process.client_mut();
        client.initialize().await?;
        let thread_id = client.start_ephemeral_thread_in(&workspace, model).await?;
        let turn = client.run_turn(&thread_id, &prompt).await?;
        Ok(TurnProbeResult { prompt_bytes, turn })
    }
    .await;
    let shutdown_result = process.shutdown().await;
    finish_probe(result, shutdown_result)
}

fn validate_model(model: Option<&str>) -> Result<Option<&str>, Error> {
    let Some(model) = model else {
        return Ok(None);
    };
    if model.is_empty() || model.len() > MAX_MODEL_ID_BYTES || model.chars().any(char::is_control) {
        return Err(Error::InvalidModel);
    }
    Ok(Some(model))
}

fn finish_probe<T>(
    result: Result<T, Error>,
    shutdown_result: Result<StderrSummary, Error>,
) -> Result<T, Error> {
    match result {
        Ok(value) => {
            shutdown_result?;
            Ok(value)
        }
        Err(error) => Err(error),
    }
}

async fn read_prompt(path: &Path) -> io::Result<String> {
    let file = tokio::fs::File::open(path).await?;
    let mut bytes = Vec::with_capacity(4096);
    file.take(MAX_PROMPT_BYTES + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "empty prompt"));
    }
    if bytes.len() as u64 > MAX_PROMPT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "prompt too large",
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "prompt is not UTF-8"))
}

fn success<T>(probe: &'static str, result: T) -> ProbeOutput
where
    T: Serialize,
{
    match serde_json::to_value(result) {
        Ok(result) => ProbeOutput::Success { probe, result },
        Err(_) => ProbeOutput::Failure {
            probe,
            code: "serialization_failed",
        },
    }
}

fn failure(probe: &'static str, error: &Error) -> ProbeOutput {
    failure_code(probe, error.code())
}

fn failure_code(probe: &'static str, code: &'static str) -> ProbeOutput {
    ProbeOutput::Failure { probe, code }
}

fn fallback_serialization_error() -> Value {
    serde_json::json!({
        "ok": false,
        "probe": "unknown",
        "error": { "code": "serialization_failed" }
    })
}

fn write_single_json_line(value: &Value) {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    let _ = serde_json::to_writer(&mut lock, value);
    let _ = lock.write_all(b"\n");
    let _ = lock.flush();
}

async fn wait_for_shutdown_or_delay(delay: Duration) -> io::Result<bool> {
    tokio::select! {
        signal_result = wait_for_shutdown_signal() => signal_result.map(|()| true),
        () = tokio::time::sleep(delay) => Ok(false),
    }
}

#[cfg(unix)]
async fn wait_for_shutdown_signal() -> io::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    tokio::select! {
        _ = terminate.recv() => Ok(()),
        _ = interrupt.recv() => Ok(()),
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() -> io::Result<()> {
    tokio::signal::ctrl_c().await
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::time::Duration;

    use clap::Parser as _;

    use jimin_codex_client::{StderrStreamState, StderrSummary};

    use super::{
        Cli, Command, CrashBudget, MAX_PROMPT_BYTES, Probe, ProbeOutput, execute_probe,
        health_state_for_account, read_prompt, serve_failure_value, terminal_state_for_error,
        validate_model,
    };

    #[test]
    fn no_subcommand_selects_the_container_serve_contract() {
        let cli = Cli::try_parse_from(["jimin-agent"]).expect("default invocation");
        assert!(cli.command.is_none());

        let cli = Cli::try_parse_from(["jimin-agent", "serve"]).expect("explicit serve");
        assert!(matches!(cli.command, Some(Command::Serve)));

        let cli = Cli::try_parse_from(["jimin-agent", "health"]).expect("health command");
        assert!(matches!(cli.command, Some(Command::Health)));
    }

    #[test]
    fn probe_model_validation_is_bounded_and_content_free() {
        assert_eq!(validate_model(None).expect("optional model"), None);
        assert_eq!(
            validate_model(Some("gpt-fixture")).expect("valid model"),
            Some("gpt-fixture")
        );
        assert!(validate_model(Some("")).is_err());
        assert!(validate_model(Some("invalid\nmodel")).is_err());
        let oversized = "m".repeat(129);
        assert!(validate_model(Some(&oversized)).is_err());
    }

    #[test]
    fn serve_child_exit_exposes_only_content_free_stderr_telemetry() {
        let summary = StderrSummary {
            total_bytes: 1024,
            byte_limit: 1024,
            line_count: 12,
            line_limit: 100,
            overflowed: true,
            stream_state: StderrStreamState::ReadError,
            child_exited: true,
        };
        let mut budget = CrashBudget::new(3, Duration::from_mins(5));
        let decision = budget.record_failure(None);
        let value = serve_failure_value("app_server_exited", Some(&summary), Some(&decision));

        assert_eq!(value["stderrBytes"], 1024);
        assert_eq!(value["stderrLines"], 12);
        assert_eq!(value["stderrOverflowed"], true);
        assert_eq!(value["stderrReadError"], true);
        assert_eq!(value["stderrDrainTimedOut"], false);
        assert_eq!(value["childExited"], true);
        assert_eq!(value["failureAttempt"], 1);
        assert_eq!(value["failureBudget"], 3);
        assert_eq!(value["budgetExhausted"], false);
        assert_eq!(value["failureSequenceReset"], false);
        assert!(!value.to_string().contains("stderr content"));
    }

    #[tokio::test]
    async fn failures_inside_the_stable_window_exhaust_the_consecutive_budget() {
        let mut budget = CrashBudget::new(3, Duration::from_mins(5));
        for expected_attempt in 1..=3 {
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            let decision = budget.record_failure(Some(Duration::from_secs(30)));
            assert_eq!(decision.attempt, expected_attempt);
            assert_eq!(decision.exhausted, expected_attempt == 3);
            assert!(!decision.reset_after_stable_run);
        }
    }

    #[test]
    fn stable_child_run_resets_the_consecutive_failure_budget() {
        let stable_window = Duration::from_mins(5);
        let mut budget = CrashBudget::new(3, stable_window);

        assert_eq!(budget.record_failure(None).attempt, 1);
        assert_eq!(
            budget
                .record_failure(Some(
                    stable_window
                        .checked_sub(Duration::from_secs(1))
                        .expect("stable window is longer than one second"),
                ))
                .attempt,
            2
        );

        let reset = budget.record_failure(Some(stable_window));
        assert_eq!(reset.attempt, 1);
        assert!(!reset.exhausted);
        assert!(reset.reset_after_stable_run);
    }

    #[test]
    fn terminal_adapter_errors_map_to_queryable_agent_states() {
        assert_eq!(
            terminal_state_for_error(&jimin_codex_client::Error::IncompatibleVersion {
                expected: "0.144.1",
                actual: "0.142.3".to_owned(),
            }),
            Some(crate::health::HealthState::Incompatible)
        );
        assert_eq!(
            terminal_state_for_error(&jimin_codex_client::Error::UnsupportedAccountType),
            Some(crate::health::HealthState::UnsupportedAccount)
        );
    }

    #[test]
    fn serve_accepts_only_an_authenticated_chatgpt_account() {
        let summary = |authenticated, account_type| jimin_codex_client::AccountSummary {
            authenticated,
            account_type,
            plan_type: None,
            requires_openai_auth: true,
        };

        assert_eq!(
            health_state_for_account(&summary(false, "none")).expect("login required state"),
            crate::health::HealthState::AuthRequired
        );
        assert_eq!(
            health_state_for_account(&summary(true, "chatgpt")).expect("ChatGPT state"),
            crate::health::HealthState::Ready
        );
        assert!(matches!(
            health_state_for_account(&summary(true, "apiKey")),
            Err(jimin_codex_client::Error::UnsupportedAccountType)
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn incompatible_compatibility_probe_remains_a_failure() {
        use std::os::unix::fs::PermissionsExt as _;

        let path = std::env::temp_dir().join(format!(
            "jimin-agent-incompatible-codex-{}",
            std::process::id()
        ));
        std::fs::write(&path, b"#!/bin/sh\nprintf 'codex-cli 9.9.9\\n'\n")
            .expect("version fixture");
        let mut permissions = std::fs::metadata(&path)
            .expect("fixture metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).expect("fixture permissions");

        let output = execute_probe(&path, Probe::Compatibility).await;
        assert!(matches!(
            output,
            ProbeOutput::Failure {
                probe: "compatibility",
                code: "codex_version_incompatible"
            }
        ));
        std::fs::remove_file(path).expect("fixture cleanup");
    }

    #[tokio::test]
    async fn prompt_reader_accepts_small_generic_utf8_fixture() {
        let mut path = std::env::temp_dir();
        path.push(format!("jimin-agent-prompt-{}.txt", std::process::id()));
        {
            let mut file = std::fs::File::create(&path).expect("fixture create");
            file.write_all(b"Explain why the sky appears blue.")
                .expect("fixture write");
        }

        let prompt = read_prompt(&path).await.expect("valid prompt");
        assert_eq!(prompt, "Explain why the sky appears blue.");
        std::fs::remove_file(path).expect("fixture cleanup");
    }

    #[tokio::test]
    async fn prompt_reader_rejects_empty_and_oversized_files() {
        let mut empty_path = std::env::temp_dir();
        empty_path.push(format!("jimin-agent-empty-{}.txt", std::process::id()));
        std::fs::File::create(&empty_path).expect("empty fixture");
        assert!(read_prompt(&empty_path).await.is_err());
        std::fs::remove_file(empty_path).expect("empty cleanup");

        let mut large_path = std::env::temp_dir();
        large_path.push(format!("jimin-agent-large-{}.txt", std::process::id()));
        {
            let mut file = std::fs::File::create(&large_path).expect("large fixture");
            let fixture_len =
                usize::try_from(MAX_PROMPT_BYTES + 1).expect("prompt bound fits usize");
            file.write_all(&vec![b'a'; fixture_len])
                .expect("large write");
        }
        assert!(read_prompt(&large_path).await.is_err());
        std::fs::remove_file(large_path).expect("large cleanup");
    }
}
