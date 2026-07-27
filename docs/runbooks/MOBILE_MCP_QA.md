# Mobile MCP 기반 모바일 QA

Jimin OS는 Mobile MCP를 개발·검수 도구로만 사용한다. 서버나 운영 모바일 앱에
MCP server 또는 `mobilecli`를 포함하지 않는다. OpenMinis에서 참고한 native
capability 경계는 Jimin OS의 Tauri plugin adapter 안에 구현하고, 데이터 원본과
AI 판단은 개인 Rust 서버에 유지한다.

## 고정 버전과 안전 기준

- `@mobilenext/mobile-mcp`: root `devDependencies`에 고정한다.
- `mobilecli`: Mobile MCP와 호환되는 버전을 root `devDependencies`에 고정한다.
- `MOBILEMCP_DISABLE_TELEMETRY=1`을 항상 적용한다.
- 기본 smoke test는 emulator/simulator만 허용한다.
- 운영 패키지 `io.jimin.os`는 자동 smoke 대상에서 제외한다.
- screenshot과 실행 결과는 `.mobile-mcp/artifacts/`에 보관하고 Git에 올리지 않는다.

## 준비

```bash
corepack pnpm install --frozen-lockfile
pnpm mobile:qa:doctor
```

Android emulator가 없다면 별도 terminal에서 테스트 AVD를 실행한다.

```bash
"$HOME/Library/Android/sdk/emulator/emulator" \
  -avd JiminOS_Test_API_36 \
  -no-snapshot-save \
  -no-boot-anim
```

개발 APK는 운영 앱을 덮어쓰지 않는 `io.jimin.os.dev`로 설치한다.

```bash
./scripts/install-local-phone-test.sh \
  deploy/env/local.env.example emulator-5554
```

## 기본 smoke

```bash
pnpm mobile:qa:smoke
```

여러 emulator가 실행 중이면 명시적으로 선택한다.

```bash
pnpm mobile:qa:smoke -- --device=emulator-5554
```

기본 smoke는 다음을 확인한다.

1. 개발 앱 설치 여부
2. cold start
3. 접근성 tree 조회
4. 화면 크기 조회
5. 네이티브 뒤로 가기
6. 앱 재실행
7. 단계별 screenshot 저장

## 회의 기능 수동 확장 시나리오

Mobile MCP로 화면을 탐색한 뒤 아래 흐름을 확인한다.

1. `회의` 탭을 연다.
2. `회의 기록하기`를 선택한다.
3. 회의 이름, 목적, 참석자와 원문을 입력한다.
4. 음성 입력을 시작하고 microphone permission 상태를 확인한다.
5. 분석을 요청한다.
6. 생성된 후속 할 일의 담당자, 우선순위와 기한을 수정한다.
7. 저장 후 프로젝트 할 일로 반영한다.
8. 뒤로 가기와 앱 재실행 후 같은 회의가 유지되는지 확인한다.

Agent가 탐색하면서 발견한 안정된 flow는 별도 deterministic test로 옮긴다.
