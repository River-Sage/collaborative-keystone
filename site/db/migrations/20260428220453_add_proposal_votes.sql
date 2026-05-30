CREATE TABLE proposal_sentiment_votes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    vote_value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT proposal_sentiment_votes_value_check
        CHECK (vote_value IN ('support', 'not_a_fit', 'unclear', 'unsafe')),

    CONSTRAINT proposal_sentiment_votes_unique_user_proposal
        UNIQUE (proposal_id, user_id)
);

CREATE TABLE proposal_merge_votes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT proposal_merge_votes_unique_user_proposal
        UNIQUE (proposal_id, user_id)
);

CREATE INDEX idx_proposal_sentiment_votes_proposal_id
    ON proposal_sentiment_votes(proposal_id);

CREATE INDEX idx_proposal_sentiment_votes_user_id
    ON proposal_sentiment_votes(user_id);

CREATE INDEX idx_proposal_merge_votes_proposal_id
    ON proposal_merge_votes(proposal_id);

CREATE INDEX idx_proposal_merge_votes_user_id
    ON proposal_merge_votes(user_id);-- Add migration script here
