# Database migrations

Migrations are forward-only and are embedded in `jimin-storage` at build time.

Before applying a migration to production:

1. apply it to an empty PostgreSQL database;
2. apply it to a restored staging backup;
3. verify `jimin_schema_metadata` and SQLx migration versions;
4. create a production backup;
5. keep the previous image digest available.

M1 identity tables use a forward-only `0002_m1_identity.sql` migration. The
session, refresh token, device, sync, and audit tables are intentionally
created before Google Calendar data. Calendar migrations must not alter the
semantics of existing session rows or refresh token verifier values.

Migration `0008_google_calendar_foundation.sql` adds the provider-owned
Calendar account, OAuth transaction, normalized event, sync, staging, and
mutation records. It does not add a Google credential to the repository or
make any outbound provider call by itself.

Migration `0020_schedule_calendar_outbox.sql` links Jimin OS schedules to the
writable primary Google Calendar and extends the durable mutation journal. Run
it first against an empty database and then a restored staging backup. Verify
that the link ownership joins are valid, the journal's single-source check is
valid, and `jimin_schema_metadata.schema_version = 20` before release. It is
forward-only: before any version-20 rows are accepted, rollback may use the
previous image after dropping the new trigger, table, indexes, constraint, and
column on a disposable copy. After writes begin, drain or archive pending
mutations and restore a verified pre-migration backup instead of downgrading in
place.

Migration `0021_work_intelligence.sql` adds the P1 decision loop without
changing existing planning rows. Goals, signals, recommendations, decisions,
verified action results, and brief runs are separate records so recommendation
approval cannot be confused with task completion. Apply it to an empty database
and a restored staging backup before release. A pre-version-21 image can be used
only before these tables receive writes; after that point rollback requires the
verified pre-migration backup rather than dropping decision history.

Migration `0022_work_brief_refresh.sql` makes one active signal map to at most
one recommendation. This prevents repeated home refreshes from recreating an
already handled suggestion. The index is additive; rollback before writes may
use the previous image, while rollback after recommendation writes uses the
verified pre-migration backup.

Migration `0023_typed_webhook_destinations.sql` limits newly managed webhook
connections to Google Chat and Discord while preserving existing generic rows
as read-only legacy data. New destination URLs are stored as encrypted secret
material and delivery rows retain an encrypted snapshot for retry safety. Apply
it to an empty database and a restored staging backup, then verify that existing
legacy deliveries can still drain and `jimin_schema_metadata.schema_version =
23`. Rollback after typed webhook writes requires the verified pre-migration
backup because ciphertext cannot be reconstructed by the previous image.

Migration `0024_retire_generic_webhooks.sql` permanently deletes the unused
generic webhook configurations and their delivery history. It then removes the
plaintext destination and authorization-header columns and constrains every
remaining webhook to Google Chat or Discord with an encrypted destination.
Apply it to an empty database and a restored staging backup, confirm that no
generic rows remain, and verify `jimin_schema_metadata.schema_version = 24`.
Rollback requires the verified pre-migration backup because deleted generic
webhook data cannot be reconstructed.

Migration `0025_agent_webhook_action_audit.sql` extends the existing Agent
action audit allowlists with `send_webhook_message`. It does not rewrite jobs,
messages, webhook configuration, or delivery history. Apply it to an empty
database and a restored version-24 backup, then execute one Agent-requested
webhook message and verify that the job, ordered action audit, and queued
delivery commit together with `jimin_schema_metadata.schema_version = 25`.
Before version-25 writes begin, rollback may use the previous image after
restoring the two version-24 check constraints on a disposable copy. After a
version-25 audit row is written, use the verified pre-migration backup rather
than downgrading in place.

Migration `0028_google_chat_mention_directory.sql` adds an editable Google Chat
name-to-user directory to typed webhook configurations and copies that directory
to every queued delivery. This keeps a retry's mention rendering immutable even
if the webhook settings change later. Apply it to an empty database and a
restored version-27 backup, then verify existing webhooks and deliveries receive
an empty `users` object and `jimin_schema_metadata.schema_version = 28`. The new
columns are additive, but after mention-aware deliveries are written rollback
must use the verified pre-migration backup so the original delivery rendering is
not lost.

