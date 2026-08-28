param()

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$ApiDir = Join-Path $RepoRoot "site\api"
$EnvPath = Join-Path $ApiDir ".env"

function Get-CkDatabaseUrl {
    if ($env:DATABASE_URL) {
        return $env:DATABASE_URL
    }

    if (-not (Test-Path $EnvPath)) {
        throw "DATABASE_URL is not set and $EnvPath does not exist."
    }

    $line = Get-Content $EnvPath | Where-Object { $_ -match '^\s*DATABASE_URL\s*=' } | Select-Object -First 1
    if (-not $line) {
        throw "DATABASE_URL was not found in $EnvPath."
    }

    return ($line -replace '^\s*DATABASE_URL\s*=\s*', '').Trim().Trim('"').Trim("'")
}

function Get-CkEnvValue {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [string]$DefaultValue = ""
    )

    $envValue = [Environment]::GetEnvironmentVariable($Name)
    if (-not [string]::IsNullOrWhiteSpace($envValue)) {
        return $envValue.Trim()
    }

    if (Test-Path $EnvPath) {
        $pattern = "^\s*$([regex]::Escape($Name))\s*="
        $line = Get-Content $EnvPath | Where-Object { $_ -match $pattern } | Select-Object -First 1
        if ($line) {
            return ($line -replace $pattern, "").Trim().Trim('"').Trim("'")
        }
    }

    return $DefaultValue
}

function Get-CkLocaleSlug {
    $value = Get-CkEnvValue -Name "CK_LOCALE_SLUG" -DefaultValue "world"
    return $value.Trim()
}

function Get-CkLocaleName {
    $value = Get-CkEnvValue -Name "CK_LOCALE_NAME" -DefaultValue "World"
    return $value.Trim()
}

function ConvertTo-CkSqlLiteral {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value
    )

    return "'" + $Value.Replace("'", "''") + "'"
}

function Resolve-CkConfiguredLocaleSql {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Sql
    )

    $localeSlugLiteral = ConvertTo-CkSqlLiteral (Get-CkLocaleSlug)
    $localeNameLiteral = ConvertTo-CkSqlLiteral (Get-CkLocaleName)

    return $Sql.Replace("'world'", $localeSlugLiteral).Replace("'World'", $localeNameLiteral)
}

function Get-CkPsql {
    $knownPath = "C:\Program Files\PostgreSQL\18\bin\psql.exe"
    if (Test-Path $knownPath) {
        return $knownPath
    }

    $command = Get-Command psql.exe -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }

    $command = Get-Command psql -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }

    throw "Could not find psql. Expected $knownPath or psql on PATH."
}

function Invoke-CkSql {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Sql
    )

    $psql = Get-CkPsql
    $databaseUrl = Get-CkDatabaseUrl
    $tmp = Join-Path $RepoRoot ".ck-dev-sql-$(Get-Random).sql"
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    $resolvedSql = Resolve-CkConfiguredLocaleSql $Sql
    [System.IO.File]::WriteAllText($tmp, $resolvedSql, $utf8NoBom)

    try {
        & $psql $databaseUrl -v ON_ERROR_STOP=1 -f $tmp
        if ($LASTEXITCODE -ne 0) {
            throw "psql exited with code $LASTEXITCODE."
        }
    } finally {
        Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue
    }
}

function Invoke-CkDemoSeeder {
    $env:CK_LOCALE_SLUG = Get-CkLocaleSlug
    $env:CK_LOCALE_NAME = Get-CkLocaleName

    Push-Location $ApiDir
    try {
        cargo run --bin seed_demo
        if ($LASTEXITCODE -ne 0) {
            throw "cargo run --bin seed_demo exited with code $LASTEXITCODE."
        }
    } finally {
        Pop-Location
    }
}

function Get-CkRefreshCountsSql {
@'
WITH counts AS (
    SELECT
        p.id,
        (
            SELECT COUNT(*)::int
            FROM proposal_sentiment_votes v
            WHERE v.proposal_id = p.id
              AND v.vote_value = 'support'
        ) AS support_count,
        (
            SELECT COUNT(*)::int
            FROM proposal_sentiment_votes v
            WHERE v.proposal_id = p.id
              AND v.vote_value = 'not_a_fit'
        ) AS not_a_fit_count,
        (
            SELECT COUNT(*)::int
            FROM proposal_sentiment_votes v
            WHERE v.proposal_id = p.id
              AND v.vote_value = 'unclear'
        ) AS unclear_count,
        (
            SELECT COUNT(*)::int
            FROM proposal_sentiment_votes v
            WHERE v.proposal_id = p.id
              AND v.vote_value = 'unsafe'
        ) AS unsafe_count,
        (
            SELECT COUNT(*)::int
            FROM proposal_merge_votes mv
            WHERE mv.proposal_id = p.id
              AND mv.target_proposal_id IS NOT NULL
              AND EXISTS (
                  SELECT 1
                  FROM proposals target
                  WHERE target.id = mv.target_proposal_id
                    AND target.primary_state = 'active'
              )
              AND EXISTS (
                  SELECT 1
                  FROM proposal_merge_relationships r
                  WHERE r.source_proposal_id = mv.proposal_id
                    AND r.target_proposal_id = mv.target_proposal_id
                    AND r.status = 'active'
              )
        ) AS merge_count
    FROM proposals p
)
UPDATE proposals p
SET
    support_count = counts.support_count,
    not_a_fit_count = counts.not_a_fit_count,
    unclear_count = counts.unclear_count,
    unsafe_count = counts.unsafe_count,
    merge_count = counts.merge_count,
    high_moderation_watch_started_at = CASE
        WHEN counts.unsafe_count >= 8
          OR (
            (counts.support_count + counts.not_a_fit_count + counts.unclear_count + counts.unsafe_count + counts.merge_count) > 0
            AND counts.unsafe_count::numeric
                / (counts.support_count + counts.not_a_fit_count + counts.unclear_count + counts.unsafe_count + counts.merge_count)::numeric >= 0.50
          )
        THEN COALESCE(p.high_moderation_watch_started_at, NOW())
        ELSE NULL
    END
FROM counts
WHERE counts.id = p.id;
'@
}

function Get-CkSeedMergeNotificationSql {
@'
WITH merge_watch_relationships AS (
    SELECT
        r.source_proposal_id,
        r.target_proposal_id,
        source.author_user_id,
        source.title AS source_title,
        target.title AS target_title,
        (
            source.support_count
            + source.not_a_fit_count
            + source.unclear_count
            + source.unsafe_count
            + source.merge_count
        ) AS source_total_count,
        source.merge_count AS source_merge_count
    FROM proposal_merge_relationships r
    JOIN proposals source ON source.id = r.source_proposal_id
    JOIN proposals target ON target.id = r.target_proposal_id
    WHERE r.status = 'active'
      AND source.primary_state = 'active'
      AND target.primary_state = 'active'
      AND (
            source.support_count
            + source.not_a_fit_count
            + source.unclear_count
            + source.unsafe_count
            + source.merge_count
          ) >= 10
      AND source.merge_count::numeric
          / NULLIF((
                source.support_count
                + source.not_a_fit_count
                + source.unclear_count
                + source.unsafe_count
                + source.merge_count
            ), 0)::numeric >= 0.20
),
notification_recipients AS (
    SELECT
        author_user_id AS recipient_user_id,
        'merge_watch_author' AS notification_type,
        source_proposal_id,
        target_proposal_id,
        source_title,
        target_title,
        source_total_count,
        source_merge_count
    FROM merge_watch_relationships

    UNION ALL

    SELECT
        moderator.id AS recipient_user_id,
        'merge_watch_moderator' AS notification_type,
        relationship.source_proposal_id,
        relationship.target_proposal_id,
        relationship.source_title,
        relationship.target_title,
        relationship.source_total_count,
        relationship.source_merge_count
    FROM merge_watch_relationships relationship
    JOIN users moderator
      ON moderator.role_code = 'moderator'
     AND moderator.email_verified = TRUE
)
INSERT INTO notification_events (
    recipient_user_id,
    notification_type,
    proposal_id,
    related_proposal_id,
    payload
)
SELECT
    recipient_user_id,
    notification_type,
    source_proposal_id,
    target_proposal_id,
    jsonb_build_object(
        'summary', 'Duplicate signals have reached the author distinction-note threshold.',
        'source_title', source_title,
        'target_title', target_title,
        'source_total_count', source_total_count,
        'source_merge_count', source_merge_count,
        'seeded', TRUE
    )
FROM notification_recipients
ON CONFLICT DO NOTHING;
'@
}

