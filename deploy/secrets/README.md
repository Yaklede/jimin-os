# Runtime secrets

M1 adds four API authentication secret files alongside the database files:

- `auth_signing_key`: Ed25519 PKCS#8 private key used only for access-token signing.
- `auth_verify_key`: matching Ed25519 public key used for bearer-token verification.
- `auth_refresh_pepper`: at least 32 random bytes used as the server HMAC pepper.
- `auth_pairing_pepper`: at least 32 random bytes used only to derive
  short-lived QR device-pairing token verifiers.

Generate the Ed25519 pair outside the repository, store both files at mode `0600`, and never copy the private key or pepper into `.env` files. The API will remain unready if any file is missing or invalid.

실제 secret은 이 디렉터리 아래의 환경별 하위 디렉터리에만 만들고 Git에 추가하지 않는다.

```text
deploy/secrets/local/
deploy/secrets/staging/
```

각 환경에는 다음 파일이 필요하다.

| 파일 | 내용 | 사용 서비스 |
|---|---|---|
| `postgres_password` | PostgreSQL용 무작위 password 한 줄 | postgres |
| `api_database_url` | 같은 password를 사용한 전체 PostgreSQL URL 한 줄 | api |
| `google_calendar_client_secret` | Google OAuth web client secret 한 줄 | api, Calendar OAuth를 켠 경우만 |
| `calendar_encryption_key` | Calendar refresh/PKCE token을 암호화할 32바이트 이상 무작위 값 한 줄 | api, Calendar OAuth를 켠 경우만 |
| `firebase_service_account` | Firebase Admin SDK 서비스 계정 JSON 원본 | api, FCM을 켠 경우만 |
| `google-services.json` | `io.jimin.os` Android 앱의 Firebase 구성 파일 | Android client build |
| `gateway_tls_cert` | PEM certificate chain; `JIMIN_TLS_MODE=files`에서만 필요 | gateway |
| `gateway_tls_key` | PEM private key; `JIMIN_TLS_MODE=files`에서만 필요 | gateway |

권장 권한은 디렉터리 `0700`, 파일 `0600`이다. `api_database_url` 예시 형식은 다음과 같으며 실제 값을 문서나 shell history에 남기지 않는다.

```text
postgres://jimin_api:<password>@postgres:5432/jimin_os
```

두 DB secret의 password가 다르면 API readiness가 실패한다. TLS가 `internal`이면 certificate와 key 파일을 만들지 않는다. 검증 script는 임시 디렉터리의 비밀이 아닌 fixture만 사용하며 이 경로에 실제 값을 생성하지 않는다.

Google Calendar는 `JIMIN_GOOGLE_CALENDAR_OAUTH_ENABLED=1`일 때만 위 두 파일을 mount한다. 이때 `JIMIN_GOOGLE_CALENDAR_REDIRECT_URI`와 Google Cloud Console의 OAuth web-client redirect URI는 정확히 같아야 한다. client credential이나 encryption key를 환경 파일·앱·로그에 넣지 않는다.

FCM은 `JIMIN_FIREBASE_MESSAGING_ENABLED=1`일 때만 `firebase_service_account`를
read-only secret으로 mount한다. Firebase Console에서 내려받은 JSON을 수정하거나
환경 변수에 펼치지 말고 파일 권한을 `0600`으로 유지한다.
