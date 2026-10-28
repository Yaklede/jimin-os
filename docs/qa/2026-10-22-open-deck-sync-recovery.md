# Open Deck bridge refresh recovery

Scope: apps/desktop/src/open-deck-bridge.ts, open-deck-sync.ts and their tests; src-tauri/src/open_deck_bridge.rs and lib.rs. No server, database, session storage or UI changes.

Contract: Preserve last successful snapshot and its collection timestamp during server failures. Mark it retrying, disable write capabilities, retry reads at 2/5/10/30 seconds, serialize overlapping refreshes, and stop publishing on disposal. Online/focus/visibility events request recovery without moving focus. Authentication continues through the existing refresh coordinator. Never retry writes here.

Diagnostics: allowlisted API route, status, error category, elapsed duration, attempt/retry timestamps and recovery only. No bearer credentials, URLs with queries, bodies, titles, notes or raw error messages. Native owner-only JSONL rotates at 256 KiB with one previous file.

Backend checklist: existing bearer/loopback guards unchanged; sync fields strictly validated; no migrations; existing formatter, TypeScript, Vitest, Rust test, Clippy and app build gates used. Focus and motion unchanged; no UI controls or new dependencies.

Validation (2026-10-22, macOS + SM-T870 physical tablet):

- Frontend: 83 suites / 508 tests passed; TypeScript and changed-file Prettier checks passed.
- Native: 5 bridge/mutation tests passed; desktop Clippy and Rust formatting passed; production macOS app built and installed. Existing keychain access was approved by the owner after update. The live bridge publishes the new `sync` metadata with `state: fresh` and continues successful collections.
- Open Deck: 66 Node tests; 18 gesture + 15 freshness checks passed; signed Android release and signed macOS app built/installed as 0.8.5.
- Physical bridge outage: descriptor hidden briefly and restored in a finally block. Schedule/task/inbox row counts stayed 11/9/5 and collection time stayed unchanged while stale. Automatic recovery completed within 4 seconds after restoration. Amber stale text, original collection time and elapsed age were visually confirmed on the physical tablet.
- Upstream-failure presentation: a temporary authenticated loopback fixture returned a retrying API-timeout snapshot. No production API, network/VPN, calendar or task was modified. Live Open Deck reported `backend_retrying` with `bridgeConnected: true`, retained all rows, recorded allowlisted `/v1/home` timeout diagnostics and automatically restored fresh data after the original descriptor was restored. Final tablet screenshot confirmed normal status.
- Actual host diagnostics were verified owner-only (0600), including failure/recovery and API timeout entries. No titles or credentials are logged.
- Full repository backend harness still reports the same 5 existing findings as an isolated HEAD archive (3 API TypeScript files, design-preview.ts, copy.ts). None is in the changed files; no new finding remains. This is a baseline limitation, not a claim of a clean repository-wide scan.

Private evidence files are under the calling task's `work/open-deck/` directory: recovery-live-bridge.json, recovery-live-upstream-fixture.json and recovery-tablet-{stale,api-stale,final}.png. No personal snapshots are committed. Existing Workspace vacation action and device pairing remain configured.