function Get-CkSeedImplementationSql {
@'
WITH moderator AS (
    SELECT id
    FROM users
    WHERE email = 'moderator@example.com'
    LIMIT 1
),
candidate AS (
    SELECT p.*
    FROM proposals p
    JOIN boards b ON b.id = p.board_id
    WHERE b.code = 'solution'
      AND p.parent_issue_proposal_id IS NOT NULL
      AND p.primary_state IN ('active', 'ranked', 'archived')
      AND p.action_description IS NOT NULL
      AND p.title IN (
          'Regional water testing lab network',
          'DEMO SOLUTION: Regional water lab network',
          'School drinking-water testing network'
      )
    ORDER BY
        CASE p.title
            WHEN 'Regional water testing lab network' THEN 1
            WHEN 'DEMO SOLUTION: Regional water lab network' THEN 1
            WHEN 'School drinking-water testing network' THEN 2
            ELSE 10
        END,
        p.support_count DESC,
        p.created_at DESC
    LIMIT 1
),
payload AS (
    SELECT
        candidate.id AS solution_proposal_id,
        candidate.parent_issue_proposal_id,
        candidate.cycle_id,
        candidate.locale_id,
        (SELECT id FROM moderator) AS moderator_user_id,
        candidate.title,
        COALESCE(
            NULLIF(BTRIM(candidate.action_description), ''),
            'Coordinate verified water testing capacity for underserved regions.'
        ) AS action_description,
        COALESCE(
            candidate.required_resource_categories,
            '["money", "equipment", "labor", "organizational support"]'::jsonb
        ) AS required_resource_categories,
        jsonb_build_array(
            jsonb_build_object(
                'criterion_description', 'Publish the shared lab directory and intake workflow for pilot communities.',
                'completion_status', 'completed',
                'evidence_note', 'Directory and intake workflow drafted for pilot partners.',
                'updated_at', '2026-05-30T00:00:00Z'
            ),
            jsonb_build_object(
                'criterion_description', 'Run three pilot testing cycles with public result summaries.',
                'completion_status', 'in_progress',
                'evidence_note', 'First pilot cycle is being coordinated with regional partners.',
                'updated_at', '2026-05-30T00:00:00Z'
            ),
            jsonb_build_object(
                'criterion_description', 'Confirm escalation contacts for urgent contamination findings.',
                'completion_status', 'not_started',
                'evidence_note', 'Escalation list will be verified after partner onboarding.',
                'updated_at', NULL
            )
        ) AS completion_criteria,
        jsonb_build_array(
            jsonb_build_object(
                'resource_category', 'money',
                'target_needed', '75000 USD',
                'target_amount', '75000',
                'target_unit', 'USD',
                'current_acquired_amount', '28500',
                'resource_status', 'in_progress',
                'external_coordination_link', 'https://example.org/water-lab-implementation-fund',
                'status_proof_note', 'Seeded local fixture: matching funds pledged by two regional partners.',
                'resource_updated_at', '2026-05-30T00:00:00Z'
            ),
            jsonb_build_object(
                'resource_category', 'equipment',
                'target_needed', '120 test kits',
                'target_amount', '120',
                'target_unit', 'test kits',
                'current_acquired_amount', '46',
                'resource_status', 'in_progress',
                'external_coordination_link', '',
                'status_proof_note', 'Initial equipment commitments logged for pilot testing sites.',
                'resource_updated_at', '2026-05-30T00:00:00Z'
            ),
            jsonb_build_object(
                'resource_category', 'labor',
                'target_needed', '8 lab partners',
                'target_amount', '8',
                'target_unit', 'lab partners',
                'current_acquired_amount', '5',
                'resource_status', 'in_progress',
                'external_coordination_link', '',
                'status_proof_note', 'Five partner labs identified; onboarding is still underway.',
                'resource_updated_at', '2026-05-30T00:00:00Z'
            )
        ) AS execution_tracking_entries,
        jsonb_build_object(
            'support_count', candidate.support_count,
            'not_a_fit_count', candidate.not_a_fit_count,
            'unclear_count', candidate.unclear_count,
            'unsafe_count', candidate.unsafe_count,
            'merge_count', candidate.merge_count,
            'seeded_implementation_fixture', TRUE
        ) AS proposal_vote_snapshot
    FROM candidate
),
upserted AS (
    INSERT INTO execution_records (
        solution_proposal_id,
        parent_issue_proposal_id,
        cycle_id,
        locale_id,
        created_by_moderator_user_id,
        title,
        action_description,
        required_resource_categories,
        completion_criteria,
        execution_tracking_entries,
        proposal_vote_snapshot,
        status
    )
    SELECT
        solution_proposal_id,
        parent_issue_proposal_id,
        cycle_id,
        locale_id,
        moderator_user_id,
        title,
        action_description,
        required_resource_categories,
        completion_criteria,
        execution_tracking_entries,
        proposal_vote_snapshot,
        'active'
    FROM payload
    ON CONFLICT ON CONSTRAINT execution_records_one_solution_per_issue_cycle
    DO UPDATE SET
        solution_proposal_id = EXCLUDED.solution_proposal_id,
        created_by_moderator_user_id = COALESCE(
            EXCLUDED.created_by_moderator_user_id,
            execution_records.created_by_moderator_user_id
        ),
        title = EXCLUDED.title,
        action_description = EXCLUDED.action_description,
        required_resource_categories = EXCLUDED.required_resource_categories,
        completion_criteria = EXCLUDED.completion_criteria,
        execution_tracking_entries = EXCLUDED.execution_tracking_entries,
        proposal_vote_snapshot = EXCLUDED.proposal_vote_snapshot,
        status = 'active',
        updated_at = NOW()
    RETURNING id, title
)
SELECT 'seeded_implementation=' || title || ' id=' || id
FROM upserted
UNION ALL
SELECT 'seeded_implementation=none'
WHERE NOT EXISTS (SELECT 1 FROM upserted);
'@
}

function Get-CkSeedModerationHoldScenarioSql {
@'
WITH scenario_titles(title, support_count, not_fit_count, unclear_count, unsafe_count, hold_started_at) AS (
    VALUES
        ('Mobile water testing training corps', 10, 1, 0, 8, NOW() - INTERVAL '2 days'),
        ('DEMO SOLUTION: Mobile water testing training corps', 10, 1, 0, 8, NOW() - INTERVAL '2 days'),
        ('Regional water testing lab network', 22, 1, 0, 8, NOW()),
        ('DEMO SOLUTION: Regional water lab network', 22, 1, 0, 8, NOW())
),
scenario_proposals AS (
    SELECT
        p.id AS proposal_id,
        p.title,
        s.support_count,
        s.not_fit_count,
        s.unclear_count,
        s.unsafe_count,
        s.hold_started_at
    FROM proposals p
    JOIN boards b ON b.id = p.board_id
    JOIN scenario_titles s ON s.title = p.title
    WHERE b.code = 'solution'
)
UPDATE proposals p
SET
    primary_state = 'active',
    archived_reason = NULL,
    moderation_note = NULL,
    merged_into_proposal_id = NULL
FROM scenario_proposals sp
WHERE p.id = sp.proposal_id;

WITH scenario_titles(title) AS (
    VALUES
        ('Mobile water testing training corps'),
        ('DEMO SOLUTION: Mobile water testing training corps'),
        ('Regional water testing lab network'),
        ('DEMO SOLUTION: Regional water lab network')
)
UPDATE proposal_watch_flags wf
SET
    cleared_at = NOW(),
    clearance_reason = 'dev_moderation_hold_scenario'
FROM proposals p
JOIN scenario_titles s ON s.title = p.title
WHERE wf.proposal_id = p.id
  AND wf.flag_code = 'frozen_for_review'
  AND wf.cleared_at IS NULL;

WITH scenario_titles(title) AS (
    VALUES
        ('Mobile water testing training corps'),
        ('DEMO SOLUTION: Mobile water testing training corps'),
        ('Regional water testing lab network'),
        ('DEMO SOLUTION: Regional water lab network')
)
DELETE FROM proposal_sentiment_votes v
USING proposals p, scenario_titles s
WHERE v.proposal_id = p.id
  AND p.title = s.title;

WITH scenario_titles(title, support_count, not_fit_count, unclear_count, unsafe_count) AS (
    VALUES
        ('Mobile water testing training corps', 10, 1, 0, 8),
        ('DEMO SOLUTION: Mobile water testing training corps', 10, 1, 0, 8),
        ('Regional water testing lab network', 22, 1, 0, 8),
        ('DEMO SOLUTION: Regional water lab network', 22, 1, 0, 8)
),
voters AS (
    SELECT
        id,
        ROW_NUMBER() OVER (ORDER BY email) AS rn
    FROM users
    WHERE email LIKE 'seed-voter-%@example.test'
),
votes AS (
    SELECT
        p.id AS proposal_id,
        voters.id AS user_id,
        CASE
            WHEN voters.rn <= scenario_titles.support_count THEN 'support'
            WHEN voters.rn <= scenario_titles.support_count + scenario_titles.not_fit_count THEN 'not_a_fit'
            WHEN voters.rn <= scenario_titles.support_count + scenario_titles.not_fit_count + scenario_titles.unclear_count THEN 'unclear'
            ELSE 'unsafe'
        END AS vote_value
    FROM scenario_titles
    JOIN proposals p ON p.title = scenario_titles.title
    JOIN boards b ON b.id = p.board_id AND b.code = 'solution'
    JOIN voters
      ON voters.rn <= scenario_titles.support_count
                    + scenario_titles.not_fit_count
                    + scenario_titles.unclear_count
                    + scenario_titles.unsafe_count
)
INSERT INTO proposal_sentiment_votes (proposal_id, user_id, vote_value)
SELECT proposal_id, user_id, vote_value
FROM votes
ON CONFLICT (proposal_id, user_id)
DO UPDATE SET vote_value = EXCLUDED.vote_value, updated_at = NOW();

WITH scenario_titles(title, support_count, not_fit_count, unclear_count, unsafe_count) AS (
    VALUES
        ('Mobile water testing training corps', 10, 1, 0, 8),
        ('DEMO SOLUTION: Mobile water testing training corps', 10, 1, 0, 8),
        ('Regional water testing lab network', 22, 1, 0, 8),
        ('DEMO SOLUTION: Regional water lab network', 22, 1, 0, 8)
)
UPDATE proposals p
SET
    support_count = scenario_titles.support_count,
    not_a_fit_count = scenario_titles.not_fit_count,
    unclear_count = scenario_titles.unclear_count,
    unsafe_count = scenario_titles.unsafe_count
FROM scenario_titles
JOIN boards b ON b.code = 'solution'
WHERE p.title = scenario_titles.title
  AND p.board_id = b.id;

WITH scenario_titles(title, hold_started_at) AS (
    VALUES
        ('Mobile water testing training corps', NOW() - INTERVAL '2 days'),
        ('DEMO SOLUTION: Mobile water testing training corps', NOW() - INTERVAL '2 days'),
        ('Regional water testing lab network', NOW()),
        ('DEMO SOLUTION: Regional water lab network', NOW())
)
UPDATE proposals p
SET high_moderation_watch_started_at = scenario_titles.hold_started_at
FROM scenario_titles
JOIN boards b ON b.code = 'solution'
WHERE p.title = scenario_titles.title
  AND p.board_id = b.id;

SELECT
    'moderation_hold_fixture' AS fixture,
    p.title,
    p.support_count,
    p.not_a_fit_count,
    p.unclear_count,
    p.unsafe_count,
    p.merge_count,
    p.high_moderation_watch_started_at,
    CASE
        WHEN p.high_moderation_watch_started_at <= NOW() - INTERVAL '24 hours' THEN 'action_ready'
        ELSE 'hold_active'
    END AS expected_review_state
FROM proposals p
JOIN boards b ON b.id = p.board_id
WHERE b.code = 'solution'
  AND p.title IN (
    'Mobile water testing training corps',
    'DEMO SOLUTION: Mobile water testing training corps',
    'Regional water testing lab network',
    'DEMO SOLUTION: Regional water lab network'
  )
ORDER BY p.title;
'@
}

