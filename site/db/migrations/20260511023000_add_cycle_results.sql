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
            'execution_record_created',
            'execution_record_updated',
            'cycle_result_resolved'
        )
    );

CREATE TABLE cycle_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cycle_id UUID NOT NULL REFERENCES cycles(id) ON DELETE RESTRICT,
    locale_id UUID NOT NULL REFERENCES locales(id) ON DELETE RESTRICT,
    board_code TEXT NOT NULL,
    winning_proposal_id UUID REFERENCES proposals(id) ON DELETE RESTRICT,
    execution_record_id UUID REFERENCES execution_records(id) ON DELETE SET NULL,
    resolved_by_moderator_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    result_status TEXT NOT NULL,
    result_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT cycle_results_cycle_board_unique
        UNIQUE (cycle_id, board_code),

    CONSTRAINT cycle_results_board_code_check
        CHECK (board_code IN ('issue', 'solution')),

    CONSTRAINT cycle_results_status_check
        CHECK (result_status IN ('resolved', 'no_ranked_winner'))
);

CREATE INDEX idx_cycle_results_published
    ON cycle_results(published_at DESC)
    WHERE published_at IS NOT NULL;

CREATE INDEX idx_cycle_results_winning_proposal
    ON cycle_results(winning_proposal_id);
