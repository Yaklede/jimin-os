# UX Writing Run Manifest

Status: reviewed

## Target Files

- `apps/desktop/src/components/DesktopInflowNotificationSettings.tsx`
- `apps/desktop/src/copy.ts`
- `apps/desktop/src-tauri/src/inflow_notifications.rs`
- `apps/api/src/push.rs`
- `crates/storage/src/push.rs`
- `crates/storage/src/inflow_notifications.rs`

## Writing Contract

- WRITING.md reviewed: yes
- TERMS.md reviewed: yes
- Locale: Korean
- Product concept: actionable work alerts without repeatedly opening the inbox
- Tone: short, calm, 해요체

## Copy Review

- Korean: new requests and replies use separate labels; no invented urgency.
- English: diagnostic identifiers are internal, not user copy.
- Terms: 새 업무 요청 / 일감에 새 답글 / 알림 켜기 / 알림 끄기.
- Error messages: explain reconnect/retry; automatic retry described truthfully.
- Buttons and CTAs: explicitly turn alerts on or off.
- Empty/loading/success states: do not claim OS notification permission from server connectivity.
- Naming: no new public implementation terminology.

## Rewrites

| Before | After | Reason |
| --- | --- | --- |
| 새 Chat 업무 | 새 업무 요청 / 일감에 새 답글 | Distinguish new work from updates. |
| 새 일정과 할 일 알림 | 새 업무 요청·새 답글·일정과 할 일 알림 | Describe the newly supported events. |

## Exceptions

- No user-approved copy exceptions. Source-code literals such as null/undefined and internal auth status names are not UI wording; source-regex findings are recorded separately in the QA document.