function Reset-CkDevAccounts {
    $sql = @'
BEGIN;

INSERT INTO users (email, password_hash, email_verified, role_code, last_login_at)
VALUES
    ('user@example.com', '$argon2id$v=19$m=19456,t=2,p=1$eJwzfITAbkxStdeO/EmxCQ$BheunlM5+Wphkx43WK6c/QVd85IOmN8A418mGk5bAfo', TRUE, 'registered_user', NULL),
    ('test2@example.com', '$argon2id$v=19$m=19456,t=2,p=1$eJwzfITAbkxStdeO/EmxCQ$BheunlM5+Wphkx43WK6c/QVd85IOmN8A418mGk5bAfo', TRUE, 'registered_user', NULL),
    ('moderator@example.com', '$argon2id$v=19$m=19456,t=2,p=1$eJwzfITAbkxStdeO/EmxCQ$BheunlM5+Wphkx43WK6c/QVd85IOmN8A418mGk5bAfo', TRUE, 'moderator', NULL)
ON CONFLICT (email)
DO UPDATE SET
    password_hash = EXCLUDED.password_hash,
    email_verified = EXCLUDED.email_verified,
    role_code = EXCLUDED.role_code,
    last_login_at = EXCLUDED.last_login_at;

DELETE FROM sessions
WHERE user_id IN (
    SELECT id FROM users
    WHERE email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
);

DELETE FROM email_verification_tokens
WHERE user_id IN (
    SELECT id FROM users
    WHERE email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
);

DELETE FROM password_reset_tokens
WHERE user_id IN (
    SELECT id FROM users
    WHERE email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
);

DELETE FROM notification_events
WHERE recipient_user_id IN (
    SELECT id FROM users
    WHERE email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
);

DELETE FROM anti_abuse_flags
WHERE user_id IN (
    SELECT id FROM users
    WHERE email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
)
OR reviewed_by_moderator_user_id IN (
    SELECT id FROM users
    WHERE email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
);

DELETE FROM user_activity_events
WHERE user_id IN (
    SELECT id FROM users
    WHERE email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
);

COMMIT;
'@
    Invoke-CkSql $sql
    Write-Host "Reset dev accounts. Password for all three is SuperSecurePass123."
}

function Reset-CkDatabaseFull {
    Write-Host "Ensuring migrations are applied with the Rust demo seeder..."
    Invoke-CkDemoSeeder

    $sql = @'
BEGIN;

TRUNCATE TABLE
    anti_abuse_flags,
    user_activity_events,
    notification_events,
    cycle_results,
    execution_records,
    proposal_merge_vote_reconciliations,
    reconsideration_windows,
    appeals,
    moderator_actions,
    proposal_watch_flags,
    merge_distinction_notes,
    proposal_merge_relationships,
    proposal_comment_votes,
    proposal_comments,
    proposal_merge_votes,
    proposal_sentiment_votes,
    review_actions,
    review_unlocks,
    proposals,
    cycles,
    sessions,
    email_verification_tokens,
    password_reset_tokens
CASCADE;

INSERT INTO locales (slug, name, is_active)
VALUES ('world', 'World', TRUE)
ON CONFLICT (slug)
DO UPDATE SET name = EXCLUDED.name, is_active = TRUE;

INSERT INTO boards (code, name, is_active)
VALUES
    ('issue', 'Issue Board', TRUE),
    ('solution', 'Solution Board', TRUE),
    ('archive', 'Archive Board', TRUE)
ON CONFLICT (code)
DO UPDATE SET name = EXCLUDED.name, is_active = TRUE;

COMMIT;
'@

    Invoke-CkSql $sql
    Reset-CkDevAccounts
    Invoke-CkDemoSeeder

    Show-CkSeedSummary
}

function Reset-CkNoPriorWinnerScenario {
    Write-Host "Ensuring migrations are applied with the Rust demo seeder..."
    Invoke-CkDemoSeeder

    $sql = @'
BEGIN;

TRUNCATE TABLE
    anti_abuse_flags,
    user_activity_events,
    notification_events,
    cycle_results,
    execution_records,
    proposal_merge_vote_reconciliations,
    reconsideration_windows,
    appeals,
    moderator_actions,
    proposal_watch_flags,
    merge_distinction_notes,
    proposal_merge_relationships,
    proposal_comment_votes,
    proposal_comments,
    proposal_merge_votes,
    proposal_sentiment_votes,
    review_actions,
    review_unlocks,
    proposals,
    cycles,
    sessions,
    email_verification_tokens,
    password_reset_tokens
CASCADE;

INSERT INTO locales (slug, name, is_active)
VALUES ('world', 'World', TRUE)
ON CONFLICT (slug)
DO UPDATE SET name = EXCLUDED.name, is_active = TRUE;

INSERT INTO boards (code, name, is_active)
VALUES
    ('issue', 'Issue Board', TRUE),
    ('solution', 'Solution Board', TRUE),
    ('archive', 'Archive Board', TRUE)
ON CONFLICT (code)
DO UPDATE SET name = EXCLUDED.name, is_active = TRUE;

INSERT INTO cycles (
    locale_id,
    cycle_number,
    starts_at,
    submission_ends_at,
    voting_ends_at,
    is_active
)
SELECT
    id,
    1,
    date_trunc('month', NOW()),
    date_trunc('month', NOW()) + INTERVAL '1 month',
    date_trunc('month', NOW()) + INTERVAL '1 month',
    TRUE
FROM locales
WHERE slug = 'world';

COMMIT;
'@

    Invoke-CkSql $sql
    Reset-CkDevAccounts
    Show-CkNoWinnerScenarioSummary
}

function New-CkLowParticipationNoWinnerScenario {
    Reset-CkNoPriorWinnerScenario

    $sql = @'
BEGIN;

WITH active_cycle AS (
    SELECT c.id AS cycle_id, c.locale_id
    FROM cycles c
    JOIN locales l ON l.id = c.locale_id
    WHERE l.slug = 'world'
      AND c.is_active = TRUE
    LIMIT 1
),
issue_board AS (
    SELECT id AS board_id
    FROM boards
    WHERE code = 'issue'
),
authors AS (
    SELECT
        email,
        id,
        ROW_NUMBER() OVER (ORDER BY email) AS rn
    FROM users
    WHERE email IN ('test2@example.com', 'user@example.com')
),
inserted AS (
    INSERT INTO proposals (
        board_id,
        cycle_id,
        locale_id,
        author_user_id,
        title,
        problem_description,
        affected_scope,
        why_it_matters,
        support_count,
        created_at
    )
    SELECT
        issue_board.board_id,
        active_cycle.cycle_id,
        active_cycle.locale_id,
        authors.id,
        'SCENARIO LOW PARTICIPATION: Issue ' || authors.rn,
        'Low-participation issue candidate ' || authors.rn || '.',
        'World',
        'Verifies that very small cycles close without producing a ranked winner.',
        1,
        NOW() + (authors.rn || ' seconds')::interval
    FROM active_cycle, issue_board, authors
    RETURNING id, author_user_id
),
cross_votes AS (
    SELECT
        inserted.id AS proposal_id,
        authors.id AS voter_id
    FROM inserted
    JOIN authors ON authors.id <> inserted.author_user_id
)
INSERT INTO proposal_sentiment_votes (proposal_id, user_id, vote_value)
SELECT proposal_id, voter_id, 'support'
FROM cross_votes;

COMMIT;
'@

    Invoke-CkSql $sql
    Show-CkNoWinnerScenarioSummary
}

function Show-CkNoWinnerScenarioSummary {
    $sql = @'
WITH active_cycle AS (
    SELECT c.id, c.locale_id, c.cycle_number
    FROM cycles c
    JOIN locales l ON l.id = c.locale_id
    WHERE l.slug = 'world'
      AND c.is_active = TRUE
    LIMIT 1
),
latest_issue_winner AS (
    SELECT cr.winning_proposal_id
    FROM cycle_results cr
    JOIN cycles c ON c.id = cr.cycle_id
    JOIN active_cycle ac ON ac.locale_id = cr.locale_id
    WHERE cr.board_code = 'issue'
      AND cr.result_status = 'resolved'
      AND cr.winning_proposal_id IS NOT NULL
      AND cr.published_at IS NOT NULL
      AND c.cycle_number < ac.cycle_number
    ORDER BY c.cycle_number DESC, cr.published_at DESC
    LIMIT 1
)
SELECT
    ac.cycle_number,
    (SELECT COUNT(*) FROM proposals p WHERE p.cycle_id = ac.id) AS active_cycle_proposals,
    (SELECT COUNT(*) FROM latest_issue_winner) AS solution_target_count
FROM active_cycle ac;

WITH active_cycle AS (
    SELECT c.id
    FROM cycles c
    JOIN locales l ON l.id = c.locale_id
    WHERE l.slug = 'world'
      AND c.is_active = TRUE
    LIMIT 1
)
SELECT
    b.code AS board,
    p.title,
    p.primary_state,
    p.support_count,
    p.not_a_fit_count,
    p.unclear_count,
    p.unsafe_count,
    p.merge_count
FROM proposals p
JOIN boards b ON b.id = p.board_id
JOIN active_cycle ac ON ac.id = p.cycle_id
ORDER BY b.code, p.created_at;
'@

    Invoke-CkSql $sql
}

