-- The business itself: its details appear on printed work orders and tell
-- purchase invoices from issued ones (by NIP) once KSeF import comes.
-- All optional in the database: an install that is already running has
-- none of them, and the app reminds the owner instead of refusing to work.
ALTER TABLE business_settings
    ADD COLUMN company_name        TEXT,
    ADD COLUMN tax_id              TEXT,
    ADD COLUMN regon               TEXT,
    ADD COLUMN legal_address       TEXT,
    ADD COLUMN service_address     TEXT,
    ADD COLUMN phone               TEXT,
    ADD COLUMN email               TEXT,
    ADD COLUMN contact_person      TEXT,
    ADD COLUMN bank_account        TEXT,
    ADD COLUMN default_vat_percent SMALLINT NOT NULL DEFAULT 23
        CHECK (default_vat_percent BETWEEN 0 AND 100);

-- Every purchase is a document. Most are invoices; a shop receipt
-- (paragon) or a used part bought from a private person has no invoice,
-- and the accountant has to know which is which.
ALTER TABLE deliveries
    ADD COLUMN document_type TEXT NOT NULL DEFAULT 'invoice'
        CHECK (document_type IN ('invoice', 'receipt', 'none'));
