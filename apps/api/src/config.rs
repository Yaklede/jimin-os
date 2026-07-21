use std::{
    env::{self, VarError},
    fs,
    net::SocketAddr,
    path::Path,
    time::Duration,
};

use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_BUILD_SHA: &str = "dev";
const DEFAULT_MAX_CONNECTIONS: u32 = 5;
const DEFAULT_ACQUIRE_TIMEOUT_MS: u64 = 2_000;
const DEFAULT_ACCESS_TOKEN_TTL_SECONDS: u64 = 10 * 60;
const DEFAULT_SESSION_TTL_SECONDS: u64 = 30 * 24 * 60 * 60;
const MAX_SECRET_FILE_BYTES: u64 = 16 * 1024;
const MAX_CALENDAR_KEY_BYTES: usize = 16 * 1024;
const DEFAULT_CALENDAR_ENCRYPTION_KEY_VERSION: i32 = 1;

pub struct AppConfig {
    bind_addr: SocketAddr,
    build_sha: String,
    database_url: SecretSetting,
    database_max_connections: u32,
    database_acquire_timeout: Duration,
    trusted_network: bool,
    authentication: AuthenticationSetting,
    calendar_oauth: CalendarOAuthSetting,
    firebase_service_account: SecretSetting,
}

pub enum SecretSetting {
    Available(SecretString),
    Missing,
    Invalid,
}

pub struct AuthenticationSettings {
    issuer: String,
    key_id: String,
    access_token_ttl: Duration,
    session_ttl: Duration,
    signing_key: SecretString,
    verify_key: SecretString,
    refresh_pepper: SecretString,
    pairing_pepper: SecretString,
}

pub enum AuthenticationSetting {
    Available(AuthenticationSettings),
    Missing,
    Invalid,
}

/// Deployment-owned Google Calendar OAuth configuration. It is optional so a
/// personal server can start before Google credentials have been provisioned,
/// but partial configuration is never treated as usable.
pub struct CalendarOAuthSettings {
    client_id: String,
    client_secret: SecretString,
    redirect_uri: String,
    encryption_key: SecretString,
    encryption_key_version: i32,
}

pub enum CalendarOAuthSetting {
    Available(CalendarOAuthSettings),
    Missing,
    Invalid,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("API bind address configuration is invalid")]
    InvalidBindAddress,
    #[error("build metadata configuration is invalid")]
    InvalidBuildSha,
    #[error("database pool configuration is invalid")]
    InvalidDatabasePool,
    #[error("trusted-network configuration is invalid")]
    InvalidTrustedNetwork,
    #[error("authentication configuration is invalid")]
    InvalidAuthentication,
    #[error("environment configuration contains non-Unicode data")]
    NonUnicodeEnvironment,
}

impl ConfigError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidBindAddress => "config.bind_address_invalid",
            Self::InvalidBuildSha => "config.build_sha_invalid",
            Self::InvalidDatabasePool => "config.database_pool_invalid",
            Self::InvalidTrustedNetwork => "config.trusted_network_invalid",
            Self::InvalidAuthentication => "config.authentication_invalid",
            Self::NonUnicodeEnvironment => "config.environment_non_unicode",
        }
    }
}

