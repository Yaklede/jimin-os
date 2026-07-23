-- High-priority assistant recommendations may now use the same durable push
-- queue as task and schedule reminders. Existing deliveries are unchanged.
ALTER TABLE push_deliveries
DROP CONSTRAINT IF EXISTS push_deliveries_item_type_check;

ALTER TABLE push_deliveries
ADD CONSTRAINT push_deliveries_item_type_check
CHECK (item_type IN ('task', 'schedule', 'brief'));

UPDATE jimin_schema_metadata
SET schema_version = 34
WHERE singleton = TRUE;
