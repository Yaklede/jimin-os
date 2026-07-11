use std::{
    env::{self, VarError},
    fs,
    path::Path,
    time::Duration,
};

use jimin_storage::Database;
use secrecy::SecretString;
use thiserror::Error;

const DEFAULT_MAX_CONNECTIONS: u32 = 2;
const DEFAULT_ACQUIRE_TIMEOUT_MS: u64 = 2_000;
const DEFAULT_CLAIM_LEASE_SECONDS: u64 = 30;
const DEFAULT_POLL_INTERVAL_MS: u64 = 500;
const MAX_SECRET_FILE_BYTES: u64 = 16 * 1024;

pub(crate) struct AgentConfig {
    database_url: SecretString,
    database_max_connections: u32,
    database_acquire_timeout: Duration,
    claim_lease: Duration,
    poll_interval: Duration,
    runner_id: String,
}

#[derive(Debug, Clone, Copy, Error)]
pub(crate) enum ConfigError {
    #[error("agent database configuration is invalid")]
    InvalidDatabase,
    #[error("agent runner configuration is invalid")]
    InvalidRunner,
    #[error("agent environment contains non-Unicode data")]
    NonUnicodeEnvironment,
}

impl ConfigError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::InvalidDatabase => "agent_database_configuration_invalid",
            Self::InvalidRunner => "agent_runner_configuration_invalid",
            Self::NonUnicodeEnvironment => "agent_environment_non_unicode",
        }
    }
}

impl AgentConfig {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        let database_url = read_database_url()?;
        let database_max_connections = parse_bounded_u32(
            env_string("JIMIN_AGENT_DATABASE_MAX_CONNECTIONS")?,
            DEFAULT_MAX_CONNECTIONS,
            1,
            10,
            ConfigError::InvalidDatabase,
        )?;
        let database_acquire_timeout = Duration::from_millis(parse_bounded_u64(
            env_string("JIMIN_AGENT_DATABASE_ACQUIRE_TIMEOUT_MS")?,
            DEFAULT_ACQUIRE_TIMEOUT_MS,
            100,
            60_000,
            ConfigError::InvalidDatabase,
        )?);
        let claim_lease = Duration::from_secs(parse_bounded_u64(
            env_string("JIMIN_AGENT_CLAIM_LEASE_SECONDS")?,
            DEFAULT_CLAIM_LEASE_SECONDS,
            5,
            5 * 60,
            ConfigError::InvalidRunner,
        )?);
        let poll_interval = Duration::from_millis(parse_bounded_u64(
            env_string("JIMIN_AGENT_POLL_INTERVAL_MS")?,
            DEFAULT_POLL_INTERVAL_MS,
            100,
            60_000,
            ConfigError::InvalidRunner,
        )?);
        let runner_id = env_string("JIMIN_AGENT_RUNNER_ID")?
            .unwrap_or_else(|| format!("agent-{}", uuid::Uuid::now_v7()));
        if !valid_runner_id(&runner_id) {
            return Err(ConfigError::InvalidRunner);
        }

        Ok(Self {
            database_url,
            database_max_connections,
            database_acquire_timeout,
            claim_lease,
            poll_interval,
            runner_id,
        })
    }

    pub(crate) fn database(&self) -> Result<Database, ConfigError> {
        Database::connect_lazy(
            &self.database_url,
            self.database_max_connections,
            self.database_acquire_timeout,
        )
        .map_err(|_| ConfigError::InvalidDatabase)
    }

    pub(crate) const fn claim_lease(&self) -> Duration {
        self.claim_lease
    }

    pub(crate) const fn poll_interval(&self) -> Duration {
        self.poll_interval
    }

    pub(crate) fn runner_id(&self) -> &str {
        &self.runner_id
    }
}

fn read_database_url() -> Result<SecretString, ConfigError> {
    let Some(path) = env_string("DATABASE_URL_FILE")? else {
        return Err(ConfigError::InvalidDatabase);
    };
    if path.is_empty() || !Path::new(&path).is_absolute() {
        return Err(ConfigError::InvalidDatabase);
    }
    let metadata = fs::metadata(&path).map_err(|_| ConfigError::InvalidDatabase)?;
    if metadata.len() == 0 || metadata.len() > MAX_SECRET_FILE_BYTES || !metadata.is_file() {
        return Err(ConfigError::InvalidDatabase);
    }
    let mut value = fs::read_to_string(path).map_err(|_| ConfigError::InvalidDatabase)?;
    while value.ends_with('\n') || value.ends_with('\r') {
        value.pop();
    }
    if value.is_empty() || value.contains('\0') {
        return Err(ConfigError::InvalidDatabase);
    }
    Ok(SecretString::from(value))
}

fn env_string(key: &str) -> Result<Option<String>, ConfigError> {
    match env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(_)) => Err(ConfigError::NonUnicodeEnvironment),
    }
}

fn parse_bounded_u32(
    value: Option<String>,
    default: u32,
    minimum: u32,
    maximum: u32,
    error: ConfigError,
) -> Result<u32, ConfigError> {
    let parsed = value
        .map_or(Ok(default), |value| value.parse())
        .map_err(|_| error)?;
    if (minimum..=maximum).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(error)
    }
}

fn parse_bounded_u64(
    value: Option<String>,
    default: u64,
    minimum: u64,
    maximum: u64,
    error: ConfigError,
) -> Result<u64, ConfigError> {
    let parsed = value
        .map_or(Ok(default), |value| value.parse())
        .map_err(|_| error)?;
    if (minimum..=maximum).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(error)
    }
}

fn valid_runner_id(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= 200 && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::{ConfigError, parse_bounded_u64, valid_runner_id};

    #[test]
    fn runner_id_is_bounded_and_content_free() {
        assert!(valid_runner_id("agent-019f4ad1"));
        assert!(!valid_runner_id(""));
        assert!(!valid_runner_id("agent\nunsafe"));
    }

    #[test]
    fn runner_timings_are_bounded() {
        assert_eq!(
            parse_bounded_u64(Some("30".to_owned()), 5, 5, 60, ConfigError::InvalidRunner)
                .expect("value should be in range"),
            30
        );
        assert!(
            parse_bounded_u64(Some("0".to_owned()), 5, 5, 60, ConfigError::InvalidRunner).is_err()
        );
    }
}
