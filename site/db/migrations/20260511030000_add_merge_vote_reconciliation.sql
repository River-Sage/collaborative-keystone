CREATE TABLE proposal_merge_vote_reconciliations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE RESTRICT,
    target_proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE RESTRICT,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    vote_kind TEXT NOT NULL,
    source_vote_value TEXT,
    target_existing_vote_value TEXT,
    outcome TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT proposal_merge_vote_reconciliations_vote_kind_check
        CHECK (vote_kind IN ('sentiment')),

    CONSTRAINT proposal_merge_vote_reconciliations_outcome_check
        CHECK (
            outcome IN (
                'transferred',
                'discarded_same_target_vote',
                'discarded_conflicting_target_vote'
            )
        )
);

CREATE INDEX idx_proposal_merge_vote_reconciliations_source
    ON proposal_merge_vote_reconciliations(source_proposal_id, created_at DESC);

CREATE INDEX idx_proposal_merge_vote_reconciliations_target
    ON proposal_merge_vote_reconciliations(target_proposal_id, created_at DESC);

CREATE INDEX idx_proposal_merge_vote_reconciliations_user
    ON proposal_merge_vote_reconciliations(user_id, created_at DESC);
