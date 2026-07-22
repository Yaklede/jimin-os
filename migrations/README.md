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

Rollback uses the previous image together with a verified database restore. Do not edit an applied migration; add a new compatible migration instead.
