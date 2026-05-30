ALTER TABLE proposals
ADD COLUMN merged_into_proposal_id UUID REFERENCES proposals(id) ON DELETE RESTRICT,
ADD COLUMN archived_reason TEXT;

ALTER TABLE proposals
ADD CONSTRAINT proposals_archived_reason_check
CHECK (
    archived_reason IS NULL
    OR archived_reason IN ('merged', 'moderation', 'manual_archive')
);

CREATE INDEX idx_proposals_merged_into
    ON proposals(merged_into_proposal_id);