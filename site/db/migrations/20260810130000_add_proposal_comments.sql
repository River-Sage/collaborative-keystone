CREATE TABLE proposal_comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE CASCADE,
    author_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    body TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'active',
    hidden_reason TEXT,
    hidden_by_moderator_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    hidden_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT proposal_comments_state_check
        CHECK (state IN ('active', 'hidden')),

    CONSTRAINT proposal_comments_one_per_user
        UNIQUE (proposal_id, author_user_id)
);

CREATE TABLE proposal_comment_votes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    comment_id UUID NOT NULL REFERENCES proposal_comments(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    vote_value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT proposal_comment_votes_value_check
        CHECK (vote_value IN ('like', 'dislike')),

    CONSTRAINT proposal_comment_votes_unique_user_comment
        UNIQUE (comment_id, user_id)
);

CREATE INDEX idx_proposal_comments_proposal_state
    ON proposal_comments(proposal_id, state);

CREATE INDEX idx_proposal_comments_author
    ON proposal_comments(author_user_id);

CREATE INDEX idx_proposal_comment_votes_comment
    ON proposal_comment_votes(comment_id);

CREATE INDEX idx_proposal_comment_votes_user
    ON proposal_comment_votes(user_id);
