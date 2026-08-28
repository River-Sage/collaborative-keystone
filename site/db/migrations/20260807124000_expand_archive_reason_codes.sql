ALTER TABLE proposals
DROP CONSTRAINT proposals_archived_reason_check;

ALTER TABLE proposals
ADD CONSTRAINT proposals_archived_reason_check
CHECK (
    archived_reason IS NULL
    OR archived_reason IN (
        'merged',
        'duplicate',
        'unsafe_illegal_deceptive',
        'spam_abuse',
        'irrelevant',
        'minimum_quality',
        'superseded',
        'moderation',
        'manual_archive',
        'not_a_fit',
        'cycle_closed'
    )
);