impl AppConfig {
    /// Loads and validates non-secret settings and resolves the database secret.
    ///
    /// # Errors
    ///
    /// Returns a classified configuration error for malformed listener, build,
    /// pool, or non-Unicode environment values. A missing database secret is
    /// represented as an unready setting so liveness can still start.
    pub fn load() -> Result<Self, ConfigError> {
        let bind_addr = env_string("JIMIN_API_BIND_ADDR")?
            .unwrap_or_else(|| DEFAULT_BIND_ADDR.to_owned())
            .parse()
            .map_err(|_| ConfigError::InvalidBindAddress)?;

        let build_sha =
            env_string("JIMIN_BUILD_SHA")?.unwrap_or_else(|| DEFAULT_BUILD_SHA.to_owned());
        if !valid_build_sha(&build_sha) {
            return Err(ConfigError::InvalidBuildSha);
        }

        let database_max_connections = parse_bounded_u32(
            env_string("JIMIN_DATABASE_MAX_CONNECTIONS")?,
            DEFAULT_MAX_CONNECTIONS,
            1,
            100,
        )?;
        let acquire_timeout_ms = parse_bounded_u64(
            env_string("JIMIN_DATABASE_ACQUIRE_TIMEOUT_MS")?,
            DEFAULT_ACQUIRE_TIMEOUT_MS,
            100,
            60_000,
        )?;
        let trusted_network =
            parse_boolean(env_string("JIMIN_TRUSTED_NETWORK")?.as_deref(), false)?;

        let database_url = match (env_string("DATABASE_URL"), env_string("DATABASE_URL_FILE")) {
            (Ok(direct), Ok(file)) => resolve_secret(direct, file, read_secret_file),
            _ => SecretSetting::Invalid,
        };
        let authentication = AuthenticationSetting::load()?;
        let calendar_oauth = CalendarOAuthSetting::load()?;
        let firebase_service_account = firebase_service_account()?;

        Ok(Self {
            bind_addr,
            build_sha,
            database_url,
            database_max_connections,
            database_acquire_timeout: Duration::from_millis(acquire_timeout_ms),
            trusted_network,
            authentication,
            calendar_oauth,
            firebase_service_account,
        })
    }

    #[must_use]
    pub const fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    #[must_use]
    pub fn build_sha(&self) -> &str {
        &self.build_sha
    }

    #[must_use]
    pub const fn database_url(&self) -> &SecretSetting {
        &self.database_url
    }

    #[must_use]
    pub const fn database_max_connections(&self) -> u32 {
        self.database_max_connections
    }

    #[must_use]
    pub const fn database_acquire_timeout(&self) -> Duration {
        self.database_acquire_timeout
    }

    #[must_use]
    pub const fn trusted_network(&self) -> bool {
        self.trusted_network
    }

    #[must_use]
    pub const fn authentication(&self) -> &AuthenticationSetting {
        &self.authentication
    }

    #[must_use]
    pub const fn calendar_oauth(&self) -> &CalendarOAuthSetting {
        &self.calendar_oauth
    }

    #[must_use]
    pub const fn firebase_service_account(&self) -> &SecretSetting {
        &self.firebase_service_account
    }
}

fn firebase_service_account() -> Result<SecretSetting, ConfigError> {
    let file = env_string("JIMIN_FIREBASE_SERVICE_ACCOUNT_FILE")?;
    Ok(resolve_secret(None, file, read_secret_file))
}

impl CalendarOAuthSetting {
    fn load() -> Result<Self, ConfigError> {
        let client_id = env_string("JIMIN_GOOGLE_CALENDAR_CLIENT_ID")?;
        let redirect_uri = env_string("JIMIN_GOOGLE_CALENDAR_REDIRECT_URI")?;
        let encryption_key_version = env_string("JIMIN_CALENDAR_ENCRYPTION_KEY_VERSION")?;
        let client_secret = secret_from_environment("JIMIN_GOOGLE_CALENDAR_CREDENTIAL")?;
        let encryption_key = secret_from_environment("JIMIN_CALENDAR_ENCRYPTION_KEY")?;

        Ok(calendar_oauth_from_values(
            client_id,
            redirect_uri,
            encryption_key_version,
            client_secret,
            encryption_key,
        ))
    }
}

