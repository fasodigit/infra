-- ============================================================================
-- ETAT-CIVIL impression-service: Soft-delete + WORM
-- ============================================================================

-- Print jobs: soft delete columns
ALTER TABLE print_jobs ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
ALTER TABLE print_jobs ADD COLUMN IF NOT EXISTS deleted_by UUID;
CREATE INDEX IF NOT EXISTS idx_print_jobs_active ON print_jobs (deleted_at) WHERE deleted_at IS NULL;

-- WORM: completed print jobs (TERMINE) are legally binding records
CREATE OR REPLACE RULE print_jobs_no_update_termine AS ON UPDATE TO print_jobs
  WHERE OLD.status = 'TERMINE' DO INSTEAD NOTHING;

-- Blockchain entries: absolute WORM — immutable hash chain
CREATE OR REPLACE RULE blockchain_no_update AS ON UPDATE TO blockchain_entries DO INSTEAD NOTHING;
CREATE OR REPLACE RULE blockchain_no_delete AS ON DELETE TO blockchain_entries DO INSTEAD NOTHING;

-- Delivery records: absolute WORM — delivery receipts are legal documents
CREATE OR REPLACE RULE delivery_records_no_update AS ON UPDATE TO delivery_records DO INSTEAD NOTHING;
CREATE OR REPLACE RULE delivery_records_no_delete AS ON DELETE TO delivery_records DO INSTEAD NOTHING;
