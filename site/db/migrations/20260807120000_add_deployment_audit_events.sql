CREATE TABLE deployment_audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type TEXT NOT NULL,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT deployment_audit_events_type_check
        CHECK (event_type IN ('first_moderator_bootstrap'))
);

CREATE INDEX idx_deployment_audit_events_type_created
    ON deployment_audit_events(event_type, created_at DESC);

CREATE INDEX idx_deployment_audit_events_actor_created
    ON deployment_audit_events(actor_user_id, created_at DESC);
