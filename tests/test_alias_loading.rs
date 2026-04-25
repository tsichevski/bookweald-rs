use bookweald_rs::alias::load_aliases;
use std::path::Path;

#[test]
fn test_load_aliases_from_sample_fixture() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixture_path = Path::new(manifest_dir)
        .join("tests")
        .join("fixtures")
        .join("aliases.json");

    let fixture_str = fixture_path
        .to_str()
        .expect("fixture path must be valid UTF-8");

    let aliases = load_aliases(fixture_str);

    // Basic presence checks
    assert!(aliases.contains_key("Толстой"));
    assert!(aliases.contains_key("Л. Н. Толстой"));
    assert!(aliases.contains_key("Dostoevsky F."));

    // Canonical person correctness
    let tolstoy = aliases.get("Толстой").unwrap();
    assert_eq!(tolstoy.id, "Толстой Лев Николаевич");
    assert_eq!(tolstoy.last_name, Some("Толстой".to_string()));
    assert_eq!(tolstoy.first_name, Some("Лев".to_string()));
    assert_eq!(tolstoy.middle_name, Some("Николаевич".to_string()));

    let dost = aliases.get("Достоевский").unwrap();
    assert_eq!(dost.id, "Достоевский Федор Михайлович");
}

#[test]
#[should_panic(expected = "Cannot open aliases file")]
fn test_load_aliases_missing_file() {
    load_aliases("/non/existent/aliases.json");
}

#[test]
#[should_panic(expected = "Invalid JSON")]
fn test_load_aliases_invalid_json() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let bad_path = Path::new(manifest_dir)
        .join("tests")
        .join("bads")
        .join("aliases.json");

    let bad_str = bad_path.to_str().expect("fixture path must be valid UTF-8");

    // For simplicity we can also just pass a non-JSON path, but a dedicated fixture is cleaner
    load_aliases(bad_str);
}
