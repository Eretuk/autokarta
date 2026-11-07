-- Used parts are common: the same part number can be on the shelf new and
-- used, with separate stock and price. Condition is part of the identity.
ALTER TABLE parts
    ADD COLUMN condition TEXT NOT NULL DEFAULT 'new' CHECK (condition IN ('new', 'used'));

DROP INDEX parts_brand_number_idx;
CREATE UNIQUE INDEX parts_brand_number_idx
    ON parts (lower(coalesce(brand, '')), regexp_replace(part_number, '[\s.-]', '', 'g'), condition)
    WHERE part_number IS NOT NULL;
