DELETE FROM proposal_merge_votes
WHERE target_proposal_id IS NULL;

ALTER TABLE proposal_merge_votes
ALTER COLUMN target_proposal_id SET NOT NULL;

UPDATE proposals p
SET merge_count = (
    SELECT COUNT(*)::int
    FROM proposal_merge_votes mv
    WHERE mv.proposal_id = p.id
      AND EXISTS (
        SELECT 1
        FROM proposals target
        WHERE target.id = mv.target_proposal_id
          AND target.primary_state = 'active'
      )
      AND EXISTS (
        SELECT 1
        FROM proposal_merge_relationships r
        WHERE r.source_proposal_id = mv.proposal_id
          AND r.target_proposal_id = mv.target_proposal_id
          AND r.status = 'active'
      )
);
