//! Domain rules shared by authentication, device, and synchronization use cases.
//!
//! This crate intentionally does not depend on HTTP, `SQLx`, or OAuth provider
//! implementations. It owns the validation rules that must stay identical for
//! every client platform.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const MAX_GOOGLE_SUBJECT_BYTES: usize = 255;
const MAX_EMAIL_BYTES: usize = 320;
const MAX_DEVICE_NAME_CHARS: usize = 80;
const MAX_VERSION_CHARS: usize = 80;
const PKCE_VERIFIER_MINIMUM_CHARS: usize = 43;
const PKCE_VERIFIER_MAXIMUM_CHARS: usize = 128;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ValidationError {
    #[error("Google identity subject is invalid")]
    InvalidGoogleSubject,
    #[error("email address is invalid")]
    InvalidEmail,
    #[error("PKCE verifier is invalid")]
    InvalidPkceVerifier,
    #[error("device registration is invalid")]
    InvalidDeviceRegistration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleSubject(String);

impl GoogleSubject {
    /// Creates a stable Google account identifier without retaining provider
    /// tokens or any credential material.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidGoogleSubject`] for an empty,
    /// overlong, or control-character value.
    pub fn parse(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_GOOGLE_SUBJECT_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(ValidationError::InvalidGoogleSubject);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailAddress {
    display: String,
    normalized: String,
}

impl EmailAddress {
    /// Parses an email address for Google identity display and allowlist
    /// comparison. This does not apply provider-specific aliasing rules such
    /// as dot or plus-address removal.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidEmail`] when the value cannot safely
    /// represent a verified account email.
    pub fn parse(value: impl Into<String>) -> Result<Self, ValidationError> {
        let display = value.into().trim().to_owned();
        if display.is_empty()
            || display.len() > MAX_EMAIL_BYTES
            || display.chars().any(char::is_control)
            || display.matches('@').count() != 1
        {
            return Err(ValidationError::InvalidEmail);
        }

        let (local, domain) = display
            .split_once('@')
            .ok_or(ValidationError::InvalidEmail)?;
        if local.is_empty() || domain.is_empty() || domain.starts_with('.') || domain.ends_with('.')
        {
            return Err(ValidationError::InvalidEmail);
        }

        Ok(Self {
            normalized: display.to_lowercase(),
            display,
        })
    }

    #[must_use]
    pub fn display(&self) -> &str {
        &self.display
    }

    #[must_use]
    pub fn normalized(&self) -> &str {
        &self.normalized
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailAllowlist {
    normalized_addresses: BTreeSet<String>,
}

impl EmailAllowlist {
    /// Parses a server-owned email allowlist into its comparison form.
    ///
    /// # Errors
    ///
    /// Returns a validation error when an entry is malformed or duplicate
    /// after normalization.
    pub fn from_entries(
        entries: impl IntoIterator<Item = String>,
    ) -> Result<Self, ValidationError> {
        let mut normalized_addresses = BTreeSet::new();
        for entry in entries {
            let email = EmailAddress::parse(entry)?;
            if !normalized_addresses.insert(email.normalized) {
                return Err(ValidationError::InvalidEmail);
            }
        }
        Ok(Self {
            normalized_addresses,
        })
    }

    #[must_use]
    pub fn permits(&self, email: &EmailAddress) -> bool {
        self.normalized_addresses.contains(email.normalized())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkceVerifier(String);

impl PkceVerifier {
    /// Validates the RFC 7636 verifier length and unreserved character set.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidPkceVerifier`] when the verifier does
    /// not meet the M1 platform profile requirements.
    pub fn parse(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        let valid_length =
            (PKCE_VERIFIER_MINIMUM_CHARS..=PKCE_VERIFIER_MAXIMUM_CHARS).contains(&value.len());
        let valid_characters = value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~'));
        if !valid_length || !valid_characters {
            return Err(ValidationError::InvalidPkceVerifier);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn expose_for_provider_exchange(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ClientPlatform {
    Macos,
    Ios,
    Android,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRegistration {
    installation_id: Uuid,
    platform: ClientPlatform,
    name: String,
    app_version: String,
    os_version: Option<String>,
}

impl DeviceRegistration {
    /// Validates device metadata received from a platform client.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidDeviceRegistration`] when the
    /// installation ID is not `UUIDv7` or any metadata is blank, overlong, or
    /// contains control characters.
    pub fn new(
        installation_id: Uuid,
        platform: ClientPlatform,
        name: impl Into<String>,
        app_version: impl Into<String>,
        os_version: Option<String>,
    ) -> Result<Self, ValidationError> {
        let name_input = name.into();
        let app_version_input = app_version.into();
        let name = sanitize_required(&name_input, MAX_DEVICE_NAME_CHARS)?;
        let app_version = sanitize_required(&app_version_input, MAX_VERSION_CHARS)?;
        let os_version = os_version
            .map(|value| sanitize_required(&value, MAX_VERSION_CHARS))
            .transpose()?;

        if installation_id.get_version_num() != 7 {
            return Err(ValidationError::InvalidDeviceRegistration);
        }

        Ok(Self {
            installation_id,
            platform,
            name,
            app_version,
            os_version,
        })
    }

    #[must_use]
    pub const fn installation_id(&self) -> Uuid {
        self.installation_id
    }

    #[must_use]
    pub const fn platform(&self) -> ClientPlatform {
        self.platform
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn app_version(&self) -> &str {
        &self.app_version
    }

    #[must_use]
    pub fn os_version(&self) -> Option<&str> {
        self.os_version.as_deref()
    }
}

fn sanitize_required(value: &str, maximum_chars: usize) -> Result<String, ValidationError> {
    let value = value.trim().to_owned();
    if value.is_empty()
        || value.chars().count() > maximum_chars
        || value.chars().any(char::is_control)
    {
        return Err(ValidationError::InvalidDeviceRegistration);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_normalization_preserves_provider_alias_semantics() {
        let email = EmailAddress::parse("  Jimin.Test+os@Example.COM ").expect("valid email");

        assert_eq!(email.display(), "Jimin.Test+os@Example.COM");
        assert_eq!(email.normalized(), "jimin.test+os@example.com");
    }

    #[test]
    fn allowlist_compares_normalized_email_only() {
        let allowlist = EmailAllowlist::from_entries(["owner@example.com".to_owned()])
            .expect("allowlist should parse");
        let email = EmailAddress::parse("OWNER@example.com").expect("email should parse");

        assert!(allowlist.permits(&email));
    }

    #[test]
    fn malformed_email_and_duplicate_allowlist_entries_are_rejected() {
        assert!(EmailAddress::parse("no-at-sign").is_err());
        assert!(
            EmailAllowlist::from_entries([
                "owner@example.com".to_owned(),
                "OWNER@example.com".to_owned(),
            ])
            .is_err()
        );
    }

    #[test]
    fn pkce_verifier_requires_rfc7636_length_and_characters() {
        let valid = "A".repeat(PKCE_VERIFIER_MINIMUM_CHARS);
        assert!(PkceVerifier::parse(valid).is_ok());
        assert!(PkceVerifier::parse("A".repeat(PKCE_VERIFIER_MINIMUM_CHARS - 1)).is_err());
        assert!(
            PkceVerifier::parse(format!("{}!", "A".repeat(PKCE_VERIFIER_MINIMUM_CHARS - 1)))
                .is_err()
        );
    }

    #[test]
    fn device_registration_requires_uuidv7_and_safe_metadata() {
        let registration = DeviceRegistration::new(
            Uuid::now_v7(),
            ClientPlatform::Ios,
            " Jimin's iPhone ",
            "0.1.0",
            Some("18.0".to_owned()),
        )
        .expect("device should be valid");

        assert_eq!(registration.name(), "Jimin's iPhone");
        assert_eq!(registration.os_version(), Some("18.0"));
        assert!(
            DeviceRegistration::new(
                Uuid::nil(),
                ClientPlatform::Ios,
                "Jimin's iPhone",
                "0.1.0",
                None,
            )
            .is_err()
        );
    }
}
