CREATE TABLE notification_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    recipient_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    notification_type TEXT NOT NULL,
    proposal_id UUID REFERENCES proposals(id) ON DELETE CASCADE,
    related_proposal_id UUID REFERENCES proposals(id) ON DELETE CASCADE,
    delivery_channel TEXT NOT NULL DEFAULT 'in_app',
    delivery_status TEXT NOT NULL DEFAULT 'created',
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    delivered_at TIMESTAMPTZ,

    CONSTRAINT notification_events_type_check
        CHECK (notification_type IN ('merge_watch_author', 'merge_watch_moderator')),
    CONSTRAINT notification_events_channel_check
        CHECK (delivery_channel IN ('in_app', 'email', 'log')),
    CONSTRAINT notification_events_status_check
        CHECK (delivery_status IN ('created', 'sent', 'failed', 'read'))
);

CREATE UNIQUE INDEX idx_notification_events_unique_relationship
    ON notification_events (
        recipient_user_id,
        notification_type,
        COALESCE(proposal_id, '00000000-0000-0000-0000-000000000000'::uuid),
        COALESCE(related_proposal_id, '00000000-0000-0000-0000-000000000000'::uuid)
    );

CREATE INDEX idx_notification_events_recipient_created
    ON notification_events(recipient_user_id, created_at DESC);

CREATE TABLE user_activity_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    proposal_id UUID REFERENCES proposals(id) ON DELETE CASCADE,
    related_proposal_id UUID REFERENCES proposals(id) ON DELETE CASCADE,
    client_ip_hint TEXT,
    user_agent_hash TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT user_activity_events_type_check
        CHECK (
            event_type IN (
                'login',
                'proposal_created',
                'review_action',
                'sentiment_vote',
                'merge_vote'
            )
        )
);

CREATE INDEX idx_user_activity_events_user_created
    ON user_activity_events(user_id, created_at DESC);

CREATE INDEX idx_user_activity_events_type_created
    ON user_activity_events(event_type, created_at DESC);

CREATE INDEX idx_user_activity_events_client_created
    ON user_activity_events(client_ip_hint, user_agent_hash, created_at DESC);

CREATE TABLE anti_abuse_flags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    flag_code TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'medium',
    status TEXT NOT NULL DEFAULT 'open',
    proposal_id UUID REFERENCES proposals(id) ON DELETE CASCADE,
    related_proposal_id UUID REFERENCES proposals(id) ON DELETE CASCADE,
    client_ip_hint TEXT,
    user_agent_hash TEXT,
    details JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_at TIMESTAMPTZ,
    reviewed_by_moderator_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    resolution_note TEXT,

    CONSTRAINT anti_abuse_flags_code_check
        CHECK (
            flag_code IN (
                'new_account_activity',
                'rapid_review_activity',
                'rapid_vote_activity',
                'shared_client_identity',
                'shared_device_browser_cluster',
                'merge_signal_cluster'
            )
        ),
    CONSTRAINT anti_abuse_flags_severity_check
        CHECK (severity IN ('low', 'medium', 'high')),
    CONSTRAINT anti_abuse_flags_status_check
        CHECK (status IN ('open', 'acknowledged', 'dismissed'))
);

CREATE INDEX idx_anti_abuse_flags_status_created
    ON anti_abuse_flags(status, created_at DESC);

CREATE INDEX idx_anti_abuse_flags_user_status
    ON anti_abuse_flags(user_id, status, created_at DESC);

CREATE UNIQUE INDEX idx_anti_abuse_flags_open_user
    ON anti_abuse_flags (
        user_id,
        flag_code,
        COALESCE(proposal_id, '00000000-0000-0000-0000-000000000000'::uuid),
        COALESCE(related_proposal_id, '00000000-0000-0000-0000-000000000000'::uuid)
    )
    WHERE user_id IS NOT NULL
      AND status = 'open';
