UPDATE proposals p
SET merge_count = (
    SELECT COUNT(*)::int
    FROM proposal_merge_votes mv
    WHERE mv.proposal_id = p.id
      AND mv.target_proposal_id IS NOT NULL
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