fn calendar_oauth_from_values(
    client_id: Option<String>,
    redirect_uri: Option<String>,
    encryption_key_version: Option<String>,
    client_secret: SecretSetting,
    encryption_key: SecretSetting,
) -> CalendarOAuthSetting {
    let any_value_present = client_id.is_some()
        || redirect_uri.is_some()
        || encryption_key_version.is_some()
        || !matches!(client_secret, SecretSetting::Missing)
        || !matches!(encryption_key, SecretSetting::Missing);
    if !any_value_present {
        return CalendarOAuthSetting::Missing;
    }
    let (Some(client_id), Some(redirect_uri)) = (client_id, redirect_uri) else {
        return CalendarOAuthSetting::Invalid;
    };
    let (SecretSetting::Available(client_secret), SecretSetting::Available(encryption_key)) =
        (client_secret, encryption_key)
    else {
        return CalendarOAuthSetting::Invalid;
    };
    let encryption_key_version = encryption_key_version
        .map_or(Some(DEFAULT_CALENDAR_ENCRYPTION_KEY_VERSION), |value| {
            value.parse::<i32>().ok()
        })
        .filter(|value| *value > 0);
    let Some(encryption_key_version) = encryption_key_version else {
        return CalendarOAuthSetting::Invalid;
    };
    if !valid_google_client_id(&client_id)
        || !valid_calendar_redirect_uri(&redirect_uri)
        || !valid_calendar_key(&encryption_key)
    {
        return CalendarOAuthSetting::Invalid;
    }
    CalendarOAuthSetting::Available(CalendarOAuthSettings {
        client_id,
        client_secret,
        redirect_uri,
        encryption_key,
        encryption_key_version,
    })
}

impl CalendarOAuthSettings {
    #[must_use]
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    #[must_use]
    pub const fn client_secret(&self) -> &SecretString {
        &self.client_secret
    }

    #[must_use]
    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    #[must_use]
    pub const fn encryption_key(&self) -> &SecretString {
        &self.encryption_key
    }

    #[must_use]
    pub const fn encryption_key_version(&self) -> i32 {
        self.encryption_key_version
    }
}

impl AuthenticationSetting {
    fn load() -> Result<Self, ConfigError> {
        let issuer = env_string("JIMIN_AUTH_ISSUER")?;
        let key_id = env_string("JIMIN_AUTH_KEY_ID")?;
        let signing_key = secret_from_environment("JIMIN_AUTH_SIGNING_KEY")?;
        let verify_key = secret_from_environment("JIMIN_AUTH_VERIFY_KEY")?;
        let refresh_pepper = secret_from_environment("JIMIN_AUTH_REFRESH_PEPPER")?;
        let pairing_pepper = secret_from_environment("JIMIN_AUTH_PAIRING_PEPPER")?;

        let any_invalid = [&signing_key, &verify_key, &refresh_pepper, &pairing_pepper]
            .iter()
            .any(|setting| matches!(setting, SecretSetting::Invalid));
        if any_invalid {
            return Ok(Self::Invalid);
        }
        let (Some(issuer), Some(key_id)) = (issuer, key_id) else {
            return Ok(Self::Missing);
        };
        let (
            SecretSetting::Available(signing_key),
            SecretSetting::Available(verify_key),
            SecretSetting::Available(refresh_pepper),
            SecretSetting::Available(pairing_pepper),
        ) = (signing_key, verify_key, refresh_pepper, pairing_pepper)
        else {
            return Ok(Self::Missing);
        };
        if !valid_auth_text(&issuer) || !valid_auth_text(&key_id) {
            return Ok(Self::Invalid);
        }
        let access_token_ttl = parse_bounded_u64(
            env_string("JIMIN_AUTH_ACCESS_TOKEN_TTL_SECONDS")?,
            DEFAULT_ACCESS_TOKEN_TTL_SECONDS,
            60,
            15 * 60,
        )
        .map_err(|_| ConfigError::InvalidAuthentication)?;
        let session_ttl = parse_bounded_u64(
            env_string("JIMIN_AUTH_SESSION_TTL_SECONDS")?,
            DEFAULT_SESSION_TTL_SECONDS,
            60 * 60,
            90 * 24 * 60 * 60,
        )
        .map_err(|_| ConfigError::InvalidAuthentication)?;

        Ok(Self::Available(AuthenticationSettings {
            issuer,
            key_id,
            access_token_ttl: Duration::from_secs(access_token_ttl),
            session_ttl: Duration::from_secs(session_ttl),
            signing_key,
            verify_key,
            refresh_pepper,
            pairing_pepper,
        }))
    }
}

