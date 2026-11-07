-- Spare parts catalogue. The part number (OEM or the maker's article) is how
-- people look a part up; many cheap parts (oil, bulbs) have none.
CREATE TABLE parts (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name             TEXT NOT NULL,
    brand            TEXT,
    part_number      TEXT,
    unit             TEXT NOT NULL DEFAULT 'pcs' CHECK (unit IN ('pcs', 'l', 'kg', 'm')),
    -- Shelf or bin, as written on it.
    location         TEXT,
    -- Thousandths of `unit`, like every quantity. 0: no reorder level.
    min_stock_milli  BIGINT NOT NULL DEFAULT 0 CHECK (min_stock_milli >= 0),
    -- Default price for work orders, minor units. Owners only.
    sell_price_minor BIGINT CHECK (sell_price_minor >= 0),
    note             TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Part numbers are searched without spaces, dashes and dots
-- ("04152-YZZA1" = "04152YZZA1").
CREATE INDEX parts_part_number_idx ON parts (regexp_replace(part_number, '[\s.-]', '', 'g'));
-- The same number from the same brand is the same part.
CREATE UNIQUE INDEX parts_brand_number_idx
    ON parts (lower(coalesce(brand, '')), regexp_replace(part_number, '[\s.-]', '', 'g'))
    WHERE part_number IS NOT NULL;
CREATE INDEX parts_name_idx ON parts (lower(name));

-- One part can carry several barcodes (different suppliers); a barcode
-- belongs to one part.
CREATE TABLE part_barcodes (
    barcode    TEXT PRIMARY KEY,
    part_id    UUID NOT NULL REFERENCES parts (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX part_barcodes_part_idx ON part_barcodes (part_id);

-- Work orders now take parts as well as labour.
ALTER TABLE work_order_items DROP CONSTRAINT work_order_items_kind_check;
ALTER TABLE work_order_items ADD CONSTRAINT work_order_items_kind_check CHECK (kind IN ('labour', 'part'));
ALTER TABLE work_order_items ADD COLUMN part_id UUID REFERENCES parts (id);
ALTER TABLE work_order_items ADD CONSTRAINT work_order_items_part_check
    CHECK ((kind = 'part') = (part_id IS NOT NULL));
CREATE INDEX work_order_items_part_idx ON work_order_items (part_id) WHERE part_id IS NOT NULL;

-- Stock ledger. The stock level is the sum of movements, never a stored
-- counter. A consumption mirrors one part line of a work order and goes
-- with it; a cancelled order has none.
CREATE TABLE stock_movements (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    part_id            UUID NOT NULL REFERENCES parts (id),
    kind               TEXT NOT NULL CHECK (kind IN ('receipt', 'consumption', 'adjustment')),
    -- Signed thousandths: receipts add, consumption takes away.
    quantity_milli     BIGINT NOT NULL CHECK (quantity_milli <> 0),
    -- Purchase price per unit, minor units. Owners only.
    unit_cost_minor    BIGINT CHECK (unit_cost_minor >= 0),
    work_order_item_id UUID UNIQUE REFERENCES work_order_items (id) ON DELETE CASCADE,
    note               TEXT,
    created_by         UUID REFERENCES users (id),
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (kind <> 'receipt' OR quantity_milli > 0),
    CHECK ((kind = 'consumption') = (work_order_item_id IS NOT NULL)),
    CHECK (kind <> 'consumption' OR quantity_milli < 0),
    CHECK (unit_cost_minor IS NULL OR kind = 'receipt')
);
CREATE INDEX stock_movements_part_idx ON stock_movements (part_id, created_at DESC);
