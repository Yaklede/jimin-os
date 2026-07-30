use std::{
    collections::BTreeSet,
    env::{self, VarError},
    fs,
    path::Path,
    time::Duration,
};

use jimin_storage::Database;
use secrecy::SecretString;
use thiserror::Error;
use uuid::Uuid;

use crate::itsm::ItsmClient;

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
    meeting_transcriber_url: Option<String>,
    itsm_client: Option<ItsmClient>,
}

#[derive(Debug, Clone, Copy, Error)]
pub(crate) enum ConfigError {
    #[error("agent database configuration is invalid")]
    InvalidDatabase,
    #[error("agent runner configuration is invalid")]
    InvalidRunner,
    #[error("agent ITSM configuration is invalid")]
    InvalidItsm,
    #[error("agent environment contains non-Unicode data")]
    NonUnicodeEnvironment,
}

impl ConfigError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::InvalidDatabase => "agent_database_configuration_invalid",
            Self::InvalidRunner => "agent_runner_configuration_invalid",
            Self::InvalidItsm => "agent_itsm_configuration_invalid",
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
        let meeting_transcriber_url =
            env_string("JIMIN_MEETING_TRANSCRIBER_URL")?.filter(|value| !value.trim().is_empty());
        if meeting_transcriber_url.as_deref().is_some_and(|value| {
            !(value.starts_with("http://") || value.starts_with("https://"))
                || value.chars().count() > 2_000
                || value.chars().any(char::is_control)
        }) {
            return Err(ConfigError::InvalidRunner);
        }
        let itsm_base_url =
            env_string("JIMIN_ITSM_BASE_URL")?.filter(|value| !value.trim().is_empty());
        let itsm_token_file =
            env_string("JIMIN_ITSM_API_TOKEN_FILE")?.filter(|value| !value.trim().is_empty());
        let itsm_allowed_source_ids = env_string("JIMIN_ITSM_ALLOWED_SOURCE_IDS")?;
        let itsm_client = match (itsm_base_url, itsm_token_file, itsm_allowed_source_ids) {
            (None, None, None) => None,
            (Some(base_url), token_file, Some(allowed_source_ids)) => {
                let token = token_file
                    .map(|path| read_secret_file(&path, ConfigError::InvalidItsm))
                    .transpose()?;
                let allowed_source_ids = parse_itsm_allowed_source_ids(&allowed_source_ids)?;
                Some(
                    ItsmClient::new(&base_url, token, allowed_source_ids)
                        .map_err(|_| ConfigError::InvalidItsm)?,
                )
            }
            _ => return Err(ConfigError::InvalidItsm),
        };

        Ok(Self {
            database_url,
            database_max_connections,
            database_acquire_timeout,
            claim_lease,
            poll_interval,
            runner_id,
            meeting_transcriber_url,
            itsm_client,
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

    pub(crate) fn meeting_transcriber_url(&self) -> Option<&str> {
        self.meeting_transcriber_url.as_deref()
    }

    pub(crate) const fn itsm_client(&self) -> Option<&ItsmClient> {
        self.itsm_client.as_ref()
    }
}

fn read_database_url() -> Result<SecretString, ConfigError> {
    let Some(path) = env_string("DATABASE_URL_FILE")? else {
        return Err(ConfigError::InvalidDatabase);
    };
    read_secret_file(&path, ConfigError::InvalidDatabase)
}

fn read_secret_file(path: &str, error: ConfigError) -> Result<SecretString, ConfigError> {
    if path.is_empty() || !Path::new(&path).is_absolute() {
        return Err(error);
    }
    let metadata = fs::metadata(path).map_err(|_| error)?;
    if metadata.len() == 0 || metadata.len() > MAX_SECRET_FILE_BYTES || !metadata.is_file() {
        return Err(error);
    }
    let mut value = fs::read_to_string(path).map_err(|_| error)?;
    while value.ends_with('\n') || value.ends_with('\r') {
        value.pop();
    }
    if value.is_empty() || value.contains('\0') {
        return Err(error);
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

fn parse_itsm_allowed_source_ids(value: &str) -> Result<BTreeSet<Uuid>, ConfigError> {
    let mut source_ids = BTreeSet::new();
    for candidate in value.split(',') {
        let candidate = candidate.trim();
        if candidate.is_empty() {
            return Err(ConfigError::InvalidItsm);
        }
        let source_id = Uuid::parse_str(candidate).map_err(|_| ConfigError::InvalidItsm)?;
        if source_id.get_version_num() != 7 {
            return Err(ConfigError::InvalidItsm);
        }
        source_ids.insert(source_id);
    }
    if source_ids.is_empty() {
        return Err(ConfigError::InvalidItsm);
    }
    Ok(source_ids)
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
    use super::{ConfigError, parse_bounded_u64, parse_itsm_allowed_source_ids, valid_runner_id};
    use uuid::Uuid;

    #[test]
    fn runner_id_is_bounded_and_content_free() {
        assert!(valid_runner_id("agent-019f4ad1"));
        assert!(!valid_runner_id(""));
        assert!(!valid_runner_id("agent\nunsafe"));
    }

    #[test]
    fn itsm_allowed_sources_require_nonempty_v7_uuids() {
        let first = Uuid::now_v7();
        let second = Uuid::now_v7();
        let parsed = parse_itsm_allowed_source_ids(&format!("{first}, {second}"))
            .expect("v7 source allowlist should parse");
        assert_eq!(parsed, [first, second].into_iter().collect());

        assert!(parse_itsm_allowed_source_ids("").is_err());
        assert!(parse_itsm_allowed_source_ids(&format!("{first},")).is_err());
        assert!(parse_itsm_allowed_source_ids("not-a-uuid").is_err());
        assert!(
            parse_itsm_allowed_source_ids("550e8400-e29b-41d4-a716-446655440000").is_err(),
            "non-v7 UUIDs must not identify a Google Chat source"
        );
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
