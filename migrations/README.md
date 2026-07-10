# Database migrations

Migrations are forward-only and are embedded in `jimin-storage` at build time.

Before applying a migration to production:

1. apply it to an empty PostgreSQL database;
2. apply it to a restored staging backup;
3. verify `jimin_schema_metadata` and SQLx migration versions;
4. create a production backup;
5. keep the previous image digest available.

Rollback uses the previous image together with a verified database restore. Do not edit an applied migration; add a new compatible migration instead.
