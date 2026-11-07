//! Input validation. Each function returns the cleaned value or an error code.

pub const LOCALES: &[&str] = &["en", "pl", "ru"];
pub const ROLES: &[&str] = &["owner", "mechanic"];
pub const VEHICLE_ROLES: &[&str] = &["customer", "fleet", "for_sale"];
pub const DISTANCE_UNITS: &[&str] = &["km", "mi"];
pub const WORK_ORDER_STATUSES: &[&str] = &["open", "done", "cancelled"];
pub const PART_UNITS: &[&str] = &["pcs", "l", "kg", "m"];
pub const PART_CONDITIONS: &[&str] = &["new", "used"];

/// Upper limit for odometer readings; anything above is a typo.
pub const MAX_ODOMETER: i32 = 9_999_999;
/// 1 000 000.000 units and 100 000 000.00 per unit keep every total far
/// below i64 limits.
pub const MAX_QUANTITY_MILLI: i64 = 1_000_000_000;
pub const MAX_PRICE_MINOR: i64 = 10_000_000_000;

pub fn username(raw: &str) -> Result<String, &'static str> {
    let v = raw.trim();
    let ok_chars = v
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !(3..=32).contains(&v.len()) || !ok_chars {
        return Err("invalid_username");
    }
    Ok(v.to_string())
}

pub fn password(v: &str) -> Result<(), &'static str> {
    match v.chars().count() {
        0..=7 => Err("password_too_short"),
        8..=256 => Ok(()),
        _ => Err("password_too_long"),
    }
}

pub fn display_name(raw: &str) -> Result<String, &'static str> {
    let v = raw.trim();
    if v.is_empty() || v.chars().count() > 80 {
        return Err("invalid_display_name");
    }
    Ok(v.to_string())
}

pub fn locale(v: &str) -> Result<String, &'static str> {
    LOCALES
        .contains(&v)
        .then(|| v.to_string())
        .ok_or("invalid_locale")
}

pub fn role(v: &str) -> Result<String, &'static str> {
    ROLES
        .contains(&v)
        .then(|| v.to_string())
        .ok_or("invalid_role")
}

/// Trims; empty input means "no value". Used for optional free text.
pub fn optional_text(raw: Option<&str>, max_chars: usize) -> Result<Option<String>, &'static str> {
    match raw.map(str::trim) {
        None | Some("") => Ok(None),
        Some(v) if v.chars().count() > max_chars => Err("text_too_long"),
        Some(v) => Ok(Some(v.to_string())),
    }
}

/// Uppercase, without spaces or dashes: what people type is "jmz bk 12f...".
fn compact_upper(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .flat_map(char::to_uppercase)
        .collect()
}

/// VIN: 11–17 characters (pre-1981 VINs are shorter), no I, O, Q.
/// The check digit is not enforced: it is only mandatory in North America.
pub fn vin(raw: Option<&str>) -> Result<Option<String>, &'static str> {
    let Some(raw) = raw.filter(|v| !v.trim().is_empty()) else {
        return Ok(None);
    };
    let v = compact_upper(raw);
    let ok = (11..=17).contains(&v.len())
        && v.chars().all(|c| {
            c.is_ascii_digit() || (c.is_ascii_uppercase() && !matches!(c, 'I' | 'O' | 'Q'))
        });
    if ok {
        Ok(Some(v))
    } else {
        Err("invalid_vin")
    }
}

/// Japanese frame (chassis) number, e.g. "GX100-1234567". The dash is kept:
/// it separates the model code from the serial.
pub fn frame_number(raw: Option<&str>) -> Result<Option<String>, &'static str> {
    let Some(raw) = raw.filter(|v| !v.trim().is_empty()) else {
        return Ok(None);
    };
    let v: String = raw
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_uppercase)
        .collect();
    let ok = (5..=24).contains(&v.len())
        && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && v.chars().any(|c| c.is_ascii_digit());
    if ok {
        Ok(Some(v))
    } else {
        Err("invalid_frame_number")
    }
}

/// Number plate as it is written (spaces kept for display), uppercase.
/// Cyrillic letters are allowed: Russian and Ukrainian plates use them.
pub fn plate(raw: Option<&str>) -> Result<Option<String>, &'static str> {
    let Some(raw) = raw.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let v: String = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .flat_map(char::to_uppercase)
        .collect();
    let compact = plate_search_key(&v);
    let ok = (2..=12).contains(&compact.chars().count())
        && v.chars()
            .all(|c| c.is_alphanumeric() || c == ' ' || c == '-');
    if ok {
        Ok(Some(v))
    } else {
        Err("invalid_plate")
    }
}

