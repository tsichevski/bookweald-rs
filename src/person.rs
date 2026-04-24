use crate::normalize;

/// Representation of a person (author, translator, etc.) in the book library.
// #[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub id: String,
    pub first_name: Option<String>,
    pub middle_name: Option<String>,
    pub last_name: Option<String>,
}

/// [normalize last_name first_name middle_name] concatenates the non-empty name parts
/// after applying [Normalize.normalize_name] to each.
/// Panics (equivalent to OCaml Failure) if all parts are empty.
fn normalize(
    last_name: Option<&str>,
    first_name: Option<&str>,
    middle_name: Option<&str>,
) -> Option<String> {
    let names: Vec<String> = [last_name, first_name, middle_name]
        .into_iter()
        .flatten()
        .map(normalize::normalize_name)
        .flatten()
        .collect();

    if names.is_empty() {
        None
    } else {
        Some(names.join(" "))
    }
}

/// [person_create_exn last_name first_name middle_name] creates a new [person] record.
/// The [id] field is set to the normalized name.
/// Panics if name normalized to None.
pub fn person_create_exn(
    last_name: Option<&str>,
    first_name: Option<&str>,
    middle_name: Option<&str>,
) -> Person {
    let id: String =
        normalize(last_name, first_name, middle_name).expect("name normalized to None");

    Person {
        id,
        first_name: first_name.map(str::to_owned),
        middle_name: middle_name.map(str::to_owned),
        last_name: last_name.map(str::to_owned),
    }
}
