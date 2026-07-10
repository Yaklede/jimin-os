# Codex App Server schema snapshots

이 디렉터리는 실제로 검증한 Codex CLI 버전별 App Server 계약을 보관한다. 생성물은 adapter 내부 검증과 호환성 회귀 테스트에만 사용하며 Jimin OS의 공개 도메인 타입으로 노출하지 않는다.

버전 디렉터리의 `metadata.json`에는 생성에 사용한 Codex 버전, 실행 파일 checksum, 생성 시각과 stable API 사용 여부가 기록된다. `--experimental`을 사용한 생성물은 이 디렉터리에 포함하지 않는다.

TypeScript binding은 build artifact라는 의미를 명확히 하고 품질 검사에서 직접 작성한 source와 분리하기 위해 버전별 `dist/typescript`에 둔다. 파일은 원본 generator output 그대로 커밋하며 `.gitattributes`에서 generated code로 표시한다.

재생성 전에는 다음을 확인한다.

1. `codex --version`이 대상 디렉터리 이름과 같은가.
2. `codex` 실행 파일의 SHA-256이 metadata와 같은가.
3. 두 생성 명령에 `--experimental`이 없는가.
4. 생성 후 adapter fixture와 compatibility test가 모두 통과하는가.

공식 계약은 [Codex App Server 문서](https://developers.openai.com/codex/app-server/)를 기준으로 한다.
