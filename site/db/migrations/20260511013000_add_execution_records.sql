ALTER TABLE moderator_actions
DROP CONSTRAINT moderator_actions_action_type_check;

ALTER TABLE moderator_actions
ADD CONSTRAINT moderator_actions_action_type_check
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
            'moderator_note',
            'execution_record_created'
        )
    );

CREATE TABLE execution_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    solution_proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE RESTRICT,
    parent_issue_proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE RESTRICT,
    cycle_id UUID NOT NULL REFERENCES cycles(id) ON DELETE RESTRICT,
    locale_id UUID NOT NULL REFERENCES locales(id) ON DELETE RESTRICT,
    created_by_moderator_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    action_description TEXT NOT NULL,
    required_resource_categories JSONB NOT NULL DEFAULT '[]'::jsonb,
    completion_criteria JSONB NOT NULL DEFAULT '[]'::jsonb,
    execution_tracking_entries JSONB NOT NULL DEFAULT '[]'::jsonb,
    proposal_vote_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT execution_records_solution_unique
        UNIQUE (solution_proposal_id),

    CONSTRAINT execution_records_one_solution_per_issue_cycle
        UNIQUE (parent_issue_proposal_id, cycle_id),

    CONSTRAINT execution_records_status_check
        CHECK (status IN ('active', 'paused', 'completed', 'cancelled'))
);

CREATE INDEX idx_execution_records_status_created
    ON execution_records(status, created_at DESC);

CREATE INDEX idx_execution_records_parent_issue
    ON execution_records(parent_issue_proposal_id, created_at DESC);
