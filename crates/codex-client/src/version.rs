use std::path::Path;

use serde::Serialize;
use tokio::process::Command;

use crate::error::{Error, Result};

pub const SUPPORTED_CODEX_VERSION: &str = "0.159.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilitySummary {
    pub compatible: bool,
    pub expected_version: &'static str,
    pub actual_version: String,
}

/// Executes `codex --version` and compares it with the adapter's pinned version.
///
/// # Errors
///
/// Returns a typed process or parse error. Malformed command output is never
/// included in the error value.
pub async fn probe_compatibility(codex_binary: &Path) -> Result<CompatibilitySummary> {
    let output = Command::new(codex_binary)
        .arg("--version")
        .kill_on_drop(true)
        .output()
        .await
        .map_err(Error::VersionCheck)?;

    if !output.status.success() {
        return Err(Error::VersionCommandFailed);
    }

    let actual_version = parse_version_output(&output.stdout)?;
    Ok(CompatibilitySummary {
        compatible: actual_version == SUPPORTED_CODEX_VERSION,
        expected_version: SUPPORTED_CODEX_VERSION,
        actual_version,
    })
}

pub(crate) async fn ensure_compatible(codex_binary: &Path) -> Result<()> {
    let summary = probe_compatibility(codex_binary).await?;
    if summary.compatible {
        Ok(())
    } else {
        Err(Error::IncompatibleVersion {
            expected: SUPPORTED_CODEX_VERSION,
            actual: summary.actual_version,
        })
    }
}

fn parse_version_output(output: &[u8]) -> Result<String> {
    const PREFIX: &str = "codex-cli ";

    let text = std::str::from_utf8(output).map_err(|_| Error::MalformedVersionOutput)?;
    let version = text
        .trim_end_matches(['\r', '\n'])
        .strip_prefix(PREFIX)
        .ok_or(Error::MalformedVersionOutput)?;

    let parts = version.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(Error::MalformedVersionOutput);
    }

    Ok(version.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{SUPPORTED_CODEX_VERSION, parse_version_output};
    use crate::error::Error;

    #[test]
    fn parses_the_pinned_codex_version() {
        assert_eq!(
            parse_version_output(b"codex-cli 0.159.0\n").expect("valid version"),
            SUPPORTED_CODEX_VERSION
        );
    }

    #[test]
    fn deployment_and_schema_use_the_adapter_runtime_version() {
        let versions = include_str!("../../../deploy/versions.env");
        let dockerfile = include_str!("../../../deploy/docker/agent.Dockerfile");
        assert!(
            versions.lines().any(|line| {
                line.strip_prefix("CODEX_VERSION=") == Some(SUPPORTED_CODEX_VERSION)
            })
        );
        let docker_versions = dockerfile
            .lines()
            .filter_map(|line| line.strip_prefix("ARG CODEX_VERSION="))
            .collect::<Vec<_>>();
        assert_eq!(docker_versions, vec![SUPPORTED_CODEX_VERSION; 2]);
        let metadata: serde_json::Value =
            serde_json::from_str(include_str!("../../../schemas/codex/0.159.0/metadata.json"))
                .expect("runtime schema metadata");
        assert_eq!(metadata["codexVersion"], SUPPORTED_CODEX_VERSION);
        let integrity = versions
            .lines()
            .find_map(|line| line.strip_prefix("CODEX_NPM_INTEGRITY="))
            .expect("pinned npm integrity");
        assert_eq!(metadata["packageIntegrity"], integrity);
        assert!(
            dockerfile
                .lines()
                .any(|line| { line.strip_prefix("ARG CODEX_NPM_INTEGRITY=") == Some(integrity) })
        );
    }

    #[test]
    fn rejects_malformed_output_without_echoing_it() {
        let error = parse_version_output(b"secret unexpected output\n").expect_err("invalid");
        assert!(matches!(error, Error::MalformedVersionOutput));
        assert!(!error.to_string().contains("secret"));
    }
}
