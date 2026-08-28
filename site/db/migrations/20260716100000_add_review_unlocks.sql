CREATE TABLE review_unlocks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    cycle_id UUID NOT NULL REFERENCES cycles(id) ON DELETE CASCADE,
    board_code TEXT NOT NULL,
    completed_review_actions INTEGER NOT NULL,
    required_review_actions INTEGER NOT NULL,
    unlocked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT review_unlocks_board_code_check
        CHECK (board_code IN ('issue', 'solution')),
    CONSTRAINT review_unlocks_unique_user_cycle_board
        UNIQUE (user_id, cycle_id, board_code)
);

CREATE INDEX idx_review_unlocks_user_cycle
    ON review_unlocks(user_id, cycle_id);
