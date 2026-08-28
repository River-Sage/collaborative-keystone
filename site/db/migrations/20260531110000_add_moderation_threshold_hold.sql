ALTER TABLE proposals
ADD COLUMN high_moderation_watch_started_at TIMESTAMPTZ;

CREATE INDEX idx_proposals_high_moderation_watch_started_at
    ON proposals(high_moderation_watch_started_at)
    WHERE high_moderation_watch_started_at IS NOT NULL;

UPDATE proposals
SET high_moderation_watch_started_at = NOW()
WHERE unsafe_count >= 8
   OR (
        (support_count + not_a_fit_count + unclear_count + unsafe_count + merge_count) > 0
        AND unsafe_count::numeric
            / (support_count + not_a_fit_count + unclear_count + unsafe_count + merge_count)::numeric >= 0.50
   );
