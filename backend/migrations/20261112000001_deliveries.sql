-- Supplier deliveries: one row per purchase invoice (faktura), its lines and
-- the files that came with it (PDF, KSeF XML, a photo of a paper invoice).
-- All of it is financial data: owners only.

CREATE TABLE suppliers (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name       TEXT NOT NULL,
    -- NIP in Poland; digits and letters only, so "PL 123-456-78-90" = "PL1234567890".
    tax_id     TEXT,
    note       TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX suppliers_tax_id_idx ON suppliers (tax_id) WHERE tax_id IS NOT NULL;
CREATE INDEX suppliers_name_idx ON suppliers (lower(name));

-- A delivery is a draft until it is posted; only posting puts the lines on
-- stock, so a half-typed (or, later, machine-read) invoice never changes it.
CREATE TABLE deliveries (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    number                 BIGINT GENERATED ALWAYS AS IDENTITY UNIQUE,
    supplier_id            UUID REFERENCES suppliers (id),
    invoice_number         TEXT,
    -- Date of the invoice, not of typing it in: monthly purchases follow it.
    invoice_date           DATE NOT NULL DEFAULT current_date,
    -- Net total as printed on the invoice, to check the typed lines against.
    invoice_total_net_minor BIGINT CHECK (invoice_total_net_minor >= 0),
    note                   TEXT,
    status                 TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'posted')),
    posted_at              TIMESTAMPTZ,
    posted_by              UUID REFERENCES users (id),
    created_by             UUID REFERENCES users (id),
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((status = 'posted') = (posted_at IS NOT NULL))
);
-- The same invoice typed in twice would count the parts twice.
CREATE UNIQUE INDEX deliveries_invoice_idx
    ON deliveries (supplier_id, lower(invoice_number))
    WHERE supplier_id IS NOT NULL AND invoice_number IS NOT NULL;
CREATE INDEX deliveries_date_idx ON deliveries (invoice_date DESC);

-- A line may stay unmatched (no part yet) while the delivery is a draft:
-- `description` and `part_number` keep what the invoice says.
CREATE TABLE delivery_items (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    delivery_id      UUID NOT NULL REFERENCES deliveries (id) ON DELETE CASCADE,
    part_id          UUID REFERENCES parts (id),
    description      TEXT,
    part_number      TEXT,
    quantity_milli   BIGINT NOT NULL CHECK (quantity_milli > 0),
    -- Net purchase price per unit, minor units, as on the invoice.
    unit_cost_minor  BIGINT NOT NULL CHECK (unit_cost_minor >= 0),
    vat_percent      SMALLINT NOT NULL DEFAULT 23 CHECK (vat_percent BETWEEN 0 AND 100),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (part_id IS NOT NULL OR description IS NOT NULL)
);
CREATE INDEX delivery_items_delivery_idx ON delivery_items (delivery_id, created_at);
CREATE INDEX delivery_items_part_idx ON delivery_items (part_id) WHERE part_id IS NOT NULL;

-- Files live in the database, not on disk, so the regular database backup
-- covers them too.
CREATE TABLE delivery_files (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    delivery_id  UUID NOT NULL REFERENCES deliveries (id) ON DELETE CASCADE,
    file_name    TEXT NOT NULL,
    content_type TEXT NOT NULL,
    size_bytes   INTEGER NOT NULL CHECK (size_bytes > 0),
    data         BYTEA NOT NULL,
    created_by   UUID REFERENCES users (id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX delivery_files_delivery_idx ON delivery_files (delivery_id, created_at);

-- A posted line owns its receipt movement, like a work order part line owns
-- its consumption.
ALTER TABLE stock_movements
    ADD COLUMN delivery_item_id UUID UNIQUE REFERENCES delivery_items (id) ON DELETE CASCADE;
ALTER TABLE stock_movements
    ADD CONSTRAINT stock_movements_delivery_check CHECK (delivery_item_id IS NULL OR kind = 'receipt');

-- A stock count may carry a cost too: the first stocktaking puts years of
-- parts on the books, and their value is needed for the stock report.
ALTER TABLE stock_movements DROP CONSTRAINT stock_movements_check3;
ALTER TABLE stock_movements
    ADD CONSTRAINT stock_movements_cost_check CHECK (unit_cost_minor IS NULL OR kind <> 'consumption');

-- Default markup for suggested sale prices; NULL: no suggestions.
ALTER TABLE business_settings
    ADD COLUMN markup_percent SMALLINT CHECK (markup_percent BETWEEN 0 AND 1000);
