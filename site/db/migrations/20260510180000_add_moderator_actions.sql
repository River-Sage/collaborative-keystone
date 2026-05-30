CREATE TABLE moderator_actions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    action_type TEXT NOT NULL,
    proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE RESTRICT,
    related_proposal_id UUID REFERENCES proposals(id) ON DELETE RESTRICT,
    moderator_user_id UUID REFERENCES users(id) ON DELETE RESTRICT,
    action_reason TEXT,
    public_note TEXT,
    internal_note TEXT,
    state_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT moderator_actions_action_type_check
        CHECK (
            action_type IN (
                'archive',
                'unarchive',
                'freeze',
                'unfreeze',
                'merge',
                'merge_reversal',
                'reconsideration_start',
                'reconsideration_end',
                'appeal_submission',
                'appeal_outcome',
                'moderator_note'
            )
        )
);

CREATE INDEX idx_moderator_actions_proposal_id
    ON moderator_actions(proposal_id, created_at DESC);

CREATE INDEX idx_moderator_actions_related_proposal_id
    ON moderator_actions(related_proposal_id, created_at DESC);

CREATE INDEX idx_moderator_actions_moderator_user_id
    ON moderator_actions(moderator_user_id, created_at DESC);
