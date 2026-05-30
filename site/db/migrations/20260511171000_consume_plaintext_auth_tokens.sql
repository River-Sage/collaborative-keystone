UPDATE email_verification_tokens
SET consumed_at = NOW()
WHERE consumed_at IS NULL
  AND length(token) <> 64;

UPDATE password_reset_tokens
SET consumed_at = NOW()
WHERE consumed_at IS NULL
  AND length(token) <> 64;
