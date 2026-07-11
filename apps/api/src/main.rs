use std::{env, net::SocketAddr, process::ExitCode, sync::Arc, time::Duration};

use jimin_api::{
    ApiState, PairingRuntime,
    auth::Authentication,
    config::{AppConfig, AuthenticationSetting, AuthenticationSettings, SecretSetting},
    probe::{ProbeTarget, run_probe},
    router, serve_with_shutdown,
};
use jimin_application::{PairingLifetime, SessionLifetime, SessionService};
use jimin_auth::{
    AccessTokenIssuer, AccessTokenSettings, AccessTokenVerifier, PairingTokenPepper,
    RefreshTokenPepper,
};
use jimin_observability::init_tracing;
use jimin_storage::Database;
use qrcode::{QrCode, render::unicode};
use secrecy::ExposeSecret;
use tokio::{net::TcpListener, signal};
use tracing::{error, info, warn};

const DEFAULT_PROBE_ADDR: &str = "127.0.0.1:8080";
const MIGRATION_RETRY_INITIAL: Duration = Duration::from_secs(1);
const MIGRATION_RETRY_MAXIMUM: Duration = Duration::from_secs(30);

#[derive(Clone, Copy)]
enum PairingOutput {
    Qr,
    Code,
}

#[tokio::main]
async fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|argument| argument == "probe")
    {
        return run_probe_command(&arguments).await;
    }

    if let Some(output) = pairing_output_for(&arguments) {
        return run_pairing_create_command(output).await;
    }

    if init_tracing().is_err() {
        return ExitCode::FAILURE;
    }

    match run_server().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_code) => {
            error!(event = "api.stopped", error_code);
            ExitCode::FAILURE
        }
    }
}

async fn run_server() -> Result<(), &'static str> {
    let config = AppConfig::load().map_err(|error| error.code())?;

    let (database, configuration_ready) = match config.database_url() {
        SecretSetting::Available(database_url) => {
            if let Ok(database) = Database::connect_lazy(
                database_url,
                config.database_max_connections(),
                config.database_acquire_timeout(),
            ) {
                (Some(database), true)
            } else {
                warn!(
                    event = "storage.configuration_invalid",
                    error_code = "storage.configuration_invalid"
                );
                (None, false)
            }
        }
        SecretSetting::Missing => {
            warn!(
                event = "storage.configuration_missing",
                error_code = "storage.configuration_missing"
            );
            (None, false)
        }
        SecretSetting::Invalid => {
            warn!(
                event = "storage.configuration_invalid",
                error_code = "storage.configuration_invalid"
            );
            (None, false)
        }
    };

    let runtime = database
        .as_ref()
        .and_then(|database| match config.authentication() {
            AuthenticationSetting::Available(settings) => build_authentication(settings, database),
            AuthenticationSetting::Missing => {
                warn!(
                    event = "auth.configuration_missing",
                    error_code = "auth.configuration_missing"
                );
                None
            }
            AuthenticationSetting::Invalid => {
                warn!(
                    event = "auth.configuration_invalid",
                    error_code = "auth.configuration_invalid"
                );
                None
            }
        });
    let configuration_ready = configuration_ready && runtime.is_some();
    let readiness_database = database
        .as_ref()
        .map(|database| Arc::new(database.clone()) as Arc<dyn jimin_api::ReadinessProbe>);
    let mut state = ApiState::new(
        config.build_sha().to_owned(),
        configuration_ready,
        readiness_database,
    );
    if let Some((authentication, pairing)) = runtime {
        state = state.with_authentication(authentication);
        state = state.with_pairing(pairing);
    }
    if let Some(database) = database.as_ref() {
        state = state.with_planning(database.clone());
        state = state.with_agent(database.clone());
    }
    let listener = TcpListener::bind(config.bind_addr())
        .await
        .map_err(|_| "api.bind_failed")?;

    info!(
        event = "api.started",
        service = "jimin-api",
        build_sha = config.build_sha(),
        port = config.bind_addr().port()
    );

    let migration_task = database
        .as_ref()
        .map(|database| tokio::spawn(reconcile_migrations(database.clone())));
    let result = serve_with_shutdown(listener, router(state), shutdown_signal())
        .await
        .map_err(|_| "api.serve_failed");

    if let Some(migration_task) = migration_task {
        migration_task.abort();
        let _ = migration_task.await;
    }
    if let Some(database) = database {
        database.close().await;
    }

    result
}

