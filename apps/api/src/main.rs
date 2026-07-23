use std::{env, net::SocketAddr, process::ExitCode, sync::Arc, time::Duration};

use jimin_api::{
    ApiState, PairingRuntime,
    auth::Authentication,
    calendar_oauth::CalendarOAuthRuntime,
    config::{
        AppConfig, AuthenticationSetting, AuthenticationSettings, CalendarOAuthSetting,
        SecretSetting,
    },
    google_chat_oauth::GoogleChatOAuthRuntime,
    probe::{ProbeTarget, run_probe},
    push::PushRuntime,
    router, serve_with_shutdown, spawn_calendar_mutation_worker, spawn_calendar_sync_worker,
    spawn_google_chat_sync_worker, spawn_push_delivery_worker, spawn_webhook_delivery_worker,
    spawn_work_brief_worker,
    webhook::WebhookRuntime,
};
use jimin_application::{PairingLifetime, SessionLifetime, SessionService};
use jimin_auth::{
    AccessTokenIssuer, AccessTokenSettings, AccessTokenVerifier, PairingTokenPepper,
    RefreshTokenPepper,
};
use jimin_observability::init_tracing;
use jimin_storage::Database;
use secrecy::ExposeSecret;
use tokio::{net::TcpListener, signal};
use tracing::{error, info, warn};

const DEFAULT_PROBE_ADDR: &str = "127.0.0.1:8080";
const MIGRATION_RETRY_INITIAL: Duration = Duration::from_secs(1);
const MIGRATION_RETRY_MAXIMUM: Duration = Duration::from_secs(30);

#[tokio::main]
async fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|argument| argument == "probe")
    {
        return run_probe_command(&arguments).await;
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

#[allow(
    clippy::too_many_lines,
    reason = "Startup keeps dependency, optional integration, and shutdown lifetimes visible in one audited sequence."
)]
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
    let webhook_runtime = match config.authentication() {
        AuthenticationSetting::Available(settings) => {
            WebhookRuntime::new(settings.pairing_pepper()).ok()
        }
        AuthenticationSetting::Missing | AuthenticationSetting::Invalid => None,
    };
    let push_runtime = match (config.authentication(), config.firebase_service_account()) {
        (AuthenticationSetting::Available(settings), SecretSetting::Available(service_account)) => {
            if let Ok(runtime) = PushRuntime::new(settings.pairing_pepper(), service_account) {
                Some(runtime)
            } else {
                warn!(
                    event = "push.configuration_invalid",
                    error_code = "push.configuration_invalid"
                );
                None
            }
        }
        (_, SecretSetting::Invalid) => {
            warn!(
                event = "push.configuration_invalid",
                error_code = "push.configuration_invalid"
            );
            None
        }
        _ => {
            info!(
                event = "push.configuration_missing",
                error_code = "push.configuration_missing"
            );
            None
        }
    };
    let configuration_ready = configuration_ready && runtime.is_some();
    let readiness_database = database
        .as_ref()
        .map(|database| Arc::new(database.clone()) as Arc<dyn jimin_api::ReadinessProbe>);
    let mut state = ApiState::new(
        config.build_sha().to_owned(),
        configuration_ready,
        readiness_database,
    )
    .with_trusted_network(config.trusted_network());
    if let Some((authentication, pairing)) = runtime {
        state = state.with_authentication(authentication);
        state = state.with_pairing(pairing);
    }
    if let Some(webhook_runtime) = webhook_runtime {
        state = state.with_webhook_runtime(webhook_runtime);
    }
    if let Some(push_runtime) = push_runtime {
        state = state.with_push_runtime(push_runtime);
    }
    match config.calendar_oauth() {
        CalendarOAuthSetting::Available(settings) => {
            if let Ok(calendar_oauth) = CalendarOAuthRuntime::new(settings) {
                state = state.with_calendar_oauth(calendar_oauth);
            } else {
                warn!(
                    event = "calendar.configuration_invalid",
                    error_code = "calendar.configuration_invalid"
                );
            }
            if let Ok(google_chat_oauth) = GoogleChatOAuthRuntime::new(settings) {
                state = state.with_google_chat_oauth(google_chat_oauth);
            } else {
                warn!(
                    event = "google_chat.configuration_invalid",
                    error_code = "google_chat.configuration_invalid"
                );
            }
        }
        CalendarOAuthSetting::Missing => info!(
            event = "calendar.configuration_missing",
            error_code = "calendar.configuration_missing"
        ),
        CalendarOAuthSetting::Invalid => warn!(
            event = "calendar.configuration_invalid",
            error_code = "calendar.configuration_invalid"
        ),
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
    let calendar_sync_task = spawn_calendar_sync_worker(&state);
    let calendar_mutation_task = spawn_calendar_mutation_worker(&state);
    let webhook_delivery_task = spawn_webhook_delivery_worker(&state);
    let google_chat_sync_task = spawn_google_chat_sync_worker(&state);
    let push_delivery_task = spawn_push_delivery_worker(&state);
    let work_brief_task = spawn_work_brief_worker(&state);
    let result = serve_with_shutdown(listener, router(state), shutdown_signal())
        .await
        .map_err(|_| "api.serve_failed");

    if let Some(migration_task) = migration_task {
        migration_task.abort();
        let _ = migration_task.await;
    }
    if let Some(calendar_sync_task) = calendar_sync_task {
        calendar_sync_task.abort();
        let _ = calendar_sync_task.await;
    }
    if let Some(calendar_mutation_task) = calendar_mutation_task {
        calendar_mutation_task.abort();
        let _ = calendar_mutation_task.await;
    }
    if let Some(webhook_delivery_task) = webhook_delivery_task {
        webhook_delivery_task.abort();
        let _ = webhook_delivery_task.await;
    }
    if let Some(google_chat_sync_task) = google_chat_sync_task {
        google_chat_sync_task.abort();
        let _ = google_chat_sync_task.await;
    }
    if let Some(push_delivery_task) = push_delivery_task {
        push_delivery_task.abort();
        let _ = push_delivery_task.await;
    }
    if let Some(work_brief_task) = work_brief_task {
        work_brief_task.abort();
        let _ = work_brief_task.await;
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
    use super::{MIGRATION_RETRY_MAXIMUM, next_migration_retry};
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
}
