-- Add migration script here
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE locales (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE cycles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    locale_id UUID NOT NULL REFERENCES locales(id) ON DELETE RESTRICT,
    cycle_number INTEGER NOT NULL,
    starts_at TIMESTAMPTZ NOT NULL,
    submission_ends_at TIMESTAMPTZ NOT NULL,
    voting_ends_at TIMESTAMPTZ NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (locale_id, cycle_number)
);

CREATE TABLE boards (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    role_code TEXT NOT NULL DEFAULT 'registered_user',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_login_at TIMESTAMPTZ
);

CREATE TABLE proposals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    board_id UUID NOT NULL REFERENCES boards(id) ON DELETE RESTRICT,
    cycle_id UUID NOT NULL REFERENCES cycles(id) ON DELETE RESTRICT,
    locale_id UUID NOT NULL REFERENCES locales(id) ON DELETE RESTRICT,
    author_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    parent_issue_proposal_id UUID REFERENCES proposals(id) ON DELETE RESTRICT,

    title TEXT NOT NULL,
    problem_description TEXT,
    affected_scope TEXT,
    why_it_matters TEXT,
    action_description TEXT,

    primary_state TEXT NOT NULL DEFAULT 'active',

    support_count INTEGER NOT NULL DEFAULT 0,
    not_a_fit_count INTEGER NOT NULL DEFAULT 0,
    unclear_count INTEGER NOT NULL DEFAULT 0,
    unsafe_count INTEGER NOT NULL DEFAULT 0,
    merge_count INTEGER NOT NULL DEFAULT 0,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT proposals_primary_state_check
        CHECK (primary_state IN ('draft', 'active', 'emerging', 'ranked', 'archived', 'merged', 'removed'))
);

CREATE INDEX idx_cycles_locale_active ON cycles(locale_id, is_active);
CREATE INDEX idx_proposals_board_cycle ON proposals(board_id, cycle_id);
CREATE INDEX idx_proposals_locale_cycle ON proposals(locale_id, cycle_id);
CREATE INDEX idx_proposals_author ON proposals(author_user_id);
CREATE INDEX idx_proposals_parent_issue ON proposals(parent_issue_proposal_id);

INSERT INTO locales (slug, name, is_active)
VALUES ('world', 'World', TRUE);

INSERT INTO boards (code, name, is_active)
VALUES
    ('issue', 'Issue Board', TRUE),
    ('solution', 'Solution Board', TRUE),
    ('archive', 'Archive Board', TRUE);