fn build_authentication(
    settings: &AuthenticationSettings,
    database: &Database,
) -> Option<(Authentication, PairingRuntime)> {
    let access_settings = AccessTokenSettings::new(
        settings.issuer(),
        settings.key_id(),
        settings.access_token_ttl(),
    )
    .ok()?;
    let access_issuer =
        AccessTokenIssuer::from_ed25519_pem(access_settings.clone(), settings.signing_key())
            .ok()?;
    let refresh_pepper = RefreshTokenPepper::new(settings.refresh_pepper().clone()).ok()?;
    let pairing_pepper = PairingTokenPepper::new(settings.pairing_pepper().clone()).ok()?;
    let verifier = AccessTokenVerifier::from_ed25519_pems(
        settings.issuer(),
        [(
            settings.key_id().to_owned(),
            settings.verify_key().expose_secret().to_owned(),
        )],
    )
    .ok()?;
    let session_lifetime = SessionLifetime::new(settings.session_ttl()).ok()?;
    let pairing_lifetime = PairingLifetime::new(Duration::from_mins(10)).ok()?;
    let sessions = SessionService::new(
        database.clone(),
        access_issuer,
        refresh_pepper,
        pairing_pepper,
        session_lifetime,
        pairing_lifetime,
    );
    Some((
        Authentication::new(verifier, Arc::new(database.clone())),
        PairingRuntime::new(sessions),
    ))
}

async fn run_pairing_create_command(output: PairingOutput) -> ExitCode {
    if init_tracing().is_err() {
        return ExitCode::FAILURE;
    }
    let Ok(config) = AppConfig::load() else {
        return ExitCode::FAILURE;
    };
    let (SecretSetting::Available(database_url), AuthenticationSetting::Available(settings)) =
        (config.database_url(), config.authentication())
    else {
        return ExitCode::FAILURE;
    };
    let Ok(database) = Database::connect_lazy(
        database_url,
        config.database_max_connections(),
        config.database_acquire_timeout(),
    ) else {
        return ExitCode::FAILURE;
    };
    let Some((_, pairing)) = build_authentication(settings, &database) else {
        return ExitCode::FAILURE;
    };
    if !matches!(
        database
            .readiness(jimin_storage::EXPECTED_SCHEMA_VERSION)
            .await,
        jimin_storage::Readiness::Ready { .. }
    ) {
        database.close().await;
        return ExitCode::FAILURE;
    }
    let result = pairing.issue_device_pairing().await;
    database.close().await;
    let Ok(issued) = result else {
        return ExitCode::FAILURE;
    };
    let Ok(expires_at) = issued
        .expires_at()
        .format(&time::format_description::well_known::Rfc3339)
    else {
        return ExitCode::FAILURE;
    };
    match output {
        PairingOutput::Qr => {
            let pairing_uri = format!(
                "jimin-os://pair?token={}",
                issued.token().serialized().expose_secret()
            );
            let Ok(pairing_qr) = render_pairing_qr(&pairing_uri) else {
                return ExitCode::FAILURE;
            };
            // This command is a trusted-server bootstrap surface. The pairing
            // URI is intentionally rendered as a QR code instead of being
            // written as text so Android can scan it directly.
            println!(
                "Jimin OS 연결 QR 코드\n\n{pairing_qr}\n이 QR 코드는 한 번만 사용할 수 있으며 {expires_at}에 만료돼요."
            );
        }
        PairingOutput::Code => {
            // macOS has no camera-scanning flow in this phase. Printing a raw
            // one-time code is an explicit recovery path and must never be
            // logged, copied to chat, or used for Android's normal flow.
            println!(
                "Jimin OS 일회용 연결 코드\n{}\n이 코드는 한 번만 사용할 수 있으며 {expires_at}에 만료돼요.",
                issued.token().serialized().expose_secret()
            );
        }
    }
    ExitCode::SUCCESS
}