Migration `0029_project_google_chat_inflow.sql` keeps the owner's personal
Calendar credential separate from multiple company Google Chat identities. It
adds project-owned Chat sources, a deduplicated inflow inbox, owner-scoped
promote/dismiss decisions, and encrypted refresh-token storage. Apply it to an
empty database and a restored version-28 backup, then verify that a repeated
provider message creates one inflow item and that
`jimin_schema_metadata.schema_version = 29`. Rollback after a company account,
source, or inflow item is written requires the verified pre-migration backup;
dropping the tables would also discard encrypted credentials and decision
history.

Migration `0031_google_chat_inflow_completion.sql` adds durable completion
delivery state to the selected Google Chat inflow item when a conversation is
promoted. The source message reaction and one idempotent thread reply can be
retried independently without rolling back the task or its webhook delivery.
Apply it to an empty database and a restored version-30 backup, then verify
that legacy promoted rows remain untouched, a new promotion queues exactly one
completion delivery, and `jimin_schema_metadata.schema_version = 31`.
Rollback after a completion delivery is requested requires the verified
pre-migration backup so provider delivery history is not discarded.

Migration `0032_task_hierarchy.sql` adds an optional parent relationship for
one-level task decomposition. Apply it to an empty database and a restored
version-31 backup, then verify existing tasks remain root tasks and
`jimin_schema_metadata.schema_version = 32`. The service only accepts a parent
owned by the same user and project, rejects deeper nesting, and prevents a child
deadline from extending beyond its parent deadline. Rollback after child tasks
are created requires the verified pre-migration backup so their hierarchy is
not silently flattened.

Migration `0034_priority_brief_notifications.sql` allows only high-priority
assistant briefs to join the existing durable push queue. Apply it to an empty
database and a restored version-33 backup, then verify existing task and
schedule deliveries remain valid, one pending urgency-2 brief queues only once
per active device and `jimin_schema_metadata.schema_version = 34`. Rollback
before a brief delivery is written may restore the previous item-type
constraint; after that point use the verified pre-migration backup so the
delivery audit is not discarded.

Migration `0035_project_operating_modes.sql` separates projects with a defined
finish line from continuously operated projects. Existing projects default to
completion mode so their current progress remains unchanged until the owner
chooses operation mode. Apply it to an empty database and a restored version-34
backup, then verify the new constraints and
`jimin_schema_metadata.schema_version = 35`. Rollback after a project mode is
changed requires the verified pre-migration backup so the owner's management
choice is not lost.

Migration `0039_conversation_surfaces.sql` marks one active conversation per
owner as the cross-device home assistant context. Existing installations choose
the most recently used active conversation during migration. Apply it to an
empty database and a restored version-38 backup, then verify that creating a new
home conversation archives the previous home context and
`jimin_schema_metadata.schema_version = 39`. Rollback requires the verified
pre-migration backup after clients begin relying on the durable home marker.

Migration `0040_google_chat_task_completion.sql` records an idempotent Google
Chat thread reply for every completion cycle of a task promoted from Chat.
Apply it to an empty database and a restored version-39 backup, then verify a
task completion queues one reply, retrying does not duplicate it, restoring the
task cancels an unsent reply, and
`jimin_schema_metadata.schema_version = 40`. Rollback requires the verified
pre-migration backup after a completion reply has been requested.

Migration `0042_android_device_signals.sql` adds owner- and device-scoped
Android signal health plus a 90-day private store for missed-call metadata.
Apply it to an empty database and a restored version-41 backup, then verify
that only an active Android device can upload, replaying the same provider call
ID is idempotent, another owner cannot read the rows, and
`jimin_schema_metadata.schema_version = 42`. The API and agent must not log
caller names or phone numbers. Rollback after call data is uploaded requires
the verified pre-migration backup so private device history is not silently
discarded.