function Stage-CkRealisticEnvironment {
    Write-Host "Resetting baseline before staging realistic local content..."
    Reset-CkDatabaseFull

    $refresh = Get-CkRefreshCountsSql
    $notifications = Get-CkSeedMergeNotificationSql
    $moderationHold = Get-CkSeedModerationHoldScenarioSql
    $implementation = Get-CkSeedImplementationSql
    $sql = @"
BEGIN;

UPDATE proposals
SET title = CASE title
        WHEN 'DEMO ISSUE: Clean water access gap' THEN 'Clean water access gaps in rural communities'
        WHEN 'DEMO ISSUE: AI transition safety net' THEN 'Worker retraining gaps during AI adoption'
        WHEN 'DEMO ISSUE: Antimicrobial resistance surge' THEN 'Rising antimicrobial resistance in community care'
        WHEN 'DEMO ISSUE: Duplicate clean water access framing' THEN 'Fragmented water-quality reporting systems'
        WHEN 'DEMO PRIOR WINNER: Clean water as current solution target' THEN 'Reliable drinking water access in underserved regions'
        WHEN 'DEMO SOLUTION: Regional water lab network' THEN 'Regional water testing lab network'
        WHEN 'DEMO SOLUTION: Water safety training toolkit' THEN 'Water safety training toolkit'
        WHEN 'DEMO SOLUTION: Contamination alert verification hub' THEN 'Contamination alert verification hub'
        WHEN 'DEMO SOLUTION: Mobile water testing training corps' THEN 'Mobile water testing training corps'
        ELSE title
    END
WHERE title IN (
    'DEMO ISSUE: Clean water access gap',
    'DEMO ISSUE: AI transition safety net',
    'DEMO ISSUE: Antimicrobial resistance surge',
    'DEMO ISSUE: Duplicate clean water access framing',
    'DEMO PRIOR WINNER: Clean water as current solution target',
    'DEMO SOLUTION: Regional water lab network',
    'DEMO SOLUTION: Water safety training toolkit',
    'DEMO SOLUTION: Contamination alert verification hub',
    'DEMO SOLUTION: Mobile water testing training corps'
);

WITH ids AS (
    SELECT
        (SELECT id FROM boards WHERE code = 'issue') AS issue_board_id,
        (SELECT id FROM boards WHERE code = 'solution') AS solution_board_id,
        (SELECT id FROM cycles WHERE is_active = TRUE ORDER BY created_at DESC LIMIT 1) AS cycle_id,
        (SELECT locale_id FROM cycles WHERE is_active = TRUE ORDER BY created_at DESC LIMIT 1) AS locale_id,
        (SELECT id FROM users WHERE email = 'seed-author-issue@example.test') AS issue_author_id,
        (SELECT id FROM users WHERE email = 'seed-author-solution@example.test') AS solution_author_id,
        (
            SELECT cr.winning_proposal_id
            FROM cycle_results cr
            JOIN cycles c ON c.id = cr.cycle_id
            WHERE cr.board_code = 'issue'
              AND cr.result_status = 'resolved'
              AND cr.published_at IS NOT NULL
              AND cr.winning_proposal_id IS NOT NULL
            ORDER BY c.cycle_number DESC, cr.published_at DESC
            LIMIT 1
        ) AS solution_target_issue_id
),
issue_seed (title, problem_description, affected_scope, why_it_matters) AS (
    VALUES
        (
            'Heat-resilient housing for older renters',
            'Older renters in poorly insulated apartments face dangerous indoor temperatures during longer and more frequent heat waves.',
            'Older adults, disabled renters, and low-income households in dense urban neighborhoods.',
            'Heat exposure is already causing preventable emergency room visits and can become fatal when housing cannot stay cool.'
        ),
        (
            'Food supply disruption during regional floods',
            'Flooding can sever local transport routes and interrupt grocery, medicine, and emergency supply deliveries for days.',
            'Flood-prone regions, rural towns, and neighborhoods with limited nearby food access.',
            'Short supply interruptions quickly become health and safety risks when people cannot replenish food, medication, or clean water.'
        ),
        (
            'Youth mental health triage delays',
            'Families and schools often cannot get timely triage when a young person shows escalating mental health risk.',
            'Students, caregivers, school staff, and community clinics with limited behavioral health capacity.',
            'Early triage can prevent crises, reduce emergency interventions, and connect young people to the right level of care sooner.'
        ),
        (
            'Medical debt blocking primary care access',
            'People with unresolved medical debt often delay routine care even when their current health needs are manageable.',
            'Uninsured and underinsured patients, especially households living paycheck to paycheck.',
            'Delayed primary care turns manageable conditions into more expensive emergencies and deepens household financial instability.'
        )
)
INSERT INTO proposals (
    board_id,
    cycle_id,
    locale_id,
    author_user_id,
    title,
    problem_description,
    affected_scope,
    why_it_matters,
    primary_state
)
SELECT
    ids.issue_board_id,
    ids.cycle_id,
    ids.locale_id,
    ids.issue_author_id,
    issue_seed.title,
    issue_seed.problem_description,
    issue_seed.affected_scope,
    issue_seed.why_it_matters,
    'active'
FROM ids
CROSS JOIN issue_seed
WHERE NOT EXISTS (
    SELECT 1
    FROM proposals existing
    WHERE existing.title = issue_seed.title
);

WITH ids AS (
    SELECT
        (SELECT id FROM boards WHERE code = 'solution') AS solution_board_id,
        (SELECT id FROM cycles WHERE is_active = TRUE ORDER BY created_at DESC LIMIT 1) AS cycle_id,
        (SELECT locale_id FROM cycles WHERE is_active = TRUE ORDER BY created_at DESC LIMIT 1) AS locale_id,
        (SELECT id FROM users WHERE email = 'seed-author-solution@example.test') AS solution_author_id,
        (
            SELECT cr.winning_proposal_id
            FROM cycle_results cr
            JOIN cycles c ON c.id = cr.cycle_id
            WHERE cr.board_code = 'issue'
              AND cr.result_status = 'resolved'
              AND cr.published_at IS NOT NULL
              AND cr.winning_proposal_id IS NOT NULL
            ORDER BY c.cycle_number DESC, cr.published_at DESC
            LIMIT 1
        ) AS solution_target_issue_id
),
solution_seed (
    title,
    action_description,
    why_it_matters,
    required_resource_categories,
    completion_criteria,
    execution_tracking_entries
) AS (
    VALUES
        (
            'Household filtration kit distribution',
            'Distribute certified household filtration kits and replacement cartridges to underserved households with known water-quality risk.',
            'Filtration kits create an immediate household-level barrier against known water-quality risks while longer infrastructure repairs remain slow or unavailable.',
            '["money", "materials", "logistics / transport", "organizational support"]'::jsonb,
            '[{"criterion_description":"At least 1,000 eligible households receive a verified filtration kit and replacement cartridge plan.","completion_status":"not_started","evidence_note":"Distribution logs and recipient attestations will confirm completion.","updated_at":null}]'::jsonb,
            '[{"resource_category":"materials","target_needed":"1,000 filtration kits","target_amount":"1000","target_unit":"filtration kits","current_acquired_amount":"","external_coordination_link":"","status_proof_note":""}]'::jsonb
        ),
        (
            'Emergency bottled-water reserve network',
            'Create shared emergency water reserve plans between utilities, local governments, distributors, and community pickup sites.',
            'Pre-positioned reserve plans reduce the delay between local water failure and safe drinking-water access for households without backup options.',
            '["logistics / transport", "organizational support", "materials"]'::jsonb,
            '[{"criterion_description":"Three underserved regions publish tested emergency drinking-water distribution playbooks.","completion_status":"not_started","evidence_note":"Exercise reports and partner sign-offs will verify readiness.","updated_at":null}]'::jsonb,
            '[{"resource_category":"logistics / transport","target_needed":"3 regional reserve plans","target_amount":"3","target_unit":"regional reserve plans","current_acquired_amount":"","external_coordination_link":"","status_proof_note":""}]'::jsonb
        ),
        (
            'School drinking-water testing network',
            'Set up recurring drinking-water testing and public reporting for schools in underserved regions.',
            'Routine testing at schools creates early warning data for children and families who otherwise may not know their drinking water has become unsafe.',
            '["labor", "skills / trades", "equipment", "organizational support"]'::jsonb,
            '[{"criterion_description":"At least 50 schools complete baseline drinking-water testing with published results.","completion_status":"not_started","evidence_note":"Lab reports and school dashboards will verify testing coverage.","updated_at":null}]'::jsonb,
            '[{"resource_category":"equipment","target_needed":"50 school test kits","target_amount":"50","target_unit":"school test kits","current_acquired_amount":"","external_coordination_link":"","status_proof_note":""}]'::jsonb
        ),
        (
            'Well repair microgrant navigator program',
            'Place navigators with local partners to help low-income households apply for well repair, testing, and remediation assistance.',
            'Navigation support turns existing repair and testing assistance into actual household fixes by helping underserved residents complete complex applications.',
            '["money", "labor", "skills / trades", "organizational support"]'::jsonb,
            '[{"criterion_description":"At least 200 households complete verified well repair, testing, or remediation applications.","completion_status":"not_started","evidence_note":"Navigator case logs and application confirmations will verify completion.","updated_at":null}]'::jsonb,
            '[{"resource_category":"labor","target_needed":"6 trained navigator roles","target_amount":"6","target_unit":"trained navigator roles","current_acquired_amount":"","external_coordination_link":"","status_proof_note":""}]'::jsonb
        )
)
INSERT INTO proposals (
    board_id,
    cycle_id,
    locale_id,
    author_user_id,
    parent_issue_proposal_id,
    title,
    action_description,
    why_it_matters,
    required_resource_categories,
    completion_criteria,
    execution_tracking_entries,
    primary_state
)
SELECT
    ids.solution_board_id,
    ids.cycle_id,
    ids.locale_id,
    ids.solution_author_id,
    ids.solution_target_issue_id,
    solution_seed.title,
    solution_seed.action_description,
    solution_seed.why_it_matters,
    solution_seed.required_resource_categories,
    solution_seed.completion_criteria,
    solution_seed.execution_tracking_entries,
    'active'
FROM ids
CROSS JOIN solution_seed
WHERE ids.solution_target_issue_id IS NOT NULL
  AND NOT EXISTS (
    SELECT 1
    FROM proposals existing
    WHERE existing.title = solution_seed.title
);

WITH staged_titles(title) AS (
    VALUES
        ('Heat-resilient housing for older renters'),
        ('Food supply disruption during regional floods'),
        ('Youth mental health triage delays'),
        ('Medical debt blocking primary care access'),
        ('Household filtration kit distribution'),
        ('Emergency bottled-water reserve network'),
        ('School drinking-water testing network'),
        ('Well repair microgrant navigator program')
)
DELETE FROM proposal_sentiment_votes v
USING proposals p, staged_titles st
WHERE v.proposal_id = p.id
  AND p.title = st.title;

WITH vote_plan(title, support_count, not_fit_count, unclear_count, unsafe_count) AS (
    VALUES
        ('Heat-resilient housing for older renters', 5, 1, 1, 0),
        ('Food supply disruption during regional floods', 12, 5, 2, 0),
        ('Youth mental health triage delays', 7, 7, 1, 0),
        ('Medical debt blocking primary care access', 15, 2, 0, 0),
        ('Household filtration kit distribution', 6, 1, 1, 0),
        ('Emergency bottled-water reserve network', 8, 8, 2, 0),
        ('School drinking-water testing network', 13, 2, 1, 0),
        ('Well repair microgrant navigator program', 5, 0, 2, 0)
),
voters AS (
    SELECT
        id,
        ROW_NUMBER() OVER (ORDER BY email) AS rn
    FROM users
    WHERE email LIKE 'seed-voter-%@example.test'
),
votes AS (
    SELECT
        p.id AS proposal_id,
        voters.id AS user_id,
        CASE
            WHEN voters.rn <= vote_plan.support_count THEN 'support'
            WHEN voters.rn <= vote_plan.support_count + vote_plan.not_fit_count THEN 'not_a_fit'
            WHEN voters.rn <= vote_plan.support_count + vote_plan.not_fit_count + vote_plan.unclear_count THEN 'unclear'
            ELSE 'unsafe'
        END AS vote_value
    FROM vote_plan
    JOIN proposals p ON p.title = vote_plan.title
    JOIN voters
      ON voters.rn <= vote_plan.support_count
                    + vote_plan.not_fit_count
                    + vote_plan.unclear_count
                    + vote_plan.unsafe_count
)
INSERT INTO proposal_sentiment_votes (proposal_id, user_id, vote_value)
SELECT proposal_id, user_id, vote_value
FROM votes
ON CONFLICT (proposal_id, user_id)
DO UPDATE SET vote_value = EXCLUDED.vote_value, updated_at = NOW();

DELETE FROM notification_events;

$refresh

$moderationHold

$refresh

$implementation

$notifications

COMMIT;
"@

    Invoke-CkSql $sql
    Write-Host "Staged a realistic local environment. Titles no longer use DEMO prefixes; use Reset-CkDatabaseFull before baseline sanity checks."
    Write-Host "Dev account login state was reset; browser tutorial dismissal flags are local, but first-login app state will show tutorials again after login."
    Show-CkSeedSummary
}

