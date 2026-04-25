// src/alias.rs
use crate::person::{Person, person_create_exn};
use serde_json;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

/// Author alias handling for book metadata normalization.
pub fn person_from_string_exn(s: &str) -> Person {
    let parts: Vec<&str> = s.trim().split_whitespace().collect();

    match parts.as_slice() {
        [last, first, middle] => person_create_exn(Some(last), Some(first), Some(middle)),
        [last, first] => person_create_exn(Some(last), Some(first), None),
        [last] => person_create_exn(Some(last), None, None),
        _ => panic!(
            "Cannot parse string to person: [{}]\n\
             Expected 1–3 parts: \"Last\", \"Last First\", or \"Last First Middle\"",
            s
        ),
    }
}

pub fn load_aliases(path: &str) -> HashMap<String, Person> {
    let file =
        File::open(path).unwrap_or_else(|e| panic!("Cannot open aliases file '{}': {}", path, e));

    let reader = BufReader::new(file);

    let json: serde_json::Value = serde_json::from_reader(reader)
        .unwrap_or_else(|e| panic!("Invalid JSON in '{}': {}", path, e));

    let mut table = HashMap::with_capacity(512);

    if let serde_json::Value::Object(obj) = json {
        for (canonical, aliases_json) in obj {
            if let serde_json::Value::Array(alias_list) = aliases_json {
                let canonical_person = person_from_string_exn(canonical.trim());
                for alias_val in alias_list {
                    if let serde_json::Value::String(alias) = alias_val {
                        let trimmed = alias.trim().to_string();
                        if !trimmed.is_empty() {
                            table.insert(trimmed, canonical_person.clone());
                        }
                    }
                }
            }
        }
    } else {
        panic!("aliases.json must be a JSON object.\nFile: {}", path);
    }

    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_person_from_string_exn() {
        let p1 = person_from_string_exn(" Толстой Лев Николаевич\t");
        assert_eq!(p1.id, "Толстой Лев Николаевич"); // ← now correct
        assert_eq!(p1.last_name, Some("Толстой".to_string()));
        assert_eq!(p1.first_name, Some("Лев".to_string()));
        assert_eq!(p1.middle_name, Some("Николаевич".to_string()));

        let p2 = person_from_string_exn("   Достоевский   ");
        assert_eq!(p2.id, "Достоевский");

        let p3 = person_from_string_exn("Pushkin Alexander");
        assert_eq!(p3.id, "Pushkin Alexander");
    }

    #[test]
    #[should_panic(expected = "Cannot parse string to person")]
    fn test_too_many_parts() {
        person_from_string_exn("Too Many Names Here");
    }

    #[test]
    #[should_panic(expected = "Cannot parse string to person")]
    fn test_empty() {
        person_from_string_exn("  \t\n");
    }
}
