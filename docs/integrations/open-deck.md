# Open Deck companion integration

Jimin OS owns the authenticated server session. The macOS client publishes a minimal read model to a local companion bridge. Open Deck never reads the OS Keychain item or receives its server access/refresh tokens.

## Protocol 1

The app writes `open-deck-bridge.json` in its application-data directory with mode 0600. The descriptor contains protocol, ephemeral loopback port, per-launch random bearer credential and PID. Both routes require the bearer credential, exact loopback Host and absence of browser Origin; no CORS headers are exposed. The request body limit is 8 KiB.

- `GET /v1/snapshot`: schema version, update time, connection state, Korean time zone, two-month range, counts, schedule summaries, open task summaries, unread request summaries, scheduled-work summaries. Original messages, notes, attachments, reference URLs and server tokens are omitted. Snapshots are bounded to 128 KiB and explicit nested field allowlists.
- `POST /v1/action`: `navigate` to an allowed OS destination, `refresh`, `open-schedule` or `open-task` with a UUID. No shell commands, arbitrary URLs or write operations. At most four actions await acknowledgement, with an eight-second native timeout. The frontend cancels navigation after 6.5 seconds, keeps unresolved workers capped at four and prevents late UI movement after a timeout or disconnect. Navigation surfaces the existing OS window only after acknowledgement; snapshot reads and refresh do not take focus.

The frontend uses the existing authenticated-session refresh coordinator and read-only API requests. It publishes every 15 seconds. The Open Deck provider polls locally every three seconds, rejects snapshots older than two minutes, and retains no stale work data in its UI after disconnection. Closing the app invalidates the listener; the next launch replaces the descriptor and credential.

## User flow

Open Deck provides an optional Jimin OS extension and OS tab. Calendar dates show schedules and deadlines, request summaries link to the OS home, and quick actions open scheduled work, new schedule, tasks, meetings and the assistant. Entity opening navigates to the existing calendar and highlights the selected item; create dialogs remain the source of truth. Visible modal editing blocks remote navigation rather than discarding work. Task completion, Chat delivery and ITSM changes still use the OS's existing confirmation and recovery flows.

The Open Deck host emits new-request and upcoming schedule/deadline notifications to the user's paired tablet. It baselines the initial inbox and persists deduplication keys. Android uses a private-lockscreen notification channel and its own runtime notification permission. Delivery requires the local apps and tablet connection; it is not a cloud push service for force-stopped clients.

## Verification

Frontend unit tests verify the minimal data boundary, Korean month range and read-only/session-expiry behavior. Rust tests cover the action allowlist and rejection of unknown snapshot fields, including nested credentials. `cargo clippy -p jimin-desktop --all-targets -- -D warnings`, scoped frontend tests/typecheck and production client build apply. The repository backend harness also ran: five pre-existing heuristic findings in deliveryRecovery.ts, inflowChanges.ts, scheduledWork.ts, design-preview.ts and copy.ts are outside this change (typed token parameters and existing product text); no new finding belongs to the bridge. No server deployment or database migration is required.

The final scoped frontend suite has 25 passing tests, including cancellation, late-response suppression, disposal and worker limits. The scoped design and UX writing harnesses pass. The backend harness passes on the changed-file scope; the full baseline scan reproduces the same five pre-existing findings.

Month-end all-day provider events use a one-day padded server lookup while the displayed calendar month remains exact. Companion snapshots discard lookup-only records outside their published range.

## Native tablet writes

The companion advertises schedule-create/task-complete capabilities and task versions. `POST /v1/mutations` validates either schedule creation or completion, persists a UUIDv7 receipt before dispatch, and never surfaces the Mac window. `GET /v1/mutations/{id}` returns running/succeeded/failed/uncertain status without copying input notes. The descriptor bearer and Origin guards apply to these routes too.

The existing authenticated frontend creates schedules using the receipt UUID as `clientMutationId`; task completion reads current status/version before applying the existing completion API. Retries reconcile already-created schedules and already-completed tasks without resending notes. A reused UUID with changed contents is rejected. Known version conflicts remain failed; ambiguous writes remain uncertain. The private 0600 journal is bounded to 200 entries/24 hours, and startup does not replay work. A stalled receipt becomes uncertain after two minutes. Android explicitly confirms completion and explains that optional notes can reach the original Google Chat conversation through the normal server behavior.
