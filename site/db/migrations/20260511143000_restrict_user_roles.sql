UPDATE users
SET role_code = 'registered_user'
WHERE role_code NOT IN ('registered_user', 'moderator');

ALTER TABLE users
ADD CONSTRAINT users_role_code_check
CHECK (role_code IN ('registered_user', 'moderator'));
