CREATE TABLE appeals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE RESTRICT,
    author_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    cycle_id UUID NOT NULL REFERENCES cycles(id) ON DELETE RESTRICT,
    appeal_reason TEXT NOT NULL,
    clarification_note TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    outcome TEXT,
    moderator_user_id UUID REFERENCES users(id) ON DELETE RESTRICT,
    moderator_note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,

    CONSTRAINT appeals_status_check
        CHECK (status IN ('pending', 'accepted', 'rejected')),

    CONSTRAINT appeals_outcome_check
        CHECK (outcome IS NULL OR outcome IN ('restore', 'uphold_archive')),

    CONSTRAINT appeals_unique_author_proposal_cycle
        UNIQUE (proposal_id, author_user_id, cycle_id)
);

CREATE INDEX idx_appeals_status_created_at
    ON appeals(status, created_at);

CREATE INDEX idx_appeals_proposal_id
    ON appeals(proposal_id, created_at DESC);

CREATE INDEX idx_appeals_author_user_id
    ON appeals(author_user_id, created_at DESC);
