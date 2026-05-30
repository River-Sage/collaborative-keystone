CREATE TABLE proposal_watch_flags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE CASCADE,
    flag_code TEXT NOT NULL,
    created_by_moderator_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    cleared_at TIMESTAMPTZ,
    cleared_by_moderator_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    clearance_reason TEXT,

    CONSTRAINT proposal_watch_flags_flag_code_check
        CHECK (flag_code IN ('frozen_for_review'))
);

CREATE UNIQUE INDEX idx_proposal_watch_flags_active_unique
    ON proposal_watch_flags(proposal_id, flag_code)
    WHERE cleared_at IS NULL;

CREATE INDEX idx_proposal_watch_flags_proposal_active
    ON proposal_watch_flags(proposal_id, flag_code, cleared_at);

INSERT INTO proposal_watch_flags (
    proposal_id,
    flag_code,
    reason
)
SELECT
    id,
    'frozen_for_review',
    'migrated_from_primary_state'
FROM proposals
WHERE primary_state = 'frozen'
ON CONFLICT DO NOTHING;

UPDATE proposals
SET primary_state = 'active'
WHERE primary_state = 'frozen';

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
            'removed'
        )
    );