impl AuthenticationSettings {
    #[must_use]
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    #[must_use]
    pub const fn access_token_ttl(&self) -> Duration {
        self.access_token_ttl
    }

    #[must_use]
    pub const fn session_ttl(&self) -> Duration {
        self.session_ttl
    }

    #[must_use]
    pub const fn signing_key(&self) -> &SecretString {
        &self.signing_key
    }

    #[must_use]
    pub const fn verify_key(&self) -> &SecretString {
        &self.verify_key
    }

    #[must_use]
    pub const fn refresh_pepper(&self) -> &SecretString {
        &self.refresh_pepper
    }

    #[must_use]
    pub const fn pairing_pepper(&self) -> &SecretString {
        &self.pairing_pepper
    }
}

fn env_string(key: &str) -> Result<Option<String>, ConfigError> {
    match env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(_)) => Err(ConfigError::NonUnicodeEnvironment),
    }
}

fn resolve_secret<F>(direct: Option<String>, file: Option<String>, read_file: F) -> SecretSetting
where
    F: FnOnce(&str) -> Result<String, ()>,
{
    match (direct, file) {
        (Some(_), Some(_)) => SecretSetting::Invalid,
        (None, None) => SecretSetting::Missing,
        (Some(value), None) => to_secret(value),
        (None, Some(path)) => {
            if path.is_empty() || !Path::new(&path).is_absolute() {
                return SecretSetting::Invalid;
            }
            read_file(&path).map_or(SecretSetting::Invalid, to_secret)
        }
    }
}

fn secret_from_environment(name: &str) -> Result<SecretSetting, ConfigError> {
    let direct = env_string(name)?;
    let file = env_string(&format!("{name}_FILE"))?;
    Ok(resolve_secret(direct, file, read_secret_file))
}

fn read_secret_file(path: &str) -> Result<String, ()> {
    let metadata = fs::metadata(path).map_err(|_| ())?;
    if metadata.len() == 0 || metadata.len() > MAX_SECRET_FILE_BYTES || !metadata.is_file() {
        return Err(());
    }

    fs::read_to_string(path).map_err(|_| ())
}

fn to_secret(mut value: String) -> SecretSetting {
    while value.ends_with('\n') || value.ends_with('\r') {
        value.pop();
    }

    if value.is_empty() || value.contains('\0') {
        SecretSetting::Invalid
    } else {
        SecretSetting::Available(SecretString::from(value))
    }
}

fn valid_build_sha(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_auth_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 255 && !value.chars().any(char::is_control)
}

fn valid_google_client_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 255
        && !value.chars().any(char::is_control)
        && !value.chars().any(char::is_whitespace)
}

fn valid_calendar_redirect_uri(value: &str) -> bool {
    value.len() <= 2_048
        && !value.chars().any(char::is_control)
        && (value.starts_with("https://") || value.starts_with("http://localhost"))
        && !value.contains('#')
}

fn valid_calendar_key(value: &SecretString) -> bool {
    let value = value.expose_secret();
    value.len() >= 32 && value.len() <= MAX_CALENDAR_KEY_BYTES && !value.contains('\0')
}

fn parse_boolean(value: Option<&str>, default: bool) -> Result<bool, ConfigError> {
    match value {
        None => Ok(default),
        Some("1" | "true") => Ok(true),
        Some("0" | "false") => Ok(false),
        Some(_) => Err(ConfigError::InvalidTrustedNetwork),
    }
}

fn parse_bounded_u32(
    value: Option<String>,
    default: u32,
    minimum: u32,
    maximum: u32,
) -> Result<u32, ConfigError> {
    let parsed = value
        .map_or(Ok(default), |value| value.parse())
        .map_err(|_| ConfigError::InvalidDatabasePool)?;
    if (minimum..=maximum).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(ConfigError::InvalidDatabasePool)
    }
}

