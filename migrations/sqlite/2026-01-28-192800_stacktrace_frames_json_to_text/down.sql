-- Revert TEXT back to BLOB

-- Step 1: Create a new table with BLOB
CREATE TABLE unwrap_stacktrace_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    hash TEXT UNIQUE NOT NULL,
    fingerprint_hash TEXT,
    frames_json BLOB NOT NULL
);

-- Step 2: Copy data from TEXT table to BLOB table
INSERT INTO unwrap_stacktrace_new (id, hash, fingerprint_hash, frames_json)
SELECT id, hash, fingerprint_hash, CAST(frames_json AS BLOB)
FROM unwrap_stacktrace;

-- Step 3: Drop the TEXT table
DROP TABLE unwrap_stacktrace;

-- Step 4: Rename the new table to the original name
ALTER TABLE unwrap_stacktrace_new RENAME TO unwrap_stacktrace;

-- Step 5: Recreate the index
CREATE INDEX IF NOT EXISTS idx_unwrap_stacktrace_fingerprint ON unwrap_stacktrace(fingerprint_hash);
