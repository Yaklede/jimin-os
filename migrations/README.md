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

Rollback uses the previous image together with a verified database restore. Do not edit an applied migration; add a new compatible migration instead.
