use bookweald_rs::fb2_parse::parse_book_info;
use bookweald_rs::person::Person;
use std::collections::HashMap;
use std::path::Path;

/// Helper for success test cases (with optional alias support).
///
/// Parses the given fixture (optionally applying aliases) and asserts
/// that the extracted metadata matches the expected values.
fn test_success(
    filename: &str,
    title: &str,
    last_name: Option<&str>,
    first_name: Option<&str>,
    middle_name: Option<&str>,
    encoding: &str,
    aliases: Option<&HashMap<String, Person>>,
) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixture_path = Path::new(manifest_dir)
        .join("tests")
        .join("fixtures")
        .join(filename);

    let book = parse_book_info(&fixture_path, aliases)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", filename, e));

    assert_eq!(book.title, title, "Wrong title in {}", filename);
    assert_eq!(book.encoding, encoding, "Wrong encoding in {}", filename);
    assert_eq!(
        book.authors.len(),
        1,
        "Expected exactly 1 author in {}",
        filename
    );
    let author = &book.authors[0];
    assert_eq!(author.last_name.as_deref(), last_name, "Wrong last name");
    assert_eq!(author.first_name.as_deref(), first_name, "Wrong first name");
    assert_eq!(
        author.middle_name.as_deref(),
        middle_name,
        "Wrong middle name"
    );
}

/// Helper for failure test cases.
///
/// Verifies that parsing the given bad fixture produces an error
/// containing the expected error message substring.
fn test_failure(filename: &str, error_contains: &str) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let bad_path = Path::new(manifest_dir)
        .join("tests")
        .join("bads")
        .join(filename);
    let err = parse_book_info(&bad_path, None).unwrap_err();
    assert!(
        err.to_string().contains(error_contains),
        "Expected error containing '{}', got: {}",
        error_contains,
        err
    );
}

// ==================== SUCCESS TESTS ====================

#[test]
fn test_parse_utf8_simple_fb2() {
    test_success(
        "utf8_simple.fb2",
        "Название книжки",
        Some("Толстой"),
        Some("Лев"),
        None,
        "UTF-8",
        None,
    );
}

#[test]
fn test_parse_cp1251_simple_fb2() {
    test_success(
        "cp1251_simple.fb2",
        "Название книжки",
        Some("Толстой"),
        Some("Лев"),
        None,
        "windows-1251",
        None,
    );
}

#[test]
fn test_parse_koi8r_simple_fb2() {
    test_success(
        "koi8r_simple.fb2",
        "Название книжки",
        Some("Толстой"),
        Some("Лев"),
        None,
        "KOI8-R",
        None,
    );
}

#[test]
fn test_parse_zipped_fb2() {
    test_success(
        "zipped.fb2.zip",
        "Название книжки",
        Some("Толстой"),
        Some("Лев"),
        None,
        "UTF-8",
        None,
    );
}

#[test]
fn test_parse_no_xml_decl_fb2() {
    test_success(
        "no_xml_decl.fb2",
        "Название книжки",
        Some("Толстой"),
        Some("Лев"),
        None,
        "UTF-8",
        None,
    );
}

/// Tests alias resolution using fixtures/aliases.json.
///
/// The fixture `aliased_author.fb2` contains a non-canonical author name
/// that should be replaced by the canonical form from the aliases map.
#[test]
fn test_parse_fb2_with_aliases() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    let aliases_path = Path::new(manifest_dir)
        .join("tests")
        .join("fixtures")
        .join("aliases.json");

    let aliases =
        bookweald_rs::alias::load_aliases(aliases_path.to_str().expect("valid UTF-8 path"));

    test_success(
        "aliased_author.fb2",
        "Название книжки",
        Some("Толстой"),
        Some("Лев"),
        Some("Николаевич"),
        "UTF-8",
        Some(&aliases),
    );
}

// ==================== FAILURE TESTS ====================

#[test]
fn test_parse_fb2_missing_title() {
    test_failure("no_title.fb2", "No <book-title> found");
}

#[test]
fn test_parse_fb2_invalid_title() {
    test_failure("invalid_title.fb2", "title normalizes to empty");
}
