# Runtime secrets

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
| `gateway_tls_cert` | PEM certificate chain; `JIMIN_TLS_MODE=files`에서만 필요 | gateway |
| `gateway_tls_key` | PEM private key; `JIMIN_TLS_MODE=files`에서만 필요 | gateway |

권장 권한은 디렉터리 `0700`, 파일 `0600`이다. `api_database_url` 예시 형식은 다음과 같으며 실제 값을 문서나 shell history에 남기지 않는다.

```text
postgres://jimin_api:<password>@postgres:5432/jimin_os
```

두 DB secret의 password가 다르면 API readiness가 실패한다. TLS가 `internal`이면 certificate와 key 파일을 만들지 않는다. 검증 script는 임시 디렉터리의 비밀이 아닌 fixture만 사용하며 이 경로에 실제 값을 생성하지 않는다.
