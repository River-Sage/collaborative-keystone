ALTER TABLE proposals
DROP CONSTRAINT proposals_primary_state_check;

ALTER TABLE proposals
ADD CONSTRAINT proposals_primary_state_check
    CHECK (
        primary_state IN (
            'draft',
            'active',
            'emerging',
            'ranked',
            'archived',
            'merged',
            'removed',
            'frozen'
        )
    );

CREATE TABLE reconsideration_windows (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE RESTRICT,
    cycle_id UUID NOT NULL REFERENCES cycles(id) ON DELETE RESTRICT,
    started_by_moderator_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    resolved_by_moderator_user_id UUID REFERENCES users(id) ON DELETE RESTRICT,
    start_reason TEXT NOT NULL,
    start_note TEXT,
    previous_archived_reason TEXT,
    previous_moderation_note TEXT,
    status TEXT NOT NULL DEFAULT 'open',
    outcome TEXT,
    resolution_note TEXT,
    starts_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ends_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '72 hours'),
    resolved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT reconsideration_windows_status_check
        CHECK (status IN ('open', 'resolved')),

    CONSTRAINT reconsideration_windows_outcome_check
        CHECK (
            outcome IS NULL
            OR outcome IN ('restore_active', 'return_archive', 'freeze')
        ),

    CONSTRAINT reconsideration_windows_unique_proposal_cycle
        UNIQUE (proposal_id, cycle_id)
);

CREATE INDEX idx_reconsideration_windows_status_ends_at
    ON reconsideration_windows(status, ends_at);

CREATE INDEX idx_reconsideration_windows_proposal_id
    ON reconsideration_windows(proposal_id, created_at DESC);

CREATE INDEX idx_reconsideration_windows_started_by
    ON reconsideration_windows(started_by_moderator_user_id, created_at DESC);
