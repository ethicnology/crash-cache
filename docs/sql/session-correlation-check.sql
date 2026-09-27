-- Read-only regression cases for the dashboard's session UUID correlation.
-- A successful run returns zero rows. Identity and project checks are mandatory.
WITH cases(name, stored_sid, observed_sid, stored_project, observed_project,
           stored_installation, observed_installation, expected) AS (
    VALUES
      ('compact UUID', '66f4e061-fe6a-4c1b-9587-a7956d000ca7', '66f4e061fe6a4c1b9587a7956d000ca7', 1, 1, 'install-a', 'install-a', true),
      ('hyphenated UUID', '66f4e061-fe6a-4c1b-9587-a7956d000ca7', '66f4e061-fe6a-4c1b-9587-a7956d000ca7', 1, 1, 'install-a', 'install-a', true),
      ('uppercase UUID', '66f4e061-fe6a-4c1b-9587-a7956d000ca7', '66F4E061FE6A4C1B9587A7956D000CA7', 1, 1, 'install-a', 'install-a', true),
      ('different project', '66f4e061-fe6a-4c1b-9587-a7956d000ca7', '66f4e061fe6a4c1b9587a7956d000ca7', 1, 2, 'install-a', 'install-a', false),
      ('different installation', '66f4e061-fe6a-4c1b-9587-a7956d000ca7', '66f4e061fe6a4c1b9587a7956d000ca7', 1, 1, 'install-a', 'install-b', false),
      ('missing installation', '66f4e061-fe6a-4c1b-9587-a7956d000ca7', '66f4e061fe6a4c1b9587a7956d000ca7', 1, 1, NULL, NULL, false),
      ('invalid separator', '66f4e061-fe6a-4c1b-9587-a7956d000ca7', '66f4e061f-e6a4c1b9587a7956d000ca7', 1, 1, 'install-a', 'install-a', false),
      ('invalid text', '66f4e061-fe6a-4c1b-9587-a7956d000ca7', 'invalid', 1, 1, 'install-a', 'install-a', false)
), checked AS (
    SELECT name, expected, coalesce(
        stored_project = observed_project
        AND stored_installation = observed_installation
        AND observed_sid ~* '^([0-9a-f]{32}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$'
        AND replace(lower(stored_sid), '-', '') = replace(lower(observed_sid), '-', ''), false) AS actual
    FROM cases
)
SELECT name, expected, actual FROM checked WHERE actual IS DISTINCT FROM expected;
