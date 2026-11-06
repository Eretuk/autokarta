//! Input validation. Each function returns the cleaned value or an error code.

pub const LOCALES: &[&str] = &["en", "pl", "ru"];
pub const ROLES: &[&str] = &["owner", "mechanic"];

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

#[cfg(test)]
mod tests {
    use super::*;

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
