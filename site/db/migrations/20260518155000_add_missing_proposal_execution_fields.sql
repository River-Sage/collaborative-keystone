ALTER TABLE proposals
ADD COLUMN IF NOT EXISTS required_resource_categories JSONB,
ADD COLUMN IF NOT EXISTS completion_criteria JSONB,
ADD COLUMN IF NOT EXISTS execution_tracking_entries JSONB;
