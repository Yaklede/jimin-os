# 홈 업무 요청 탭 수정

- 원인: 전체 홈은 2열 그리드인데 새 wrapper에 grid-column을 지정하지 않아 카드가 반 너비로 줄었다. 신규/기존을 두 카드로 쌓아 목록·상세도 반복됐다.
- 수정: wrapper가 홈 전체 너비를 차지하고 하나의 영역에서 신규/기존 탭을 전환한다. 기존 목록·상세·확인/제외 기능은 유지한다.
- 숨긴 탭은 접근성 트리와 화면에서 제외하되 component를 유지해 선택/작성 상태가 사라지지 않게 한다.
- 키보드: 좌/우, Home/End로 활성 탭 및 focus 이동. 터치는 각 탭 버튼 44px 이상.
- 모바일: 짧은 탭 이름과 동일한 건수, 기존 펼침 방식 유지. 390×844 요청 뷰포트에서 실제 clientWidth/scrollWidth 375/375, visible panel 1개를 확인했다.
- 실제 브라우저에서 신규↔기존 전환, 키보드 왼쪽 이동, 제외 사유 작성 후 왕복 시 입력 보존을 확인했다. 샘플만 사용했으며 운영 데이터는 변경하지 않았다.
- 화면 테스트 313개, TypeScript 및 web build 통과. 백엔드 변경은 없다.
- 개발 검증 동안 운영 설치본은 교체하지 않았다. 사용자 승인 후 아래 운영 반영을 진행했다.

## 운영 반영

- 사용자 승인 후 `b4b88a5` 화면을 운영 서버 주소 `https://os.jimin.ai.kr`로 macOS release 빌드하고 production assets 검사를 통과했다.
- `/Applications/Jimin OS.app`을 업데이트하고 재실행했다. 기존 앱 ID/서명 요구사항 유지 및 codesign deep/strict 검증 통과.
- 설치 실행 파일 SHA-256: `7cd05bf708e544d9bbe31f0ba32729c326d01b4f6d06928e0fde764abf14a599`.
- 이전 설치본: `/tmp/jimin-os-release-inflow-tabs-20261006/previous-installed.app`.
- 실제 데이터의 새 요청 19건/기존 업데이트 2건 탭 전환과 단일 전체 너비 목록·상세를 확인했다. 업무 상태 변경·원문 Chat 발송은 하지 않았다.
- 서버/API 변경 없음. 기존 서버 SHA `7414ed94aba418fee43ca25c572d20b4288e7e16` 및 schema 57 유지. Mac에서 live/ready 응답 정상 확인.
- 개발 미리보기 탭/임시 소스 및 Vite 종료. 작업용 dist/node_modules는 운영 설치 확인 후 정리했다.
- Android 기기는 연결되지 않아 이번 실기기 설치는 실행하지 않았다. 모바일 반응형 검증은 개발 단계에서 통과했다.
