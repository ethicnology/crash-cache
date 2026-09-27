\set ON_ERROR_STOP on
\getenv app_password CRASH_CACHE_DB_PASSWORD
\getenv metabase_password METABASE_DB_PASSWORD
\getenv readonly_password METABASE_READONLY_PASSWORD
CREATE ROLE crash_cache LOGIN PASSWORD :'app_password';
CREATE ROLE metabase_app LOGIN PASSWORD :'metabase_password';
CREATE ROLE metabase_readonly LOGIN PASSWORD :'readonly_password';
ALTER DATABASE crash_cache OWNER TO crash_cache;
CREATE DATABASE metabase OWNER metabase_app;
REVOKE ALL ON DATABASE crash_cache FROM PUBLIC;
GRANT CONNECT ON DATABASE crash_cache TO crash_cache, metabase_readonly;
GRANT USAGE ON SCHEMA public TO metabase_readonly;
GRANT USAGE, CREATE ON SCHEMA public TO crash_cache;
ALTER DEFAULT PRIVILEGES FOR ROLE crash_cache IN SCHEMA public GRANT SELECT ON TABLES TO metabase_readonly;
