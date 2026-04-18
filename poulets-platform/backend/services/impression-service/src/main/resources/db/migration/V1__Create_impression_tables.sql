-- Print Jobs Table
CREATE TABLE IF NOT EXISTS print_jobs (
    id UUID PRIMARY KEY,
    document_id UUID NOT NULL,
    demande_id UUID NOT NULL,
    tenant_id VARCHAR(50) NOT NULL,
    client_id UUID NOT NULL,
    status VARCHAR(30) NOT NULL DEFAULT 'EN_ATTENTE',
    priority INT NOT NULL DEFAULT 5,
    document_type VARCHAR(50) NOT NULL,
    document_reference VARCHAR(100),
    operator_id UUID,
    printed_at TIMESTAMPTZ,
    delivered_at TIMESTAMPTZ,
    delivered_to VARCHAR(255),
    delivery_method VARCHAR(50),
    recipient_signature VARCHAR(500),
    copies_count INT NOT NULL DEFAULT 1,
    copies_printed INT DEFAULT 0,
    worm_bucket VARCHAR(100),
    worm_object_key VARCHAR(500),
    worm_locked_at TIMESTAMPTZ,
    worm_retention_until TIMESTAMPTZ,
    is_worm_locked BOOLEAN NOT NULL DEFAULT FALSE,
    document_hash VARCHAR(128),
    blockchain_hash VARCHAR(128),
    pdf_storage_path VARCHAR(500),
    reprint_count INT NOT NULL DEFAULT 0,
    reprint_reason VARCHAR(500),
    reprint_authorized_by UUID,
    original_print_job_id UUID,
    metadata JSONB DEFAULT '{}',
    error_message VARCHAR(1000),
    notes VARCHAR(2000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    version BIGINT DEFAULT 0
);

-- Blockchain Entries Table
CREATE TABLE IF NOT EXISTS blockchain_entries (
    id UUID PRIMARY KEY,
    document_id UUID NOT NULL,
    print_job_id UUID,
    document_hash VARCHAR(128) NOT NULL,
    previous_block_hash VARCHAR(128) NOT NULL,
    block_hash VARCHAR(128) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    tenant_id VARCHAR(50) NOT NULL,
    operator_id UUID NOT NULL,
    action VARCHAR(30) NOT NULL,
    block_number BIGINT NOT NULL,
    nonce BIGINT NOT NULL,
    details VARCHAR(2000),
    client_ip VARCHAR(50),
    user_agent VARCHAR(500),
    synced_to_audit_log BOOLEAN NOT NULL DEFAULT FALSE,
    synced_at TIMESTAMPTZ
);

-- Delivery Records Table
CREATE TABLE IF NOT EXISTS delivery_records (
    id UUID PRIMARY KEY,
    print_job_id UUID NOT NULL,
    document_id UUID NOT NULL,
    tenant_id VARCHAR(50) NOT NULL,
    client_id UUID NOT NULL,
    operator_id UUID NOT NULL,
    delivery_method VARCHAR(30) NOT NULL,
    recipient_name VARCHAR(255) NOT NULL,
    recipient_id_number VARCHAR(50),
    recipient_id_type VARCHAR(50),
    recipient_phone VARCHAR(20),
    recipient_email VARCHAR(255),
    recipient_relationship VARCHAR(100),
    signature_data TEXT,
    signature_hash VARCHAR(128),
    delivery_location VARCHAR(500),
    tracking_number VARCHAR(100),
    courier_name VARCHAR(100),
    metadata JSONB DEFAULT '{}',
    notes VARCHAR(2000),
    delivered_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for print_jobs
CREATE INDEX IF NOT EXISTS idx_print_jobs_document_id ON print_jobs(document_id);
CREATE INDEX IF NOT EXISTS idx_print_jobs_demande_id ON print_jobs(demande_id);
CREATE INDEX IF NOT EXISTS idx_print_jobs_tenant_id ON print_jobs(tenant_id);
CREATE INDEX IF NOT EXISTS idx_print_jobs_status ON print_jobs(status);
CREATE INDEX IF NOT EXISTS idx_print_jobs_operator_id ON print_jobs(operator_id);
CREATE INDEX IF NOT EXISTS idx_print_jobs_created_at ON print_jobs(created_at);
CREATE INDEX IF NOT EXISTS idx_print_jobs_priority ON print_jobs(priority);
CREATE INDEX IF NOT EXISTS idx_print_jobs_client_id ON print_jobs(client_id);

-- Indexes for blockchain_entries
CREATE INDEX IF NOT EXISTS idx_blockchain_document_id ON blockchain_entries(document_id);
CREATE INDEX IF NOT EXISTS idx_blockchain_tenant_id ON blockchain_entries(tenant_id);
CREATE INDEX IF NOT EXISTS idx_blockchain_block_hash ON blockchain_entries(block_hash);
CREATE INDEX IF NOT EXISTS idx_blockchain_timestamp ON blockchain_entries(timestamp);
CREATE INDEX IF NOT EXISTS idx_blockchain_action ON blockchain_entries(action);
CREATE INDEX IF NOT EXISTS idx_blockchain_block_number ON blockchain_entries(block_number);
CREATE INDEX IF NOT EXISTS idx_blockchain_print_job_id ON blockchain_entries(print_job_id);

-- Indexes for delivery_records
CREATE INDEX IF NOT EXISTS idx_delivery_print_job_id ON delivery_records(print_job_id);
CREATE INDEX IF NOT EXISTS idx_delivery_tenant_id ON delivery_records(tenant_id);
CREATE INDEX IF NOT EXISTS idx_delivery_client_id ON delivery_records(client_id);
CREATE INDEX IF NOT EXISTS idx_delivery_delivered_at ON delivery_records(delivered_at);
CREATE INDEX IF NOT EXISTS idx_delivery_tracking_number ON delivery_records(tracking_number);

-- Foreign key (if original print job exists in same table)
ALTER TABLE print_jobs
    ADD CONSTRAINT fk_print_jobs_original
    FOREIGN KEY (original_print_job_id)
    REFERENCES print_jobs(id)
    ON DELETE SET NULL;

-- Foreign key from delivery_records to print_jobs
ALTER TABLE delivery_records
    ADD CONSTRAINT fk_delivery_print_job
    FOREIGN KEY (print_job_id)
    REFERENCES print_jobs(id);

-- Foreign key from blockchain_entries to print_jobs (optional - print_job_id can be null)
-- ALTER TABLE blockchain_entries
--     ADD CONSTRAINT fk_blockchain_print_job
--     FOREIGN KEY (print_job_id)
--     REFERENCES print_jobs(id);