/// Same normalization as the `vehicles_plate_idx` index.
pub fn plate_search_key(v: &str) -> String {
    v.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect()
}

pub fn year(v: Option<i32>) -> Result<Option<i16>, &'static str> {
    match v {
        None => Ok(None),
        Some(y) if (1900..=2100).contains(&y) => Ok(Some(y as i16)),
        Some(_) => Err("invalid_year"),
    }
}

pub fn odometer(v: i64) -> Result<i32, &'static str> {
    if (0..=MAX_ODOMETER as i64).contains(&v) {
        Ok(v as i32)
    } else {
        Err("invalid_odometer")
    }
}

pub fn one_of(v: &str, allowed: &[&str], code: &'static str) -> Result<String, &'static str> {
    allowed.contains(&v).then(|| v.to_string()).ok_or(code)
}

pub fn currency(v: &str) -> Result<String, &'static str> {
    let ok = v.len() == 3 && v.chars().all(|c| c.is_ascii_uppercase());
    ok.then(|| v.to_string()).ok_or("invalid_currency")
}

pub fn client_name(raw: &str) -> Result<String, &'static str> {
    let v = raw.trim();
    if v.is_empty() || v.chars().count() > 120 {
        return Err("invalid_client_name");
    }
    Ok(v.to_string())
}

/// Phones are kept as typed (people write them in many ways), only checked
/// to look like a phone number.
pub fn phone(raw: Option<&str>) -> Result<Option<String>, &'static str> {
    let Some(v) = raw.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let digits = v.chars().filter(char::is_ascii_digit).count();
    let ok = (5..=20).contains(&digits)
        && v.chars().count() <= 32
        && v.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '+' | ' ' | '-' | '(' | ')' | '.'));
    ok.then(|| Some(v.to_string())).ok_or("invalid_phone")
}

pub fn item_description(raw: &str) -> Result<String, &'static str> {
    let v = raw.trim();
    if v.is_empty() {
        return Err("invalid_description");
    }
    if v.chars().count() > 500 {
        return Err("text_too_long");
    }
    Ok(v.to_string())
}

pub fn quantity_milli(v: i64) -> Result<i64, &'static str> {
    if (1..=MAX_QUANTITY_MILLI).contains(&v) {
        Ok(v)
    } else {
        Err("invalid_quantity")
    }
}

pub fn price_minor(v: Option<i64>) -> Result<Option<i64>, &'static str> {
    match v {
        None => Ok(None),
        Some(p) if (0..=MAX_PRICE_MINOR).contains(&p) => Ok(Some(p)),
        Some(_) => Err("invalid_price"),
    }
}

/// Required short text: trimmed, not empty, at most `max_chars`.
fn required_text(raw: &str, max_chars: usize, code: &'static str) -> Result<String, &'static str> {
    let v = raw.trim();
    if v.is_empty() || v.chars().count() > max_chars {
        return Err(code);
    }
    Ok(v.to_string())
}

pub fn part_name(raw: &str) -> Result<String, &'static str> {
    required_text(raw, 160, "invalid_part_name")
}

/// OEM number or the maker's article, uppercase, inner spaces collapsed.
/// Dashes, dots and spaces are kept as written but ignored when searching.
pub fn part_number(raw: Option<&str>) -> Result<Option<String>, &'static str> {
    let Some(raw) = raw.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let v = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase();
    let ok = v.chars().count() <= 40
        && v.chars().any(|c| c.is_alphanumeric())
        && v.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '.' | '/'));
    ok.then_some(Some(v)).ok_or("invalid_part_number")
}

/// Same normalization as the `parts_part_number_idx` index.
pub fn part_number_key(v: &str) -> String {
    v.chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '.'))
        .flat_map(char::to_uppercase)
        .collect()
}

/// Barcodes and QR payloads are compared exactly, so only the surrounding
/// whitespace is trimmed.
pub fn barcode(raw: &str) -> Result<String, &'static str> {
    let v = raw.trim();
    let ok = (1..=128).contains(&v.chars().count()) && !v.chars().any(char::is_control);
    ok.then(|| v.to_string()).ok_or("invalid_barcode")
}

/// Stock quantities may be zero (a minimum level of 0 means "no minimum").
pub fn stock_quantity_milli(v: i64) -> Result<i64, &'static str> {
    if (0..=MAX_QUANTITY_MILLI).contains(&v) {
        Ok(v)
    } else {
        Err("invalid_quantity")
    }
}

