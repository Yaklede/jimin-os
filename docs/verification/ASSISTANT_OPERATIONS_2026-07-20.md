# Assistant operations verification — 2026-07-20

Status: contract, loopback live smoke, and private-server package gates complete;
physical Android VPN verification pending

Scope: private-server client packaging, general conversation, streaming, natural-language
task creation, batch action contracts, schedule conflict handling, project and webhook
transactions, goals, recommendations, and Google Calendar outbox persistence.

## Checklist

- [x] A private-server client accepts one HTTPS origin and rejects HTTP, paths,
      credentials, and loopback hosts.
- [x] A private-server build fails when `VITE_LOCAL_PHONE_TEST=1` is present.
- [x] Built JavaScript contains the expected server origin and no
      `http://127.0.0.1:8080` reference.
- [x] Android private-server installation removes a stale `adb reverse tcp:8080`.
- [x] The macOS private-server package is ad-hoc signed, installed, launched, and
      reaches production liveness and readiness probes.
- [x] The Android private-server APK is built and installed without a local
      `tcp:8080` reverse route.
- [x] General conversation completes without a work canvas.
- [x] Conversation processing exposes observable streaming snapshots.
- [x] A natural-language request creates a concise task with notes and tomorrow's
      Korea-local due date, then the smoke fixture is deleted.
- [x] Daily overview, completion history, future-date exclusion, and Korea-local
      date parsing contracts pass.
- [x] Batch task completion, restore, project deletion, bulk schedule cancellation,
      and departure-time schedule conversion contracts pass.
- [x] Schedule conflicts stop the mutation and create alternatives in the decision
      inbox unless the user explicitly accepts the conflict.
- [x] Google Calendar mutation outbox, retry, disconnect, and idempotency contracts
      pass against PostgreSQL.
- [x] Project webhook delivery, retry fencing, immutable snapshots, and Agent action
      audit contracts pass against PostgreSQL.
- [x] Goal progress, work brief generation, and recommendation decision history
      contracts pass against PostgreSQL.

## Evidence

| Gate | Result |
| --- | --- |
| Client build configuration | PASS |
| Frontend | 21 files / 95 tests PASS |
| API | 66 tests PASS |
| Agent | 44 tests PASS |
| PostgreSQL | 33 isolated scenarios PASS |
| Loopback live Agent | Observable SSE snapshots; general conversation and natural-language task creation PASS |
| macOS private client | Installed and running; code signature, production liveness, and readiness PASS |
| Android private client | APK installed on the emulator; production origin and local-address exclusion PASS |

The PostgreSQL runner gives every scenario a fresh database. Worker-wide queues such
as webhook delivery claims are intentionally global, so sharing one database between
otherwise owner-scoped tests can consume another scenario's pending fixture.

## Defects found and fixed

| ID | Finding | Fix | Re-verification |
| --- | --- | --- | --- |
| AO-001 | Recommendation approval test expected the obsolete `approved` intermediate state. Safe review actions now finish as `executed` in the same transaction. | Updated the status and two-version-transition contract. | Isolated recommendation lifecycle PASS |
| AO-002 | A shared PostgreSQL database let the global webhook worker claim another test's pending delivery. | Added a runner that creates one clean database per integration scenario. | 33/33 isolated scenarios PASS |

## Operational checks still separate

- Production Google provider create/update/delete must be checked without altering
  unrelated personal events.
- Google Chat and Discord receiver delivery require test destinations supplied for
  that purpose.
- Physical Android reminder delivery requires waiting for a real future alarm.
- Physical Android private-server access still needs a connected device with its
  Twingate/VPN route enabled. The emulator has no private-network route and correctly
  shows the server connection recovery screen.
- Apple distribution signing and notarization remain outside ad-hoc personal installs.