function Show-CkSeedSummary {
    $sql = @'
SELECT
    u.email,
    u.role_code,
    u.email_verified,
    u.last_login_at
FROM users u
WHERE u.email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
ORDER BY u.email;

SELECT
    b.code AS board,
    p.primary_state,
    COUNT(*) AS proposal_count
FROM proposals p
JOIN boards b ON b.id = p.board_id
GROUP BY b.code, p.primary_state
ORDER BY b.code, p.primary_state;

SELECT
    b.code AS board,
    p.title,
    p.primary_state,
    p.support_count,
    p.not_a_fit_count,
    p.unclear_count,
    p.unsafe_count,
    p.merge_count,
    p.high_moderation_watch_started_at
FROM proposals p
JOIN boards b ON b.id = p.board_id
WHERE b.code IN ('issue', 'solution')
ORDER BY b.code, p.primary_state, p.title;

CREATE TEMP TABLE _ck_trust_summary (
    trust_tables_migrated BOOLEAN NOT NULL,
    open_trust_flags INTEGER,
    dev_user_activity_events INTEGER,
    author_merge_notifications INTEGER,
    moderator_merge_notifications INTEGER
);

DO $$
BEGIN
    IF to_regclass('public.anti_abuse_flags') IS NULL THEN
        INSERT INTO _ck_trust_summary (trust_tables_migrated)
        VALUES (FALSE);
    ELSE
        EXECUTE $trust_summary$
            INSERT INTO _ck_trust_summary (
                trust_tables_migrated,
                open_trust_flags,
                dev_user_activity_events,
                author_merge_notifications,
                moderator_merge_notifications
            )
            SELECT
                TRUE,
                (SELECT COUNT(*)::int FROM anti_abuse_flags WHERE status = 'open'),
                (
                    SELECT COUNT(*)::int
                    FROM user_activity_events e
                    JOIN users u ON u.id = e.user_id
                    WHERE u.email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
                ),
                (
                    SELECT COUNT(*)::int
                    FROM notification_events n
                    WHERE n.notification_type = 'merge_watch_author'
                ),
                (
                    SELECT COUNT(*)::int
                    FROM notification_events n
                    JOIN users u ON u.id = n.recipient_user_id
                    WHERE n.notification_type = 'merge_watch_moderator'
                      AND u.email = 'moderator@example.com'
                )
        $trust_summary$;
    END IF;
END
$$;

SELECT *
FROM _ck_trust_summary;

SELECT
    er.title,
    pi.title AS issue_title,
    er.status,
    jsonb_array_length(er.execution_tracking_entries) AS resource_count,
    jsonb_array_length(er.completion_criteria) AS criteria_count
FROM execution_records er
JOIN proposals pi ON pi.id = er.parent_issue_proposal_id
ORDER BY er.created_at DESC;
'@
    Invoke-CkSql $sql
}

function New-CkImplementationScenario {
    $implementation = Get-CkSeedImplementationSql
    $sql = @"
BEGIN;

$implementation

COMMIT;
"@
    Invoke-CkSql $sql
    Write-Host "Implementation scenario ready. Open Implementations to review seeded resource tracking."
}

function New-CkModerationHoldScenario {
    $refresh = Get-CkRefreshCountsSql
    $moderationHold = Get-CkSeedModerationHoldScenarioSql
    $sql = @"
BEGIN;

$refresh

$moderationHold

$refresh

COMMIT;
"@
    Invoke-CkSql $sql
    Write-Host "Moderation hold scenario ready. Mobile water testing is action-ready; regional water lab is still in the 24-hour hold."
}

function Reset-CkUserParticipation {
    param(
        [string]$Email = "test2@example.com"
    )

    $safeEmail = $Email.Replace("'", "''")
    $refresh = Get-CkRefreshCountsSql
    $sql = @"
BEGIN;

DELETE FROM review_actions
WHERE user_id = (SELECT id FROM users WHERE email = '$safeEmail');

DELETE FROM review_unlocks
WHERE user_id = (SELECT id FROM users WHERE email = '$safeEmail');

DELETE FROM proposal_merge_votes
WHERE user_id = (SELECT id FROM users WHERE email = '$safeEmail');

DELETE FROM proposal_sentiment_votes
WHERE user_id = (SELECT id FROM users WHERE email = '$safeEmail');

DELETE FROM proposal_comment_votes
WHERE user_id = (SELECT id FROM users WHERE email = '$safeEmail');

DELETE FROM proposal_comments
WHERE author_user_id = (SELECT id FROM users WHERE email = '$safeEmail');

DELETE FROM anti_abuse_flags
WHERE user_id = (SELECT id FROM users WHERE email = '$safeEmail');

DELETE FROM user_activity_events
WHERE user_id = (SELECT id FROM users WHERE email = '$safeEmail')
  AND event_type IN ('review_action', 'sentiment_vote', 'merge_vote', 'proposal_created', 'proposal_comment_created', 'proposal_comment_vote');

$refresh

COMMIT;
"@
    Invoke-CkSql $sql
    Write-Host "Cleared reviews, votes, and comments for $Email."
}

function Reset-CkUserLoginState {
    param(
        [string]$Email = "test2@example.com"
    )

    $safeEmail = $Email.Replace("'", "''")
    $sql = @"
BEGIN;

DELETE FROM sessions
WHERE user_id = (SELECT id FROM users WHERE email = '$safeEmail');

UPDATE users
SET last_login_at = NULL,
    email_verified = TRUE
WHERE email = '$safeEmail';

DELETE FROM email_verification_tokens
WHERE user_id = (SELECT id FROM users WHERE email = '$safeEmail');

DELETE FROM password_reset_tokens
WHERE user_id = (SELECT id FROM users WHERE email = '$safeEmail');

DELETE FROM user_activity_events
WHERE user_id = (SELECT id FROM users WHERE email = '$safeEmail')
  AND event_type = 'login';

COMMIT;
"@
    Invoke-CkSql $sql
    Write-Host "Cleared sessions/tokens and last_login_at for $Email."
}

function New-CkVerificationScenario {
    param(
        [string]$Email = "test2@example.com",
        [string]$Token = "dev-verify-test2"
    )

    $safeEmail = $Email.Replace("'", "''")
    $safeToken = $Token.Replace("'", "''")
    $sql = @"
BEGIN;

UPDATE users
SET email_verified = FALSE,
    last_login_at = NULL
WHERE email = '$safeEmail';

DELETE FROM sessions
WHERE user_id = (SELECT id FROM users WHERE email = '$safeEmail');

DELETE FROM email_verification_tokens
WHERE user_id = (SELECT id FROM users WHERE email = '$safeEmail');

INSERT INTO email_verification_tokens (user_id, token, expires_at)
SELECT id, encode(digest('$safeToken', 'sha256'), 'hex'), NOW() + INTERVAL '24 hours'
FROM users
WHERE email = '$safeEmail';

COMMIT;
"@
    Invoke-CkSql $sql
    Write-Host "Verification scenario ready for $Email. Plain token: $Token"
}

function New-CkPasswordResetScenario {
    param(
        [string]$Email = "test2@example.com",
        [string]$Token = "dev-reset-test2"
    )

    $safeEmail = $Email.Replace("'", "''")
    $safeToken = $Token.Replace("'", "''")
    $sql = @"
BEGIN;

DELETE FROM password_reset_tokens
WHERE user_id = (SELECT id FROM users WHERE email = '$safeEmail');

INSERT INTO password_reset_tokens (user_id, token, expires_at)
SELECT id, encode(digest('$safeToken', 'sha256'), 'hex'), NOW() + INTERVAL '1 hour'
FROM users
WHERE email = '$safeEmail';

COMMIT;
"@
    Invoke-CkSql $sql
    Write-Host "Password reset scenario ready for $Email. Plain token: $Token"
}

function Set-CkCyclePhase {
    param(
        [ValidateSet("active", "closed")]
        [string]$Phase = "active"
    )

    if ($Phase -eq "active") {
        $dates = "starts_at = date_trunc('month', NOW()), submission_ends_at = date_trunc('month', NOW()) + INTERVAL '1 month', voting_ends_at = date_trunc('month', NOW()) + INTERVAL '1 month'"
    } else {
        $dates = "starts_at = date_trunc('month', NOW()) - INTERVAL '1 month', submission_ends_at = date_trunc('month', NOW()), voting_ends_at = date_trunc('month', NOW())"
    }

    $sql = @"
UPDATE cycles
SET $dates
WHERE is_active = TRUE;

SELECT cycle_number, starts_at, submission_ends_at, voting_ends_at, is_active
FROM cycles
WHERE is_active = TRUE;
"@
    Invoke-CkSql $sql
}

function Reset-CkModerationFacet {
    $refresh = Get-CkRefreshCountsSql
    $sql = @"
BEGIN;

DELETE FROM appeals;
DELETE FROM reconsideration_windows;
DELETE FROM proposal_watch_flags;
DELETE FROM moderator_actions;

UPDATE proposals
SET primary_state = CASE
        WHEN title = 'DEMO PRIOR WINNER: Clean water as current solution target' THEN 'archived'
        ELSE 'active'
    END,
    archived_reason = CASE
        WHEN title = 'DEMO PRIOR WINNER: Clean water as current solution target' THEN 'cycle_closed'
        ELSE NULL
    END,
    moderation_note = NULL,
    merged_into_proposal_id = NULL
WHERE title LIKE 'DEMO %';

$refresh

COMMIT;
"@
    Invoke-CkSql $sql
    Write-Host "Cleared moderation actions, appeals, reconsiderations, and frozen flags."
}

function Reset-CkTrustFacet {
    $notifications = Get-CkSeedMergeNotificationSql
    $sql = @"
BEGIN;

DELETE FROM anti_abuse_flags;
DELETE FROM user_activity_events;
DELETE FROM notification_events;

$notifications

COMMIT;
"@
    Invoke-CkSql $sql
    Write-Host "Cleared trust-review flags and activity signals; reseeded demo merge notifications."
}