/// Line total in minor units: quantity (thousandths) x unit price,
/// rounded half up to a whole minor unit.
pub fn line_total(quantity_milli: i64, unit_price_minor: i64) -> i64 {
    let raw = quantity_milli as i128 * unit_price_minor as i128;
    ((raw + 500) / 1000) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vins() {
        assert_eq!(
            vin(Some(" wvw zzz 1jz-3w386752 ")).unwrap().as_deref(),
            Some("WVWZZZ1JZ3W386752")
        );
        assert_eq!(vin(Some("")), Ok(None));
        assert_eq!(vin(None), Ok(None));
        assert_eq!(vin(Some("WVWZZZ1JZ3W38675O")), Err("invalid_vin")); // letter O
        assert_eq!(vin(Some("SHORT")), Err("invalid_vin"));
    }

    #[test]
    fn frame_numbers() {
        assert_eq!(
            frame_number(Some("gx100 -1234567")).unwrap().as_deref(),
            Some("GX100-1234567")
        );
        assert_eq!(frame_number(Some("ABCDE")), Err("invalid_frame_number")); // no digits
        assert_eq!(frame_number(Some("GX1#0")), Err("invalid_frame_number"));
    }

    #[test]
    fn plates() {
        assert_eq!(
            plate(Some("  wa  12345 ")).unwrap().as_deref(),
            Some("WA 12345")
        );
        assert_eq!(
            plate(Some("а123вс 77")).unwrap().as_deref(),
            Some("А123ВС 77")
        );
        assert_eq!(plate_search_key("WA 123-45"), "WA12345");
        assert_eq!(plate(Some("W")), Err("invalid_plate"));
        assert_eq!(plate(Some("WA_123")), Err("invalid_plate"));
    }

    #[test]
    fn phones() {
        assert_eq!(
            phone(Some(" +48 600-100-200 ")).unwrap().as_deref(),
            Some("+48 600-100-200")
        );
        assert_eq!(phone(Some("call me")), Err("invalid_phone"));
        assert_eq!(phone(Some("123")), Err("invalid_phone"));
    }

    #[test]
    fn part_numbers() {
        assert_eq!(
            part_number(Some("  04152-yzza1 ")).unwrap().as_deref(),
            Some("04152-YZZA1")
        );
        assert_eq!(
            part_number(Some("0 986  494 053")).unwrap().as_deref(),
            Some("0 986 494 053")
        );
        assert_eq!(part_number(Some("")), Ok(None));
        assert_eq!(part_number(Some("--")), Err("invalid_part_number"));
        assert_eq!(part_number(Some("A#1")), Err("invalid_part_number"));
        assert_eq!(part_number_key("0 986-494.053"), "0986494053");
        assert_eq!(part_number_key("04152-yzza1"), "04152YZZA1");
    }

    #[test]
    fn barcodes() {
        assert_eq!(barcode(" 5901234123457\n"), Ok("5901234123457".into()));
        assert_eq!(barcode("   "), Err("invalid_barcode"));
        assert_eq!(barcode("a\u{0}b"), Err("invalid_barcode"));
    }

    #[test]
    fn totals_round_half_up() {
        assert_eq!(line_total(1000, 12_345), 12_345); // 1 x 123.45
        assert_eq!(line_total(1500, 10_000), 15_000); // 1.5 h x 100.00
        assert_eq!(line_total(333, 100), 33); // 0.333 x 1.00 = 0.333
        assert_eq!(line_total(335, 100), 34); // 0.335 -> rounds up
        assert_eq!(
            line_total(MAX_QUANTITY_MILLI, MAX_PRICE_MINOR),
            10_000_000_000_000_000
        );
    }

    #[test]
    fn optional_texts() {
        assert_eq!(optional_text(Some("  "), 10), Ok(None));
        assert_eq!(optional_text(Some(" hi "), 10), Ok(Some("hi".into())));
        assert_eq!(optional_text(Some("abcdefghijk"), 10), Err("text_too_long"));
    }

    #[test]
    fn usernames() {
        assert_eq!(username("  ivan.k ").unwrap(), "ivan.k");
        assert!(username("ab").is_err());
        assert!(username("има").is_err());
        assert!(username("a b c").is_err());
    }

    #[test]
    fn passwords() {
        assert_eq!(password("short"), Err("password_too_short"));
        assert!(password("long enough").is_ok());
        // Length is counted in characters, not bytes.
        assert!(password("пароль12").is_ok());
    }
}