Migration `0044_gmail_multi_account_workspaces.sql` separates Gmail credentials
from the single Calendar connection and assigns every Gmail account, sync
cursor, and message metadata row to an owner-scoped personal or company
workspace. Apply it to an empty database and a restored version-43 backup.
Before release, compare Gmail account, sync-state, and message counts before and
after the migration; confirm legacy Calendar-backed Gmail metadata is retained
under the personal workspace with reauthorization required; and verify Calendar
and Google Chat credentials are unchanged. Then connect one personal and one
company Gmail account, confirm cross-workspace reads are rejected, and verify
`jimin_schema_metadata.schema_version = 44`. The migration is forward-only.
After a Gmail account is authorized or synchronized, rollback requires the
verified pre-migration backup because the previous image cannot interpret the
workspace-scoped encrypted credentials or cache rows.

Migration `0046_gmail_history_cursor.sql` adds one server-only Gmail History
cursor to each account/workspace sync state. Existing accounts intentionally
start with a null cursor and establish a fresh provider baseline before using
incremental History synchronization. Apply it to an empty database and a
restored version-45 backup, then verify personal and company accounts advance
independently, a stale expected cursor cannot commit messages, reconnecting
preserves the cursor, and `jimin_schema_metadata.schema_version = 46`. The
migration is additive, but after a History cursor advances, rollback requires
the verified pre-migration backup so an older image cannot silently resume from
an unrelated inbox watermark.

Migration `0047_gmail_inflow_attention.sql` orders Gmail assistant candidates
by the latest source activity instead of their first-seen time. Existing rows
start at their latest stored update time; a later message on a dismissed,
deferred, or promoted thread returns that thread to review without removing its
linked task. Apply it to an empty database and a restored version-46 backup,
then verify that an older thread with a new reply moves to the first page and
keyset pagination remains duplicate-free. Confirm
`jimin_schema_metadata.schema_version = 47`. The migration is additive, but
rollback after new replies are ingested requires the verified pre-migration
backup so the previous image does not silently hide recent thread activity.

Rollback uses the previous image together with a verified database restore. Do not edit an applied migration; add a new compatible migration instead.

Migration `0048_google_chat_follow_up_attention.sql` lets a reply on an
already-promoted Google Chat thread return to pending attention while retaining
the existing task link. Apply it to an empty database and a restored version-47
backup, then verify that promoted threads accept a new pending reply with the
same `promoted_task_id` and that fresh pending candidates still have no task
link. Confirm `jimin_schema_metadata.schema_version = 48`.

Rollback uses the previous image together with a verified database restore.
Rows containing pending or dismissed follow-up attention must be handled before
restoring the older promotion-state constraint, so rollback should restore the
verified pre-migration backup rather than mutate those links in place.

Migration `0049_inflow_reference_documents.sql` stores up to four bounded,
read-only source snapshots alongside each Google Chat inflow analysis. Apply it
to an empty database and a restored version-48 backup, then verify existing
analyses receive an empty JSON array, an ITSM-enriched analysis persists only
validated HTTPS references, and `jimin_schema_metadata.schema_version = 49`.
The migration is additive. After enriched analyses are written, rollback must
use the verified pre-migration backup because the older worker cannot preserve
the evidence used to produce its task summary.

Migration `0050_task_assignment_public_details.sql` stores the reviewed public
assignment projection separately from editable task notes and immutable inflow
evidence. Apply it to an empty database and a restored version-49 backup, then
verify promoted tasks persist only bounded summary, action item, completion
criterion, and reference-link arrays and that
`jimin_schema_metadata.schema_version = 50`. Existing tasks remain valid
without a projection and use their explicitly public notes as the notification
summary. Rollback uses the previous image together with a verified version-49
backup after new task assignment projections have been written.

Migration `0051_project_itsm_connections.sql` replaces deployment-maintained
Google Chat source UUID allowlists with an owner- and project-scoped ITSM
connection. The table stores only connection metadata; the trusted origin and
read-only token remain deployment secrets mounted only into the agent. Each
connection records a non-secret positive decimal ITSM project identifier, and
the agent rejects issue content whose `issue.project.id` does not match it.
Apply it to an empty database and a restored version-50 backup, verify existing
projects and inflow analyses are unchanged, and confirm
`jimin_schema_metadata.schema_version = 51`. After project connection rows are
written, rollback uses the verified version-50 backup so an older worker cannot
silently ignore the owner's enrichment boundary.
