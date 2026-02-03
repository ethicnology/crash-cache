-- PostgreSQL supports ALTER COLUMN TYPE directly
-- This migration changes frames_json from BYTEA to TEXT
-- Note: For PostgreSQL, the initial schema already uses TEXT, so this is a no-op
-- This migration exists for consistency with SQLite migration history

-- If the column was BYTEA, we would do:
-- ALTER TABLE unwrap_stacktrace ALTER COLUMN frames_json TYPE TEXT USING convert_from(frames_json, 'UTF8');

-- Since PostgreSQL initial schema already has TEXT, this is intentionally empty
SELECT 1;
