UPDATE sessions
SET revoked_at = NOW()
WHERE revoked_at IS NULL;
