CREATE TABLE proposal_merge_relationships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE CASCADE,
    target_proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE CASCADE,
    created_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT proposal_merge_relationships_unique_pair
        UNIQUE (source_proposal_id, target_proposal_id),

    CONSTRAINT proposal_merge_relationships_no_self_reference
        CHECK (source_proposal_id <> target_proposal_id),

    CONSTRAINT proposal_merge_relationships_status_check
        CHECK (status IN ('active', 'inactive'))
);

CREATE INDEX idx_proposal_merge_relationships_source
    ON proposal_merge_relationships(source_proposal_id, status);

CREATE INDEX idx_proposal_merge_relationships_target
    ON proposal_merge_relationships(target_proposal_id, status);

ALTER TABLE proposal_merge_votes
ADD COLUMN target_proposal_id UUID REFERENCES proposals(id) ON DELETE SET NULL,
ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

CREATE INDEX idx_proposal_merge_votes_target_proposal_id
    ON proposal_merge_votes(target_proposal_id);

ALTER TABLE merge_distinction_notes
ADD COLUMN difference_type TEXT NOT NULL DEFAULT 'other';

ALTER TABLE merge_distinction_notes
ADD CONSTRAINT merge_distinction_notes_difference_type_check
    CHECK (
        difference_type IN (
            'different_scope',
            'different_cause',
            'different_affected_group',
            'different_implementation',
            'different_completion_criteria',
            'other'
        )
    );

INSERT INTO proposal_merge_relationships (
    source_proposal_id,
    target_proposal_id,
    created_by_user_id,
    status,
    created_at,
    updated_at
)
SELECT
    source_proposal_id,
    target_proposal_id,
    author_user_id,
    'active',
    created_at,
    updated_at
FROM merge_distinction_notes
ON CONFLICT (source_proposal_id, target_proposal_id)
DO NOTHING;
