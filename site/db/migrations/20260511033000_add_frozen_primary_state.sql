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
            'frozen',
            'removed'
        )
    );
