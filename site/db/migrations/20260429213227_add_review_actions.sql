CREATE TABLE review_actions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE CASCADE,
    cycle_id UUID NOT NULL REFERENCES cycles(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT review_actions_unique_user_proposal_cycle
        UNIQUE (user_id, proposal_id, cycle_id)
);

CREATE INDEX idx_review_actions_user_cycle
    ON review_actions(user_id, cycle_id);

CREATE INDEX idx_review_actions_proposal_cycle
    ON review_actions(proposal_id, cycle_id);