use crate::normalize::normalize_chunk;
use crate::person::Person;
use md5;

/// Book record and creation utilities.
pub struct Book {
    /// External ID (e.g. from <id> element). Defined for most books.
    pub ext_id: Option<String>,

    /// Optional version of the book (e.g. "1.1", "1.2", ...).
    /// The tuple (version, ext_id) should be unique, but this is not enforced.
    pub version: Option<String>,

    /// Book title — this field is required and must not be empty after normalization.
    pub title: String,

    /// List of authors. May be empty (e.g. for magazines or anonymous works).
    pub authors: Vec<Person>,

    /// Book language as specified in the FB2 metadata (not validated).
    pub lang: Option<String>,

    /// Book genre as specified in the FB2 metadata (not validated).
    pub genre: Option<String>,

    /// Original filename without the .fb2 extension.
    pub filename: String,

    /// Original character encoding of the file (e.g. "utf8", "windows-1251").
    pub encoding: String,
}

/// [book_create_exn title authors ext_id version lang genre filename encoding]
/// Creates a new [book] record.
/// - Computes id from normalized title + author ids.
/// - Panics with descriptive message if title is empty after normalization
///   or if no valid data is provided (equivalent to OCaml Failure).
pub fn book_create_exn(
    title: String,
    authors: Vec<Person>,
    ext_id: Option<String>,
    version: Option<String>,
    lang: Option<String>,
    genre: Option<String>,
    filename: String,
    encoding: String,
) -> Book {
    // Check title is not empty after normalization
    normalize_chunk(&title).expect(&format!("title normalizes to empty: '{}'", title));

    Book {
        ext_id,
        version,
        title,
        authors,
        lang,
        genre,
        filename,
        encoding,
    }
}

/// Computes a MD5 digest for a book.
///
/// `ext_id` and `version` default to empty string if `None`
/// `title` is passed through `normalize_chunk` (panick on failure)
/// `authors` are expected to be already normalized IDs
pub fn book_digest(
    Book {
        ext_id,
        version,
        title,
        authors,
        ..
    }: &Book,
) -> String {
    let ext_id = ext_id.as_deref().unwrap_or("");
    let version = version.as_deref().unwrap_or("");
    let norm_title = normalize_chunk(title).unwrap();

    // Concat: norm_title, ext_id, version, author1, author2, ...
    let mut s = String::new();
    s.push_str(&norm_title);
    s.push_str(ext_id);
    s.push_str(version);
    for a in authors {
        s.push_str(&a.id);
    }
    hex::encode(md5::compute(s).0)
}
