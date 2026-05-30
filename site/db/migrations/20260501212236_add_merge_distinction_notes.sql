-- Add migration script here
CREATE TABLE merge_distinction_notes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE CASCADE,
    target_proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE CASCADE,
    author_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    note_text TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT merge_distinction_notes_unique_relationship
        UNIQUE (source_proposal_id, target_proposal_id),

    CONSTRAINT merge_distinction_notes_no_self_reference
        CHECK (source_proposal_id <> target_proposal_id)
);

CREATE INDEX idx_merge_distinction_notes_source
    ON merge_distinction_notes(source_proposal_id);

CREATE INDEX idx_merge_distinction_notes_target
    ON merge_distinction_notes(target_proposal_id);