fn pairing_output_for(arguments: &[String]) -> Option<PairingOutput> {
    match arguments {
        [command, action] if command == "pairing" && action == "create" => Some(PairingOutput::Qr),
        [command, action, option]
            if command == "pairing" && action == "create" && option == "--code" =>
        {
            Some(PairingOutput::Code)
        }
        _ => None,
    }
}

fn render_pairing_qr(pairing_uri: &str) -> Result<String, qrcode::types::QrError> {
    QrCode::new(pairing_uri.as_bytes()).map(|code| code.render::<unicode::Dense1x2>().build())
}

async fn reconcile_migrations(database: Database) {
    let mut retry_delay = MIGRATION_RETRY_INITIAL;
    loop {
        if database.migrate().await.is_ok() {
            info!(event = "storage.migration_ready");
            return;
        }
        warn!(
            event = "storage.migration_unavailable",
            error_code = "storage.migration_unavailable",
            retry_seconds = retry_delay.as_secs()
        );
        tokio::time::sleep(retry_delay).await;
        retry_delay = next_migration_retry(retry_delay);
    }
}

fn next_migration_retry(current: Duration) -> Duration {
    current.saturating_mul(2).min(MIGRATION_RETRY_MAXIMUM)
}

async fn run_probe_command(arguments: &[String]) -> ExitCode {
    let target = match arguments {
        [command, target] if command == "probe" && target == "live" => ProbeTarget::Live,
        [command, target] if command == "probe" && target == "ready" => ProbeTarget::Ready,
        _ => return ExitCode::from(2),
    };
    let address = match env::var("JIMIN_API_PROBE_ADDR") {
        Ok(value) => value.parse::<SocketAddr>(),
        Err(env::VarError::NotPresent) => DEFAULT_PROBE_ADDR.parse(),
        Err(env::VarError::NotUnicode(_)) => return ExitCode::FAILURE,
    };
    let Ok(address) = address else {
        return ExitCode::FAILURE;
    };

    if run_probe(target, address).await.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut stream) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
            stream.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    info!(event = "api.shutdown_requested");
}

#[cfg(test)]
mod tests {
    use super::{
        MIGRATION_RETRY_MAXIMUM, PairingOutput, next_migration_retry, pairing_output_for,
        render_pairing_qr,
    };
    use std::time::Duration;

    #[test]
    fn migration_retry_backoff_is_bounded() {
        assert_eq!(
            next_migration_retry(Duration::from_secs(1)),
            Duration::from_secs(2)
        );
        assert_eq!(
            next_migration_retry(Duration::from_secs(20)),
            MIGRATION_RETRY_MAXIMUM
        );
        assert_eq!(
            next_migration_retry(MIGRATION_RETRY_MAXIMUM),
            MIGRATION_RETRY_MAXIMUM
        );
    }

    #[test]
    fn renders_a_pairing_uri_without_printing_its_raw_value() {
        let pairing_uri = "jimin-os://pair?token=pairing-secret";

        let rendered = render_pairing_qr(pairing_uri).expect("pairing QR should render");

        assert!(!rendered.is_empty());
        assert!(!rendered.contains(pairing_uri));
        assert!(!rendered.contains("pairing-secret"));
    }

    #[test]
    fn pairing_output_uses_qr_by_default_and_requires_an_explicit_code_flag() {
        assert!(matches!(
            pairing_output_for(&["pairing".to_owned(), "create".to_owned()]),
            Some(PairingOutput::Qr)
        ));
        assert!(matches!(
            pairing_output_for(&[
                "pairing".to_owned(),
                "create".to_owned(),
                "--code".to_owned()
            ]),
            Some(PairingOutput::Code)
        ));
    }
}
