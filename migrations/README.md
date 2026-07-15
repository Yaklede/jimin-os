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

Rollback uses the previous image together with a verified database restore. Do not edit an applied migration; add a new compatible migration instead.
