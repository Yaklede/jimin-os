-- A completed or failed OAuth transaction no longer needs the PKCE verifier.
-- Make its encrypted columns nullable so the service can cryptographically
-- delete the one-time verifier immediately after the exchange attempt.
ALTER TABLE calendar_oauth_authorizations
    ALTER COLUMN pkce_verifier_ciphertext DROP NOT NULL,
    ALTER COLUMN pkce_nonce DROP NOT NULL,
    ALTER COLUMN encryption_key_version DROP NOT NULL;

UPDATE calendar_oauth_authorizations
SET pkce_verifier_ciphertext = NULL,
    pkce_nonce = NULL,
    encryption_key_version = NULL
WHERE status NOT IN ('pending', 'exchanging');

ALTER TABLE calendar_oauth_authorizations
    ADD CONSTRAINT calendar_oauth_authorizations_pkce_lifetime_check
    CHECK (
        (
            status IN ('pending', 'exchanging')
            AND pkce_verifier_ciphertext IS NOT NULL
            AND pkce_nonce IS NOT NULL
            AND encryption_key_version IS NOT NULL
        )
        OR (
            status NOT IN ('pending', 'exchanging')
            AND pkce_verifier_ciphertext IS NULL
            AND pkce_nonce IS NULL
            AND encryption_key_version IS NULL
        )
    );

UPDATE jimin_schema_metadata
SET schema_version = 9
WHERE singleton = TRUE;
