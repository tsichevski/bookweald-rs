//! String normalization utilities for Bookweald.
//!
//! Cleans names extracted from FB2 files (especially Russian books) for
//! stable `Person.id` / `Book.id` generation and filesystem-safe names.

/// Normalizes a full name string (may contain hyphens).
///
/// - Splits on `-`
/// - Normalizes each part with [`normalize_chunk`]
/// - Joins surviving parts with `-`
/// - Returns `None` if the final result is empty
///
/// # Examples
///
/// ```
/// use bookweald_rs::normalize::normalize_name;
///
/// assert_eq!(normalize_name(&"1щёпкина ".to_owned()), Some("Щепкина".to_string()));
/// assert_eq!(normalize_name(&"Щепкина-Куперник".to_owned()), Some("Щепкина-Куперник".to_string()));
/// assert_eq!(normalize_name(&" !4-###".to_owned()), None);
/// ```
pub fn normalize_name(s: &String) -> Option<String> {
    let joined: String = s
        .split('-')
        .filter_map(normalize_chunk)
        .collect::<Vec<_>>()
        .join("-");

    (!joined.is_empty()).then_some(joined)
}

/// Internal helper: normalizes a single chunk (part between hyphens).
///
/// - Replaces Ё/ё with е
/// - Keeps only alphabetic Unicode characters
/// - Applies title case (first letter upper, rest lower)
pub fn normalize_chunk(s: &str) -> Option<String> {
    let cleaned: String = s
        .chars()
        .filter_map(|ch| match ch {
            'Ё' | 'ё' => Some('е'),
            c if c.is_alphabetic() => Some(c),
            _ => None,
        })
        .collect();

    if cleaned.is_empty() {
        return None;
    }

    // Title-case: first letter uppercase, rest lowercase
    let mut chars = cleaned.chars();
    let first = chars.next()?.to_uppercase().collect::<String>();
    let rest: String = chars
        .map(|c| c.to_lowercase().collect::<String>())
        .collect();

    Some(format!("{first}{rest}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ====================== normalize_name ======================

    #[test]
    fn test_normalize_name_basic() {
        assert_eq!(
            normalize_name(&"1щёпкина ".to_owned()),
            Some("Щепкина".to_string())
        );
        assert_eq!(
            normalize_name(&"Щепкина-Куперник".to_owned()),
            Some("Щепкина-Куперник".to_string())
        );
        assert_eq!(normalize_name(&" !4-###".to_owned()), None);
    }

    #[test]
    fn test_normalize_name_hyphenated() {
        assert_eq!(
            normalize_name(&"иван-иванович".to_owned()),
            Some("Иван-Иванович".to_string())
        );
        assert_eq!(
            normalize_name(&"Л. Н. Толстой".to_owned()),
            Some("Лнтолстой".to_string())
        );
        assert_eq!(
            normalize_name(&"Алексей-Николаевич-Толстой".to_owned()),
            Some("Алексей-Николаевич-Толстой".to_string())
        );
    }

    #[test]
    fn test_normalize_name_empty_or_only_junk() {
        assert_eq!(normalize_name(&"".to_owned()), None);
        assert_eq!(normalize_name(&"   ".to_owned()), None);
        assert_eq!(normalize_name(&"---".to_owned()), None);
        assert_eq!(normalize_name(&"123!@#".to_owned()), None);
        assert_eq!(normalize_name(&"- - -".to_owned()), None);
        assert_eq!(normalize_name(&"!@#-123-###".to_owned()), None);
    }

    #[test]
    fn test_normalize_name_mixed_chunks() {
        assert_eq!(
            normalize_name(&"Ёлка-ёжик-123-!!!".to_owned()),
            Some("Елка-Ежик".to_string())
        );
        assert_eq!(
            normalize_name(&"  Hello-   World!  ".to_owned()),
            Some("Hello-World".to_string())
        );
        assert_eq!(
            normalize_name(&"αβγ-123-δεζ".to_owned()),
            Some("Αβγ-Δεζ".to_string())
        );
    }

    #[test]
    fn test_normalize_name_single_letter() {
        assert_eq!(normalize_name(&"а".to_owned()), Some("А".to_string()));
        assert_eq!(normalize_name(&"ё".to_owned()), Some("Е".to_string()));
        assert_eq!(
            normalize_name(&"A-B-C".to_owned()),
            Some("A-B-C".to_string())
        );
    }

    // ====================== normalize_chunk ======================

    #[test]
    fn test_normalize_chunk_basic() {
        assert_eq!(normalize_chunk("лев"), Some("Лев".to_string()));
        assert_eq!(
            normalize_chunk("НИКОЛАЕВИЧ"),
            Some("Николаевич".to_string())
        );
        assert_eq!(normalize_chunk("Толстой"), Some("Толстой".to_string()));
    }

    #[test]
    fn test_normalize_chunk_with_non_alpha() {
        assert_eq!(
            normalize_chunk("hello-world"),
            Some("Helloworld".to_string())
        );
        assert_eq!(
            normalize_chunk("user123!@#name"),
            Some("Username".to_string())
        );
        assert_eq!(normalize_chunk("---abc---"), Some("Abc".to_string()));
        assert_eq!(normalize_chunk("123!@#"), None);
    }

    #[test]
    fn test_normalize_chunk_cyrillic() {
        assert_eq!(normalize_chunk("привет"), Some("Привет".to_string()));
        assert_eq!(normalize_chunk("МИР"), Some("Мир".to_string()));
    }

    #[test]
    fn test_normalize_chunk_yo_replacement() {
        assert_eq!(normalize_chunk("Ёлка"), Some("Елка".to_string()));
        assert_eq!(normalize_chunk("ёжик"), Some("Ежик".to_string()));
        assert_eq!(normalize_chunk("всё"), Some("Все".to_string()));
        assert_eq!(normalize_chunk("ЁЁЁ"), Some("Еее".to_string()));
    }

    #[test]
    fn test_normalize_chunk_mixed() {
        assert_eq!(
            normalize_chunk("Hello-Ёжик-123!"),
            Some("Helloежик".to_string())
        );
        assert_eq!(normalize_chunk("Rust-Ё-2025"), Some("Rustе".to_string()));
    }

    #[test]
    fn test_normalize_chunk_unicode() {
        assert_eq!(
            normalize_chunk(" naïve café"),
            Some("Naïvecafé".to_string())
        );
        assert_eq!(normalize_chunk("αβγ"), Some("Αβγ".to_string()));
    }

    #[test]
    fn test_normalize_chunk_edge_cases() {
        assert_eq!(normalize_chunk(""), None);
        assert_eq!(normalize_chunk(" "), None);
        assert_eq!(normalize_chunk("!@#"), None);
        assert_eq!(normalize_chunk("a"), Some("A".to_string()));
        assert_eq!(normalize_chunk("ё"), Some("Е".to_string()));
        assert_eq!(normalize_chunk("Я"), Some("Я".to_string()));
    }
}
