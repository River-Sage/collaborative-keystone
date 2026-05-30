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
            'execution_record_updated'
        )
    );
