ALTER TABLE proposals
ADD COLUMN moderation_note TEXT;

ALTER TABLE proposals
DROP CONSTRAINT proposals_archived_reason_check;

ALTER TABLE proposals
ADD CONSTRAINT proposals_archived_reason_check
CHECK (
    archived_reason IS NULL
    OR archived_reason IN ('merged', 'moderation', 'manual_archive', 'irrelevant', 'not_a_fit')
);