function New-CkTrustReviewScenario {
    $sql = @'
BEGIN;

WITH ids AS (
    SELECT
        (SELECT id FROM users WHERE email = 'test2@example.com') AS user_id,
        (SELECT id FROM proposals WHERE title = 'DEMO ISSUE: Duplicate clean water access framing') AS proposal_id,
        (SELECT id FROM proposals WHERE title = 'DEMO ISSUE: Clean water access gap') AS related_proposal_id
),
cleared AS (
    DELETE FROM anti_abuse_flags
    WHERE user_id = (SELECT user_id FROM ids)
      AND flag_code = 'merge_signal_cluster'
      AND proposal_id = (SELECT proposal_id FROM ids)
      AND related_proposal_id = (SELECT related_proposal_id FROM ids)
)
INSERT INTO anti_abuse_flags (
    user_id,
    flag_code,
    severity,
    proposal_id,
    related_proposal_id,
    client_ip_hint,
    user_agent_hash,
    details
)
SELECT
    user_id,
    'merge_signal_cluster',
    'high',
    proposal_id,
    related_proposal_id,
    'dev-scenario:shared-network',
    'dev-scenario-browser',
    jsonb_build_object(
        'summary', 'Seeded trust-review scenario for moderator workflow testing.',
        'distinct_user_count', 3,
        'window_hours', 24,
        'seeded', TRUE
    )
FROM ids
WHERE user_id IS NOT NULL
  AND proposal_id IS NOT NULL
  AND related_proposal_id IS NOT NULL;

INSERT INTO user_activity_events (
    user_id,
    event_type,
    proposal_id,
    related_proposal_id,
    client_ip_hint,
    user_agent_hash,
    metadata
)
SELECT
    user_id,
    'merge_vote',
    proposal_id,
    related_proposal_id,
    'dev-scenario:shared-network',
    'dev-scenario-browser',
    jsonb_build_object('seeded_trust_review_scenario', TRUE)
FROM ids
WHERE user_id IS NOT NULL
  AND proposal_id IS NOT NULL
  AND related_proposal_id IS NOT NULL;

COMMIT;
'@
    Invoke-CkSql $sql
    Write-Host "Trust Review scenario ready. Log in as moderator@example.com and open Trust Review."
}

function New-CkAppealScenario {
    $sql = @'
BEGIN;

WITH ids AS (
    SELECT
        (SELECT id FROM boards WHERE code = 'issue') AS board_id,
        (SELECT id FROM cycles WHERE is_active = TRUE ORDER BY created_at DESC LIMIT 1) AS cycle_id,
        (SELECT locale_id FROM cycles WHERE is_active = TRUE ORDER BY created_at DESC LIMIT 1) AS locale_id,
        (SELECT id FROM users WHERE email = 'test2@example.com') AS author_id,
        (SELECT id FROM users WHERE email = 'moderator@example.com') AS moderator_id
),
upserted AS (
    INSERT INTO proposals (
        board_id,
        cycle_id,
        locale_id,
        author_user_id,
        title,
        problem_description,
        affected_scope,
        why_it_matters,
        primary_state,
        archived_reason,
        moderation_note
    )
    SELECT
        board_id,
        cycle_id,
        locale_id,
        author_id,
        'SCENARIO APPEAL: Archived test2 proposal',
        'A test proposal owned by test2 so the author appeal flow can be exercised.',
        'World',
        'This record exists only for local testing.',
        'archived',
        'manual_archive',
        'Seeded appeal scenario.'
    FROM ids
    WHERE NOT EXISTS (
        SELECT 1
        FROM proposals
        WHERE title = 'SCENARIO APPEAL: Archived test2 proposal'
    )
    RETURNING id
),
proposal AS (
    SELECT id FROM upserted
    UNION ALL
    SELECT id FROM proposals WHERE title = 'SCENARIO APPEAL: Archived test2 proposal'
    LIMIT 1
)
UPDATE proposals p
SET primary_state = 'archived',
    archived_reason = 'manual_archive',
    moderation_note = 'Seeded appeal scenario.',
    merged_into_proposal_id = NULL
FROM proposal
WHERE p.id = proposal.id;

DELETE FROM appeals
WHERE proposal_id = (SELECT id FROM proposals WHERE title = 'SCENARIO APPEAL: Archived test2 proposal');

INSERT INTO moderator_actions (proposal_id, moderator_user_id, action_type, action_reason, public_note, internal_note, state_snapshot)
SELECT
    p.id,
    u.id,
    'archive',
    'manual_archive',
    'Seeded appeal scenario.',
    'Seeded by New-CkAppealScenario.',
    jsonb_build_object('seeded', true)
FROM proposals p
CROSS JOIN users u
WHERE p.title = 'SCENARIO APPEAL: Archived test2 proposal'
  AND u.email = 'moderator@example.com';

COMMIT;
'@
    Invoke-CkSql $sql
    Write-Host "Appeal scenario ready. Log in as test2@example.com and open the archived proposal."
}

function New-CkReconsiderationScenario {
    $sql = @'
BEGIN;

WITH ids AS (
    SELECT
        (SELECT id FROM boards WHERE code = 'issue') AS board_id,
        (SELECT id FROM cycles WHERE is_active = TRUE ORDER BY created_at DESC LIMIT 1) AS cycle_id,
        (SELECT locale_id FROM cycles WHERE is_active = TRUE ORDER BY created_at DESC LIMIT 1) AS locale_id,
        (SELECT id FROM users WHERE email = 'user@example.com') AS author_id,
        (SELECT id FROM users WHERE email = 'moderator@example.com') AS moderator_id
),
upserted AS (
    INSERT INTO proposals (
        board_id,
        cycle_id,
        locale_id,
        author_user_id,
        title,
        problem_description,
        affected_scope,
        why_it_matters,
        primary_state,
        archived_reason,
        moderation_note
    )
    SELECT
        board_id,
        cycle_id,
        locale_id,
        author_id,
        'SCENARIO RECONSIDERATION: Archived public proposal',
        'A test proposal for moderator reconsideration windows.',
        'World',
        'This record exists only for local testing.',
        'archived',
        'manual_archive',
        'Seeded reconsideration scenario.'
    FROM ids
    WHERE NOT EXISTS (
        SELECT 1
        FROM proposals
        WHERE title = 'SCENARIO RECONSIDERATION: Archived public proposal'
    )
    RETURNING id
),
proposal AS (
    SELECT id FROM upserted
    UNION ALL
    SELECT id FROM proposals WHERE title = 'SCENARIO RECONSIDERATION: Archived public proposal'
    LIMIT 1
)
UPDATE proposals p
SET primary_state = 'archived',
    archived_reason = 'manual_archive',
    moderation_note = 'Seeded reconsideration scenario.',
    merged_into_proposal_id = NULL
FROM proposal
WHERE p.id = proposal.id;

DELETE FROM reconsideration_windows
WHERE proposal_id = (SELECT id FROM proposals WHERE title = 'SCENARIO RECONSIDERATION: Archived public proposal');

INSERT INTO moderator_actions (proposal_id, moderator_user_id, action_type, action_reason, public_note, internal_note, state_snapshot)
SELECT
    p.id,
    u.id,
    'archive',
    'manual_archive',
    'Seeded reconsideration scenario.',
    'Seeded by New-CkReconsiderationScenario.',
    jsonb_build_object('seeded', true)
FROM proposals p
CROSS JOIN users u
WHERE p.title = 'SCENARIO RECONSIDERATION: Archived public proposal'
  AND u.email = 'moderator@example.com';

COMMIT;
'@
    Invoke-CkSql $sql
    Write-Host "Reconsideration scenario ready. Log in as moderator@example.com and open the archived proposal."
}

function Reset-CkMergeFacet {
    $refresh = Get-CkRefreshCountsSql
    $sql = @"
BEGIN;

DELETE FROM proposal_merge_vote_reconciliations;
DELETE FROM merge_distinction_notes;
DELETE FROM proposal_merge_votes;
DELETE FROM proposal_merge_relationships;
DELETE FROM notification_events;
DELETE FROM anti_abuse_flags WHERE flag_code = 'merge_signal_cluster';
DELETE FROM user_activity_events WHERE event_type = 'merge_vote';

UPDATE proposals
SET primary_state = CASE
        WHEN title = 'DEMO PRIOR WINNER: Clean water as current solution target' THEN 'archived'
        ELSE 'active'
    END,
    archived_reason = CASE
        WHEN title = 'DEMO PRIOR WINNER: Clean water as current solution target' THEN 'cycle_closed'
        ELSE NULL
    END,
    merged_into_proposal_id = NULL
WHERE title LIKE 'DEMO %';

$refresh

COMMIT;
"@
    Invoke-CkSql $sql
    Invoke-CkDemoSeeder

    Write-Host "Merge demo votes and relationships reseeded."
}

function Reset-CkExecutionFacet {
    $sql = @'
BEGIN;

DELETE FROM cycle_results WHERE board_code = 'solution';
DELETE FROM execution_records;

UPDATE proposals
SET primary_state = 'active',
    archived_reason = NULL,
    moderation_note = NULL,
    merged_into_proposal_id = NULL
WHERE title LIKE 'DEMO SOLUTION:%';

COMMIT;
'@
    Invoke-CkSql $sql
    Write-Host "Execution records cleared; demo solution proposals are active again."
}

