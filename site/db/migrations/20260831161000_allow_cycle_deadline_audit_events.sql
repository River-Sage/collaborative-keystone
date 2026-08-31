ALTER TABLE deployment_audit_events
    DROP CONSTRAINT deployment_audit_events_type_check;

ALTER TABLE deployment_audit_events
    ADD CONSTRAINT deployment_audit_events_type_check
        CHECK (event_type IN (
            'first_moderator_bootstrap',
            'cycle_deadline_extended'
        ));
