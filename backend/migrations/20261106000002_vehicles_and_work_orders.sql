-- Business-wide settings: exactly one row.
CREATE TABLE business_settings (
    id            BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id),
    currency      TEXT NOT NULL DEFAULT 'PLN' CHECK (currency ~ '^[A-Z]{3}$'),
    distance_unit TEXT NOT NULL DEFAULT 'km' CHECK (distance_unit IN ('km', 'mi')),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO business_settings DEFAULT VALUES;

-- Customers and rental drivers. Personal data: never shown on public pages.
CREATE TABLE clients (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name       TEXT NOT NULL,
    phone      TEXT,
    note       TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX clients_name_idx ON clients (lower(name));

-- The vehicle is the center of the model. VIN is not the key: many
-- right-hand-drive Japanese cars only have a frame number, typos happen.
CREATE TABLE vehicles (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vin           TEXT,
    frame_number  TEXT,
    plate         TEXT,
    make          TEXT,
    model         TEXT,
    year          SMALLINT CHECK (year BETWEEN 1900 AND 2100),
    engine        TEXT,
    note          TEXT,
    role          TEXT NOT NULL DEFAULT 'customer' CHECK (role IN ('customer', 'fleet', 'for_sale')),
    -- What the dashboard shows. A UK import reads miles even in a km business.
    odometer_unit TEXT NOT NULL CHECK (odometer_unit IN ('km', 'mi')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (vin IS NOT NULL OR frame_number IS NOT NULL OR plate IS NOT NULL)
);
CREATE UNIQUE INDEX vehicles_vin_idx ON vehicles (vin) WHERE vin IS NOT NULL;
CREATE UNIQUE INDEX vehicles_frame_number_idx ON vehicles (frame_number) WHERE frame_number IS NOT NULL;
-- Plates are searched without spaces and dashes ("WA 12345" = "WA12345").
CREATE INDEX vehicles_plate_idx ON vehicles (regexp_replace(plate, '[\s-]', '', 'g'));

-- Who owned the car and when. The open row (ended_on IS NULL) is the
-- current owner. The history belongs to the car, not to the people.
CREATE TABLE vehicle_ownerships (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vehicle_id UUID NOT NULL REFERENCES vehicles (id) ON DELETE CASCADE,
    client_id  UUID NOT NULL REFERENCES clients (id),
    started_on DATE NOT NULL DEFAULT current_date,
    ended_on   DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (ended_on IS NULL OR ended_on >= started_on)
);
CREATE UNIQUE INDEX vehicle_ownerships_current_idx ON vehicle_ownerships (vehicle_id) WHERE ended_on IS NULL;
CREATE INDEX vehicle_ownerships_client_idx ON vehicle_ownerships (client_id);

CREATE TABLE work_orders (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Short human number for talking about it: "order 42".
    number     BIGINT GENERATED ALWAYS AS IDENTITY UNIQUE,
    vehicle_id UUID NOT NULL REFERENCES vehicles (id),
    -- Who brought the car this time (snapshot; owners change).
    client_id  UUID REFERENCES clients (id),
    status     TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'done', 'cancelled')),
    opened_on  DATE NOT NULL DEFAULT current_date,
    complaint  TEXT,
    notes      TEXT,
    created_by UUID REFERENCES users (id),
    closed_at  TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX work_orders_vehicle_idx ON work_orders (vehicle_id, opened_on DESC);
CREATE INDEX work_orders_open_idx ON work_orders (opened_on DESC) WHERE status = 'open';

CREATE TABLE odometer_readings (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vehicle_id    UUID NOT NULL REFERENCES vehicles (id) ON DELETE CASCADE,
    -- In the vehicle's odometer_unit.
    reading       INTEGER NOT NULL CHECK (reading >= 0),
    read_on       DATE NOT NULL DEFAULT current_date,
    work_order_id UUID UNIQUE REFERENCES work_orders (id) ON DELETE SET NULL,
    created_by    UUID REFERENCES users (id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX odometer_readings_vehicle_idx ON odometer_readings (vehicle_id, read_on DESC, reading DESC);

-- Lines of a work order. Only labour for now; parts arrive with the stock.
CREATE TABLE work_order_items (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_order_id    UUID NOT NULL REFERENCES work_orders (id) ON DELETE CASCADE,
    kind             TEXT NOT NULL DEFAULT 'labour' CHECK (kind IN ('labour')),
    description      TEXT NOT NULL,
    -- Thousandths, so 1.5 hours is 1500. Never floats.
    quantity_milli   BIGINT NOT NULL DEFAULT 1000 CHECK (quantity_milli > 0),
    -- Minor units (grosze) of business_settings.currency. NULL: not priced yet.
    unit_price_minor BIGINT CHECK (unit_price_minor >= 0),
    created_by       UUID REFERENCES users (id),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX work_order_items_order_idx ON work_order_items (work_order_id, created_at);
