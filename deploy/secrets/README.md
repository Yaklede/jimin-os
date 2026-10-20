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
| `itsm_read_credential` | ITSM 조회 및 일감 마감일·완료 상태 수정 권한을 가진 API token 한 줄. 기존 파일명 유지 | agent, ITSM 연결을 켠 경우만 |
| `google-services.json` | `io.jimin.os` Android 앱의 Firebase 구성 파일 | Android client build |
| `gateway_tls_cert` | PEM certificate chain; `JIMIN_TLS_MODE=files`에서만 필요 | gateway |
| `gateway_tls_key` | PEM private key; `JIMIN_TLS_MODE=files`에서만 필요 | gateway |

권장 권한은 디렉터리 `0700`, 파일 `0600`이다. 운영 서버에서 ITSM token을
배포 사용자가 아닌 root만 관리한다면 `itsm_read_credential`은 `root:root`,
`0400`으로 만들고 Compose 배포와 사전 검증도 root 권한으로 실행한다. Token은
환경 변수, command argument, shell history, 로그 또는 database에 복사하지 않는다.
`api_database_url` 예시 형식은 다음과 같으며 실제 값을 문서나 shell history에
남기지 않는다.

```text
postgres://jimin_api:<password>@postgres:5432/jimin_os
```

두 DB secret의 password가 다르면 API readiness가 실패한다. TLS가 `internal`이면 certificate와 key 파일을 만들지 않는다. 검증 script는 임시 디렉터리의 비밀이 아닌 fixture만 사용하며 이 경로에 실제 값을 생성하지 않는다.

Google Calendar는 `JIMIN_GOOGLE_CALENDAR_OAUTH_ENABLED=1`일 때만 위 두 파일을 mount한다. 이때 `JIMIN_GOOGLE_CALENDAR_REDIRECT_URI`와 Google Cloud Console의 OAuth web-client redirect URI는 정확히 같아야 한다. client credential이나 encryption key를 환경 파일·앱·로그에 넣지 않는다.

FCM은 `JIMIN_FIREBASE_MESSAGING_ENABLED=1`일 때만 `firebase_service_account`를
read-only secret으로 mount한다. Firebase Console에서 내려받은 JSON을 수정하거나
환경 변수에 펼치지 말고 파일 권한을 `0600`으로 유지한다.

ITSM 원문 보강은 환경별 config에서 `JIMIN_ITSM_ENABLED=1`로 설정할 때만
켜진다. 공통 배포 script가 `deploy/compose.itsm.yaml` overlay를 자동으로
선택하고 Agent용 `itsm_read_credential`을 사전 검증한다. API에는 화면의
연결 가능 상태를 표시하기 위한 non-secret enable flag만 전달한다. ITSM server
주소와 token file은 Agent에만 전달한다.
`JIMIN_ITSM_BASE_URL`에는 신뢰할 수 있는 HTTPS origin만 지정하고,
`itsm_read_credential`에는 이슈 조회·수정 권한을 가진 token을 넣는다. 기존 관리자
키도 재사용할 수 있으며 새 키 발급이나 파일명 변경은 필수가 아니다. 코드가 쓰는
필드는 `due_date`, `status_id`로 제한하고 제목·설명·담당자·댓글은 수정하지 않는다.
Agent는 Chat에
들어온 URL을 직접 호출하지 않고 설정된 origin의 숫자형 `/issues/{id}`만 API로
다시 조합해 조회한다. 어느 프로젝트가 ITSM 원문 보강을 사용하는지는 프로젝트
연결 설정으로 관리하며 source UUID나 token을 환경 파일에 복사하지 않는다.

schema 63부터 사용자가 일감을 등록·변경·완료할 때 검증된 티켓을 대기열에 넣는다.
마이그레이션 자체는 과거 일감이나 원격 티켓을 변경하지 않는다. 여러 티켓은 등록
화면에서 하나를 선택하며 함께 반영하기를 끌 수 있다. 기존 일감은 과거에 조회에
성공한 티켓이 하나로 확정되는 경우에만 다음 변경 때 연결한다. 같은 티켓의 연결된
일감이 모두 완료된 경우에만 완료 상태를 보낸다. 한 티켓의 마감일은 연결된 취소되지
않은 일감들의 가장 늦은 기한을 한국 날짜로 변환한다. 처음 기한 없이 등록하면
기존 원격 날짜를 보존하고, 이후 명시적으로 기한을 없애면 원격 날짜도 지운다.

완료 상태는 현재 서버의 상태 목록과 티켓의 허용 전환을 조회해 선택한다. 여러
완료 상태가 있어 하나로 확정할 수 없다면 `JIMIN_ITSM_COMPLETION_STATUS_ID`에
서버의 실제 종료 상태 번호를 설정한다. 상태 번호·사용자 정보·키를 AI가 추측하지
않는다. 이미 종료된 티켓은 상태를 다시 바꾸지 않는다. OS에서 재활성화할 때
레드마인을 자동으로 재개방하는 기능은 이 단방향 연동에 포함되지 않는다.
