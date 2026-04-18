-- V2: Ajout des champs de verification QR code HMAC-signe
-- Les codes de verification proviennent de validation-acte-service (HMAC-SHA256)

ALTER TABLE print_jobs ADD COLUMN qr_verification_code VARCHAR(500);
ALTER TABLE print_jobs ADD COLUMN verification_url VARCHAR(500);

CREATE INDEX idx_print_jobs_qr_code ON print_jobs(qr_verification_code);
