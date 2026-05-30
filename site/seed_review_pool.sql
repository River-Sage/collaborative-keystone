WITH active_cycle AS (
    SELECT c.id AS cycle_id, c.locale_id
    FROM cycles c
    JOIN locales l ON l.id = c.locale_id
    WHERE l.slug = 'world'
      AND c.is_active = TRUE
    ORDER BY c.created_at DESC
    LIMIT 1
),
issue_board AS (
    SELECT id AS board_id
    FROM boards
    WHERE code = 'issue'
    LIMIT 1
),
solution_board AS (
    SELECT id AS board_id
    FROM boards
    WHERE code = 'solution'
    LIMIT 1
),
author_user AS (
    SELECT id AS user_id
    FROM users
    WHERE email IN ('user@example.com', 'test2@example.com')
    ORDER BY CASE WHEN email = 'user@example.com' THEN 0 ELSE 1 END
    LIMIT 1
),
parent_issue AS (
    SELECT id AS proposal_id
    FROM proposals
    WHERE title = 'Global food waste and distribution inefficiency'
    LIMIT 1
)
INSERT INTO proposals (
    board_id,
    cycle_id,
    locale_id,
    author_user_id,
    parent_issue_proposal_id,
    title,
    problem_description,
    affected_scope,
    why_it_matters,
    action_description,
    required_resource_categories,
    completion_criteria,
    execution_tracking_entries,
    primary_state,
    support_count,
    not_a_fit_count,
    unclear_count,
    unsafe_count,
    merge_count
)
SELECT
    ib.board_id,
    ac.cycle_id,
    ac.locale_id,
    au.user_id,
    NULL::uuid,
    'Seed Issue A - merge heavy',
    'A seeded issue intended to test merge-heavy review-pool behavior.',
    'Global',
    'Used for review-bucket testing.',
    NULL::text,
    NULL::jsonb,
    NULL::jsonb,
    NULL::jsonb,
    'active',
    3, 1, 0, 0, 3
FROM active_cycle ac, issue_board ib, author_user au
UNION ALL
SELECT
    ib.board_id,
    ac.cycle_id,
    ac.locale_id,
    au.user_id,
    NULL::uuid,
    'Seed Issue B - merge heavy',
    'Another seeded issue intended to test merge-heavy review-pool behavior.',
    'Global',
    'Used for review-bucket testing.',
    NULL::text,
    NULL::jsonb,
    NULL::jsonb,
    NULL::jsonb,
    'active',
    2, 1, 1, 0, 2
FROM active_cycle ac, issue_board ib, author_user au
UNION ALL
SELECT
    ib.board_id,
    ac.cycle_id,
    ac.locale_id,
    au.user_id,
    NULL::uuid,
    'Seed Issue C - even low vote',
    'Seeded issue with very even low sentiment totals.',
    'Global',
    'Used for review-bucket testing.',
    NULL::text,
    NULL::jsonb,
    NULL::jsonb,
    NULL::jsonb,
    'active',
    2, 2, 0, 0, 0
FROM active_cycle ac, issue_board ib, author_user au
UNION ALL
SELECT
    ib.board_id,
    ac.cycle_id,
    ac.locale_id,
    au.user_id,
    NULL::uuid,
    'Seed Issue D - even low vote',
    'Second seeded issue with balanced low totals.',
    'Global',
    'Used for review-bucket testing.',
    NULL::text,
    NULL::jsonb,
    NULL::jsonb,
    NULL::jsonb,
    'active',
    1, 1, 0, 0, 0
FROM active_cycle ac, issue_board ib, author_user au
UNION ALL
SELECT
    ib.board_id,
    ac.cycle_id,
    ac.locale_id,
    au.user_id,
    NULL::uuid,
    'Seed Issue E - disliked not buried',
    'Seeded issue with some negative sentiment but still inside the ratio ceiling.',
    'Global',
    'Used for review-bucket testing.',
    NULL::text,
    NULL::jsonb,
    NULL::jsonb,
    NULL::jsonb,
    'active',
    1, 3, 1, 0, 0
FROM active_cycle ac, issue_board ib, author_user au
UNION ALL
SELECT
    ib.board_id,
    ac.cycle_id,
    ac.locale_id,
    au.user_id,
    NULL::uuid,
    'Seed Issue F - fallback',
    'Seeded issue with nearly no exposure.',
    'Global',
    'Used for review-bucket testing.',
    NULL::text,
    NULL::jsonb,
    NULL::jsonb,
    NULL::jsonb,
    'active',
    0, 0, 0, 0, 0
FROM active_cycle ac, issue_board ib, author_user au
UNION ALL
SELECT
    sb.board_id,
    ac.cycle_id,
    ac.locale_id,
    au.user_id,
    pi.proposal_id,
    'Seed Solution A - fallback',
    NULL::text,
    NULL::text,
    NULL::text,
    'Seeded solution with almost no interaction.',
    '["money"]'::jsonb,
    '[{"description":"Test completion"}]'::jsonb,
    '[{"resource_category":"money","target_needed":"1000 USD","current_acquired_amount":"0 USD","status_note":"Seed data"}]'::jsonb,
    'active',
    0, 0, 0, 0, 0
FROM active_cycle ac, solution_board sb, author_user au, parent_issue pi
UNION ALL
SELECT
    sb.board_id,
    ac.cycle_id,
    ac.locale_id,
    au.user_id,
    pi.proposal_id,
    'Seed Solution B - merge heavy',
    NULL::text,
    NULL::text,
    NULL::text,
    'Seeded solution intended to trigger merge-heavy behavior.',
    '["money","labor_manpower"]'::jsonb,
    '[{"description":"Test completion"}]'::jsonb,
    '[{"resource_category":"labor_manpower","target_needed":"5 volunteers","current_acquired_amount":"0","status_note":"Seed data"}]'::jsonb,
    'active',
    2, 1, 0, 0, 3
FROM active_cycle ac, solution_board sb, author_user au, parent_issue pi
UNION ALL
SELECT
    sb.board_id,
    ac.cycle_id,
    ac.locale_id,
    au.user_id,
    pi.proposal_id,
    'Seed Solution C - buried',
    NULL::text,
    NULL::text,
    NULL::text,
    'Seeded solution that should tend to rank lower.',
    '["money"]'::jsonb,
    '[{"description":"Test completion"}]'::jsonb,
    '[{"resource_category":"money","target_needed":"500 USD","current_acquired_amount":"0 USD","status_note":"Seed data"}]'::jsonb,
    'active',
    1, 6, 1, 1, 0
FROM active_cycle ac, solution_board sb, author_user au, parent_issue pi;