fn parse_bounded_u64(
    value: Option<String>,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> Result<u64, ConfigError> {
    let parsed = value
        .map_or(Ok(default), |value| value.parse())
        .map_err(|_| ConfigError::InvalidDatabasePool)?;
    if (minimum..=maximum).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(ConfigError::InvalidDatabasePool)
    }
}

#[cfg(test)]
mod tests {
    use secrecy::ExposeSecret;

    use super::*;

    #[test]
    fn direct_secret_is_supported() {
        let setting = resolve_secret(Some("postgres://db".to_owned()), None, |_| Err(()));

        let SecretSetting::Available(secret) = setting else {
            panic!("direct value should be accepted");
        };
        assert_eq!(secret.expose_secret(), "postgres://db");
    }

    #[test]
    fn file_secret_strips_only_line_endings() {
        let setting = resolve_secret(None, Some("/run/secrets/database".to_owned()), |_| {
            Ok("postgres://db\r\n".to_owned())
        });

        let SecretSetting::Available(secret) = setting else {
            panic!("file value should be accepted");
        };
        assert_eq!(secret.expose_secret(), "postgres://db");
    }

    #[test]
    fn direct_and_file_secret_is_rejected() {
        let setting = resolve_secret(
            Some("postgres://direct".to_owned()),
            Some("/run/secrets/database".to_owned()),
            |_| Ok("postgres://file".to_owned()),
        );

        assert!(matches!(setting, SecretSetting::Invalid));
    }

    #[test]
    fn relative_secret_path_is_rejected() {
        let setting = resolve_secret(None, Some("relative/path".to_owned()), |_| {
            Ok("postgres://db".to_owned())
        });

        assert!(matches!(setting, SecretSetting::Invalid));
    }

    #[test]
    fn build_sha_is_strictly_bounded() {
        assert!(valid_build_sha("abc-123_test.sha"));
        assert!(!valid_build_sha("abc 123"));
        assert!(!valid_build_sha(""));
    }

    #[test]
    fn trusted_network_flag_accepts_only_explicit_boolean_values() {
        assert!(matches!(parse_boolean(None, false), Ok(false)));
        assert!(matches!(parse_boolean(Some("1"), false), Ok(true)));
        assert!(matches!(parse_boolean(Some("true"), false), Ok(true)));
        assert!(matches!(parse_boolean(Some("0"), true), Ok(false)));
        assert!(matches!(parse_boolean(Some("false"), true), Ok(false)));
        assert!(matches!(
            parse_boolean(Some("yes"), false),
            Err(ConfigError::InvalidTrustedNetwork)
        ));
    }

    #[test]
    fn calendar_oauth_configuration_distinguishes_missing_partial_and_ready() {
        let missing = calendar_oauth_from_values(
            None,
            None,
            None,
            SecretSetting::Missing,
            SecretSetting::Missing,
        );
        assert!(matches!(missing, CalendarOAuthSetting::Missing));

        let partial = calendar_oauth_from_values(
            Some("client.apps.googleusercontent.com".to_owned()),
            None,
            None,
            SecretSetting::Missing,
            SecretSetting::Missing,
        );
        assert!(matches!(partial, CalendarOAuthSetting::Invalid));

        let ready = calendar_oauth_from_values(
            Some("client.apps.googleusercontent.com".to_owned()),
            Some("https://localhost:8443/oauth/google/calendar/callback".to_owned()),
            Some("2".to_owned()),
            SecretSetting::Available(SecretString::from("calendar-client-secret")),
            SecretSetting::Available(SecretString::from(
                "calendar-encryption-key-with-more-than-32-bytes",
            )),
        );
        let CalendarOAuthSetting::Available(settings) = ready else {
            panic!("complete configuration should be ready");
        };
        assert_eq!(settings.encryption_key_version(), 2);
    }
}