function Test-CkSeedRequirements {
    $sql = @'
CREATE TEMP TABLE _ck_sanity_assertions (
    sort_order INTEGER NOT NULL,
    requirement TEXT NOT NULL,
    status TEXT NOT NULL,
    detail TEXT NOT NULL
);

WITH
active_cycle AS (
    SELECT c.*
    FROM cycles c
    JOIN locales l ON l.id = c.locale_id
    WHERE l.slug = 'world'
      AND c.is_active = TRUE
    ORDER BY c.created_at DESC
    LIMIT 1
),
solution_target AS (
    SELECT cr.winning_proposal_id
    FROM cycle_results cr
    JOIN cycles c ON c.id = cr.cycle_id
    JOIN active_cycle ac ON ac.locale_id = cr.locale_id
    WHERE cr.board_code = 'issue'
      AND cr.result_status = 'resolved'
      AND cr.published_at IS NOT NULL
      AND cr.winning_proposal_id IS NOT NULL
      AND c.cycle_number < ac.cycle_number
    ORDER BY c.cycle_number DESC, cr.published_at DESC
    LIMIT 1
),
actual_sentiment_counts AS (
    SELECT
        proposal_id,
        COUNT(*) FILTER (WHERE vote_value = 'support')::int AS support_count,
        COUNT(*) FILTER (WHERE vote_value = 'not_a_fit')::int AS not_a_fit_count,
        COUNT(*) FILTER (WHERE vote_value = 'unclear')::int AS unclear_count,
        COUNT(*) FILTER (WHERE vote_value = 'unsafe')::int AS unsafe_count
    FROM proposal_sentiment_votes
    GROUP BY proposal_id
),
actual_merge_counts AS (
    SELECT
        mv.proposal_id,
        COUNT(*)::int AS merge_count
    FROM proposal_merge_votes mv
    JOIN proposals target
      ON target.id = mv.target_proposal_id
     AND target.primary_state = 'active'
    JOIN proposal_merge_relationships r
      ON r.source_proposal_id = mv.proposal_id
     AND r.target_proposal_id = mv.target_proposal_id
     AND r.status = 'active'
    GROUP BY mv.proposal_id
),
counter_mismatches AS (
    SELECT p.id, p.title
    FROM proposals p
    WHERE p.title LIKE 'DEMO %'
      AND (
        p.support_count <> COALESCE((SELECT support_count FROM actual_sentiment_counts a WHERE a.proposal_id = p.id), 0)
        OR p.not_a_fit_count <> COALESCE((SELECT not_a_fit_count FROM actual_sentiment_counts a WHERE a.proposal_id = p.id), 0)
        OR p.unclear_count <> COALESCE((SELECT unclear_count FROM actual_sentiment_counts a WHERE a.proposal_id = p.id), 0)
        OR p.unsafe_count <> COALESCE((SELECT unsafe_count FROM actual_sentiment_counts a WHERE a.proposal_id = p.id), 0)
        OR p.merge_count <> COALESCE((SELECT merge_count FROM actual_merge_counts a WHERE a.proposal_id = p.id), 0)
      )
),
bad_issue_payloads AS (
    SELECT p.id, p.title
    FROM proposals p
    JOIN boards b ON b.id = p.board_id
    WHERE b.code = 'issue'
      AND p.title LIKE 'DEMO %'
      AND (
        NULLIF(BTRIM(p.title), '') IS NULL
        OR NULLIF(BTRIM(COALESCE(p.problem_description, '')), '') IS NULL
        OR NULLIF(BTRIM(COALESCE(p.affected_scope, '')), '') IS NULL
        OR NULLIF(BTRIM(COALESCE(p.why_it_matters, '')), '') IS NULL
        OR p.action_description IS NOT NULL
      )
),
bad_solution_payloads AS (
    SELECT p.id, p.title
    FROM proposals p
    JOIN boards b ON b.id = p.board_id
    WHERE b.code = 'solution'
      AND p.title LIKE 'DEMO SOLUTION:%'
      AND (
        p.parent_issue_proposal_id IS NULL
        OR p.parent_issue_proposal_id <> (SELECT winning_proposal_id FROM solution_target)
        OR NULLIF(BTRIM(p.title), '') IS NULL
        OR NULLIF(BTRIM(COALESCE(p.action_description, '')), '') IS NULL
        OR NULLIF(BTRIM(COALESCE(p.why_it_matters, '')), '') IS NULL
        OR jsonb_typeof(p.required_resource_categories) <> 'array'
        OR jsonb_array_length(CASE WHEN jsonb_typeof(p.required_resource_categories) = 'array' THEN p.required_resource_categories ELSE '[]'::jsonb END) = 0
        OR jsonb_typeof(p.completion_criteria) <> 'array'
        OR jsonb_array_length(CASE WHEN jsonb_typeof(p.completion_criteria) = 'array' THEN p.completion_criteria ELSE '[]'::jsonb END) = 0
        OR jsonb_typeof(p.execution_tracking_entries) <> 'array'
        OR jsonb_array_length(CASE WHEN jsonb_typeof(p.execution_tracking_entries) = 'array' THEN p.execution_tracking_entries ELSE '[]'::jsonb END) = 0
        OR EXISTS (
            SELECT 1
            FROM jsonb_array_elements_text(
                CASE WHEN jsonb_typeof(p.required_resource_categories) = 'array' THEN p.required_resource_categories ELSE '[]'::jsonb END
            ) AS category(value)
            WHERE LOWER(BTRIM(category.value)) NOT IN (
                'money',
                'labor',
                'manpower',
                'labor / manpower',
                'skills',
                'trades',
                'skills / trades',
                'materials',
                'equipment',
                'logistics',
                'transport',
                'logistics / transport',
                'organizational support',
                'other'
            )
        )
    )
),
bad_merge_votes AS (
    SELECT mv.id
    FROM proposal_merge_votes mv
    JOIN proposals source ON source.id = mv.proposal_id
    JOIN boards source_board ON source_board.id = source.board_id
    LEFT JOIN proposals target ON target.id = mv.target_proposal_id
    LEFT JOIN boards target_board ON target_board.id = target.board_id
    WHERE mv.target_proposal_id IS NULL
       OR mv.target_proposal_id = mv.proposal_id
       OR target.id IS NULL
       OR source.primary_state <> 'active'
       OR target.primary_state <> 'active'
       OR source_board.code <> target_board.code
       OR source.cycle_id <> target.cycle_id
       OR source.locale_id <> target.locale_id
       OR NOT EXISTS (
            SELECT 1
            FROM proposal_merge_relationships r
            WHERE r.source_proposal_id = mv.proposal_id
              AND r.target_proposal_id = mv.target_proposal_id
              AND r.status = 'active'
       )
),
high_merge_pairs AS (
    SELECT
        r.source_proposal_id,
        r.target_proposal_id,
        source.title AS source_title,
        target.title AS target_title,
        source.primary_state AS source_state,
        source.merged_into_proposal_id,
        (source.support_count + source.not_a_fit_count + source.unclear_count + source.unsafe_count + source.merge_count) AS source_total,
        COUNT(mv.id)::int AS directed_merge_count
    FROM proposal_merge_relationships r
    JOIN proposals source ON source.id = r.source_proposal_id
    JOIN proposals target ON target.id = r.target_proposal_id
    LEFT JOIN proposal_merge_votes mv
      ON mv.proposal_id = r.source_proposal_id
     AND mv.target_proposal_id = r.target_proposal_id
    WHERE r.status = 'active'
    GROUP BY
        r.source_proposal_id,
        r.target_proposal_id,
        source.title,
        target.title,
        source.primary_state,
        source.merged_into_proposal_id,
        source.support_count,
        source.not_a_fit_count,
        source.unclear_count,
        source.unsafe_count,
        source.merge_count
    HAVING
        (source.support_count + source.not_a_fit_count + source.unclear_count + source.unsafe_count + source.merge_count) >= 20
        AND COUNT(mv.id)::numeric
            / NULLIF((source.support_count + source.not_a_fit_count + source.unclear_count + source.unsafe_count + source.merge_count), 0)::numeric >= 0.35
),
high_moderation_watch AS (
    SELECT p.id, p.title
    FROM proposals p
    WHERE p.primary_state = 'active'
      AND (
        p.unsafe_count >= 8
        OR (
            (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) > 0
            AND p.unsafe_count::numeric
                / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
        )
      )
),
dev_user_participation AS (
    SELECT
        (
            SELECT COUNT(*)
            FROM review_actions ra
            JOIN users u ON u.id = ra.user_id
            WHERE u.email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
        )
        + (
            SELECT COUNT(*)
            FROM proposal_sentiment_votes sv
            JOIN users u ON u.id = sv.user_id
            WHERE u.email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
        )
        + (
            SELECT COUNT(*)
            FROM proposal_merge_votes mv
            JOIN users u ON u.id = mv.user_id
            WHERE u.email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
        ) AS operation_count
),
dev_auth_state AS (
    SELECT
        (
            SELECT COUNT(*)
            FROM sessions s
            JOIN users u ON u.id = s.user_id
            WHERE u.email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
        )
        + (
            SELECT COUNT(*)
            FROM email_verification_tokens evt
            JOIN users u ON u.id = evt.user_id
            WHERE u.email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
        )
        + (
            SELECT COUNT(*)
            FROM password_reset_tokens prt
            JOIN users u ON u.id = prt.user_id
            WHERE u.email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
        ) AS operation_count
),
trust_baseline_state AS (
    SELECT
        (SELECT COUNT(*) FROM anti_abuse_flags WHERE status = 'open') AS open_trust_flags,
        (
            SELECT COUNT(*)
            FROM user_activity_events e
            JOIN users u ON u.id = e.user_id
            WHERE u.email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
        ) AS dev_activity_events,
        (
            SELECT COUNT(*)
            FROM notification_events n
            JOIN proposals p ON p.id = n.proposal_id
            WHERE n.notification_type = 'merge_watch_author'
              AND p.title IN (
                'DEMO ISSUE: Duplicate clean water access framing',
                'DEMO SOLUTION: Mobile water testing training corps'
              )
        ) AS demo_author_merge_notifications,
        (
            SELECT COUNT(*)
            FROM notification_events n
            JOIN proposals p ON p.id = n.proposal_id
            JOIN users u ON u.id = n.recipient_user_id
            WHERE n.notification_type = 'merge_watch_moderator'
              AND u.email = 'moderator@example.com'
              AND p.title IN (
                'DEMO ISSUE: Duplicate clean water access framing',
                'DEMO SOLUTION: Mobile water testing training corps'
              )
        ) AS demo_moderator_merge_notifications
),
eligible_reviews AS (
    SELECT
        u.email,
        b.code AS board_code,
        COUNT(*)::int AS eligible_count
    FROM users u
    CROSS JOIN active_cycle ac
    JOIN proposals p ON p.cycle_id = ac.id
    JOIN boards b ON b.id = p.board_id
    LEFT JOIN review_actions ra
      ON ra.proposal_id = p.id
     AND ra.user_id = u.id
     AND ra.cycle_id = p.cycle_id
    LEFT JOIN proposal_sentiment_votes sv
      ON sv.proposal_id = p.id
     AND sv.user_id = u.id
    LEFT JOIN proposal_merge_votes mv
      ON mv.proposal_id = p.id
     AND mv.user_id = u.id
    WHERE u.email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
      AND b.code IN ('issue', 'solution')
      AND p.primary_state = 'active'
      AND p.author_user_id <> u.id
      AND ra.id IS NULL
      AND sv.id IS NULL
      AND mv.id IS NULL
      AND (p.not_a_fit_count + p.unclear_count + p.unsafe_count) <= 8 * GREATEST(p.support_count, 1)
      AND p.unsafe_count < 8
      AND (
        (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) = 0
        OR p.unsafe_count::numeric
            / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric < 0.50
      )
      AND NOT EXISTS (
        SELECT 1
        FROM proposal_watch_flags wf
        WHERE wf.proposal_id = p.id
          AND wf.flag_code = 'frozen_for_review'
          AND wf.cleared_at IS NULL
      )
    GROUP BY u.email, b.code
)
INSERT INTO _ck_sanity_assertions (sort_order, requirement, status, detail)
SELECT
    10,
    'Configured locale is active and singular',
    CASE WHEN (SELECT COUNT(*) FROM locales WHERE slug = 'world' AND is_active = TRUE) = 1 THEN 'PASS' ELSE 'FAIL' END,
    'active_configured_locale_count=' || (SELECT COUNT(*) FROM locales WHERE slug = 'world' AND is_active = TRUE)
UNION ALL
SELECT
    20,
    'Required boards are active',
    CASE WHEN (SELECT COUNT(*) FROM boards WHERE code IN ('issue', 'solution', 'archive') AND is_active = TRUE) = 3 THEN 'PASS' ELSE 'FAIL' END,
    'active_required_board_count=' || (SELECT COUNT(*) FROM boards WHERE code IN ('issue', 'solution', 'archive') AND is_active = TRUE)
UNION ALL
SELECT
    30,
    'Exactly one active configured-locale cycle exists',
    CASE WHEN (SELECT COUNT(*) FROM cycles c JOIN locales l ON l.id = c.locale_id WHERE l.slug = 'world' AND c.is_active = TRUE) = 1 THEN 'PASS' ELSE 'FAIL' END,
    'active_configured_locale_cycle_count=' || (SELECT COUNT(*) FROM cycles c JOIN locales l ON l.id = c.locale_id WHERE l.slug = 'world' AND c.is_active = TRUE)
UNION ALL
SELECT
    40,
    'Dev accounts are verified and role-correct',
    CASE WHEN (
        SELECT COUNT(*)
        FROM users
        WHERE (email = 'user@example.com' AND role_code = 'registered_user' AND email_verified = TRUE)
           OR (email = 'test2@example.com' AND role_code = 'registered_user' AND email_verified = TRUE)
           OR (email = 'moderator@example.com' AND role_code = 'moderator' AND email_verified = TRUE)
    ) = 3 THEN 'PASS' ELSE 'FAIL' END,
    'matching_dev_accounts=' || (
        SELECT COUNT(*)
        FROM users
        WHERE (email = 'user@example.com' AND role_code = 'registered_user' AND email_verified = TRUE)
           OR (email = 'test2@example.com' AND role_code = 'registered_user' AND email_verified = TRUE)
           OR (email = 'moderator@example.com' AND role_code = 'moderator' AND email_verified = TRUE)
    )
UNION ALL
SELECT
    50,
    'Dev account operational state is clean',
    CASE WHEN (SELECT operation_count FROM dev_auth_state) = 0 THEN 'PASS' ELSE 'FAIL' END,
    'sessions_and_tokens=' || (SELECT operation_count FROM dev_auth_state)
UNION ALL
SELECT
    60,
    'Dev users have no baseline votes or reviews',
    CASE WHEN (SELECT operation_count FROM dev_user_participation) = 0 THEN 'PASS' ELSE 'FAIL' END,
    'review_vote_merge_records=' || (SELECT operation_count FROM dev_user_participation)
UNION ALL
SELECT
    70,
    'Baseline demo proposal counts match expected boards and states',
    CASE WHEN
        (SELECT COUNT(*) FROM proposals p JOIN boards b ON b.id = p.board_id WHERE b.code = 'issue' AND p.primary_state = 'active' AND p.title LIKE 'DEMO ISSUE:%') = 4
        AND (SELECT COUNT(*) FROM proposals p JOIN boards b ON b.id = p.board_id WHERE b.code = 'issue' AND p.primary_state = 'archived' AND p.title = 'DEMO PRIOR WINNER: Clean water as current solution target') = 1
        AND (SELECT COUNT(*) FROM proposals p JOIN boards b ON b.id = p.board_id WHERE b.code = 'solution' AND p.primary_state = 'active' AND p.title LIKE 'DEMO SOLUTION:%') = 4
    THEN 'PASS' ELSE 'FAIL' END,
    'issue_active=' || (SELECT COUNT(*) FROM proposals p JOIN boards b ON b.id = p.board_id WHERE b.code = 'issue' AND p.primary_state = 'active' AND p.title LIKE 'DEMO ISSUE:%')
        || ', issue_archived_prior_winner=' || (SELECT COUNT(*) FROM proposals p JOIN boards b ON b.id = p.board_id WHERE b.code = 'issue' AND p.primary_state = 'archived' AND p.title = 'DEMO PRIOR WINNER: Clean water as current solution target')
        || ', solution_active=' || (SELECT COUNT(*) FROM proposals p JOIN boards b ON b.id = p.board_id WHERE b.code = 'solution' AND p.primary_state = 'active' AND p.title LIKE 'DEMO SOLUTION:%')
UNION ALL
SELECT
    80,
    'Issue proposals carry required issue fields only',
    CASE WHEN (SELECT COUNT(*) FROM bad_issue_payloads) = 0 THEN 'PASS' ELSE 'FAIL' END,
    'bad_issue_payloads=' || (SELECT COUNT(*) FROM bad_issue_payloads)
UNION ALL
SELECT
    90,
    'Solution board has a prior-cycle target issue',
    CASE WHEN (SELECT COUNT(*) FROM solution_target) = 1 THEN 'PASS' ELSE 'FAIL' END,
    'solution_target_count=' || (SELECT COUNT(*) FROM solution_target)
UNION ALL
SELECT
    100,
    'Solution proposals target the current winning issue and include execution structure',
    CASE WHEN (SELECT COUNT(*) FROM bad_solution_payloads) = 0 THEN 'PASS' ELSE 'FAIL' END,
    'bad_solution_payloads=' || (SELECT COUNT(*) FROM bad_solution_payloads)
UNION ALL
SELECT
    110,
    'Stored vote counters match vote tables',
    CASE WHEN (SELECT COUNT(*) FROM counter_mismatches) = 0 THEN 'PASS' ELSE 'FAIL' END,
    'counter_mismatches=' || (SELECT COUNT(*) FROM counter_mismatches)
UNION ALL
SELECT
    120,
    'Duplicate-link votes are targeted, active, same board/cycle/locale, and relationship-backed',
    CASE WHEN (SELECT COUNT(*) FROM bad_merge_votes) = 0 THEN 'PASS' ELSE 'FAIL' END,
    'bad_merge_votes=' || (SELECT COUNT(*) FROM bad_merge_votes)
UNION ALL
SELECT
    130,
    'Seed includes high duplicate-link review scenarios without auto-merging',
    CASE WHEN
        (SELECT COUNT(*) FROM high_merge_pairs) >= 2
        AND (SELECT COUNT(*) FROM high_merge_pairs WHERE source_state <> 'active' OR merged_into_proposal_id IS NOT NULL) = 0
    THEN 'PASS' ELSE 'FAIL' END,
    'high_merge_pairs=' || (SELECT COUNT(*) FROM high_merge_pairs)
        || ', auto_merged_high_pairs=' || (SELECT COUNT(*) FROM high_merge_pairs WHERE source_state <> 'active' OR merged_into_proposal_id IS NOT NULL)
UNION ALL
SELECT
    140,
    'Baseline has no high-moderation active proposals',
    CASE WHEN (SELECT COUNT(*) FROM high_moderation_watch) = 0 THEN 'PASS' ELSE 'FAIL' END,
    'high_moderation_watch_count=' || (SELECT COUNT(*) FROM high_moderation_watch)
UNION ALL
SELECT
    150,
    'Archived historical proposals do not carry active duplicate-link votes',
    CASE WHEN (
        SELECT COUNT(*)
        FROM proposal_merge_votes mv
        JOIN proposals p ON p.id = mv.proposal_id
        WHERE p.primary_state = 'archived'
    ) = 0 THEN 'PASS' ELSE 'FAIL' END,
    'archived_source_merge_votes=' || (
        SELECT COUNT(*)
        FROM proposal_merge_votes mv
        JOIN proposals p ON p.id = mv.proposal_id
        WHERE p.primary_state = 'archived'
    )
UNION ALL
SELECT
    160,
    'Scenario facets are clean after full reset',
    CASE WHEN
        NOT EXISTS (SELECT 1 FROM proposal_watch_flags WHERE cleared_at IS NULL)
        AND NOT EXISTS (SELECT 1 FROM appeals)
        AND NOT EXISTS (SELECT 1 FROM reconsideration_windows)
        AND NOT EXISTS (SELECT 1 FROM execution_records)
    THEN 'PASS' ELSE 'FAIL' END,
    'active_watch_flags=' || (SELECT COUNT(*) FROM proposal_watch_flags WHERE cleared_at IS NULL)
        || ', appeals=' || (SELECT COUNT(*) FROM appeals)
        || ', reconsiderations=' || (SELECT COUNT(*) FROM reconsideration_windows)
        || ', execution_records=' || (SELECT COUNT(*) FROM execution_records)
UNION ALL
SELECT
    165,
    'Trust-review baseline is clean and merge notifications are seeded',
    CASE WHEN
        (SELECT open_trust_flags FROM trust_baseline_state) = 0
        AND (SELECT dev_activity_events FROM trust_baseline_state) = 0
        AND (SELECT demo_author_merge_notifications FROM trust_baseline_state) >= 2
        AND (SELECT demo_moderator_merge_notifications FROM trust_baseline_state) >= 2
    THEN 'PASS' ELSE 'FAIL' END,
    'open_trust_flags=' || (SELECT open_trust_flags FROM trust_baseline_state)
        || ', dev_activity_events=' || (SELECT dev_activity_events FROM trust_baseline_state)
        || ', demo_author_merge_notifications=' || (SELECT demo_author_merge_notifications FROM trust_baseline_state)
        || ', demo_moderator_merge_notifications=' || (SELECT demo_moderator_merge_notifications FROM trust_baseline_state)
UNION ALL
SELECT
    170,
    'Review unlock baseline exposes four eligible issue and solution reviews per dev user',
    CASE WHEN (
        SELECT COUNT(*)
        FROM eligible_reviews
        WHERE email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
          AND board_code IN ('issue', 'solution')
          AND eligible_count = 4
    ) = 6 THEN 'PASS' ELSE 'FAIL' END,
    'matching_user_board_pairs=' || (
        SELECT COUNT(*)
        FROM eligible_reviews
        WHERE email IN ('user@example.com', 'test2@example.com', 'moderator@example.com')
          AND board_code IN ('issue', 'solution')
          AND eligible_count = 4
    );

SELECT status, requirement, detail
FROM _ck_sanity_assertions
ORDER BY sort_order;

DO $$
DECLARE
    fail_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO fail_count
    FROM _ck_sanity_assertions
    WHERE status <> 'PASS';

    IF fail_count > 0 THEN
        RAISE EXCEPTION 'CK seed sanity failed with % failing assertion(s).', fail_count;
    END IF;
END
$$;
'@

    Invoke-CkSql $sql
    Write-Host "Seed sanity checks passed."
}
