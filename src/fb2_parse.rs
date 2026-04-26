use crate::book::Book;
use crate::normalize::normalize_chunk;
use crate::person::{Person, normalize, person_create_exn};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read};
use std::path::Path;

fn apply_aliases(p: Person, aliases: Option<&HashMap<String, Person>>) -> Person {
    let key = &p.id;
    // TODO trace::debug!("Alias %s replaced by %s in %s" candidate.id, e.id, path);
    aliases.and_then(|a| a.get(key)).cloned().unwrap_or(p)
}

pub fn parse_book_info(
    path: &Path,
    aliases: Option<&HashMap<String, Person>>,
) -> Result<Book, Box<dyn std::error::Error>> {
    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();

    // ── Create reader (handles ZIP without lifetime issues) ──
    let mut reader: Reader<Box<dyn BufRead>> =
        if path.extension().and_then(|e| e.to_str()) == Some("zip") {
            let zip_file = File::open(path)?;
            let mut archive = zip::ZipArchive::new(zip_file)?;
            let mut fb2_entry = archive.by_index(0)?; // first .fb2 inside ZIP

            // Read entire entry into memory (FB2 files are small → safe & simple)
            let mut content = Vec::new();
            fb2_entry.read_to_end(&mut content)?;
            Reader::from_reader(Box::new(Cursor::new(content)))
        } else {
            let file = File::open(path)?;
            Reader::from_reader(Box::new(BufReader::new(file)))
        };

    // ── Configuration ──
    let config = reader.config_mut();
    config.trim_text(true);
    config.expand_empty_elements = false; // In this task we can safely ignore empty elements whatsoever

    let mut buf = Vec::new();
    let mut path_stack: Vec<Vec<u8>> = Vec::new();
    let mut current_first_name: Option<String> = None;
    let mut current_middle_name: Option<String> = None;
    let mut current_last_name: Option<String> = None;
    let mut ext_id: Option<String> = None;
    let mut title: Option<String> = None;
    let mut lang: Option<String> = None;
    let mut genre: Option<String> = None;
    let mut version: Option<String> = None;
    let mut encoding: Option<String> = None;

    let mut authors: Vec<Person> = Vec::new();

    // Helper that consumes the current name fields and appends a unique author.
    let append_current_author_unique =
        |last: &mut Option<String>,
         first: &mut Option<String>,
         middle: &mut Option<String>,
         authors: &mut Vec<Person>| {
            match (&last, &first, &middle) {
                (None, _, None) => {
                    // Skip authors with only middlename set
                    *middle = None;
                }
                (last_name, first_name, middle_name) => {
                    match normalize(last_name, first_name, middle_name) {
                        None => tracing::warn!(
                            "Ignoring author with name that normalized to empty in {}",
                            path.display()
                        ),
                        Some(_id) => {
                            let candidate = person_create_exn(last_name, first_name, middle_name);
                            let candidate = apply_aliases(candidate, aliases);
                            if !authors.iter().any(|y| y.id == candidate.id) {
                                authors.push(candidate);
                            }
                        }
                    }
                }
            }

            *first = None;
            *middle = None;
            *last = None;
        };

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Decl(e)) => {
                // This is the <?xml ...?> event
                if let Some(enc) = e.encoder() {
                    encoding = Some(enc.name().to_string());
                }
                continue;
            }

            Ok(Event::Start(ref e)) => {
                let name = e.name().as_ref().to_vec();
                path_stack.push(name);

                let path_slice: Vec<&[u8]> = path_stack.iter().map(|v| v.as_slice()).collect();

                match path_slice.as_slice() {
                    [
                        ..,
                        b"description",
                        b"title-info" | b"document-info",
                        b"author",
                    ] => append_current_author_unique(
                        &mut current_last_name,
                        &mut current_first_name,
                        &mut current_middle_name,
                        &mut authors,
                    ),
                    _ => (),
                }
            }

            Ok(Event::End(_)) => {
                path_stack.pop();
            }

            Ok(Event::Text(e)) => {
                let text = e.decode()?; // Assume text is already trimmed by XML parser
                if text.is_empty() {
                    buf.clear();
                    continue;
                }

                let path_slice: Vec<&[u8]> = path_stack.iter().map(|v| v.as_slice()).collect();
                let text = text.to_string();
                match path_slice.as_slice() {
                    // title-info
                    [.., b"description", b"title-info", b"book-title"] => title = Some(text),
                    [.., b"description", b"title-info", b"lang"] => lang = Some(text),
                    [.., b"description", b"title-info", b"genre"] => genre = Some(text),

                    // author fields
                    [.., b"description", b"title-info", b"author", b"first-name"] => {
                        current_first_name = Some(text);
                    }
                    [.., b"description", b"title-info", b"author", b"middle-name"] => {
                        current_middle_name = Some(text);
                    }
                    [.., b"description", b"title-info", b"author", b"last-name"] => {
                        current_last_name = Some(text);
                    }

                    // document-info
                    [.., b"description", b"document-info", b"id"] => ext_id = Some(text),
                    [.., b"description", b"document-info", b"version"] => version = Some(text),

                    _ => {}
                }
            }

            Ok(Event::Eof) => break,
            Err(e) => return Err(Box::new(e)),
            _ => {}
        }
        buf.clear();
    }

    let title = match title {
        None => return Err("No <book-title> found in FB2 file".into()),
        Some(title) => title,
    };

    // Check title is not empty after normalization
    match normalize_chunk(&title) {
        None => Err(format!("Book title normalizes to empty: '{}'", &title).into()),
        _ => {
            append_current_author_unique(
                &mut current_last_name,
                &mut current_first_name,
                &mut current_middle_name,
                &mut authors,
            );

            let encoding = encoding.unwrap_or("UTF-8".to_string());
            Ok(Book {
                title,
                authors,
                ext_id,
                version,
                lang,
                genre,
                filename,
                encoding,
            })
        }
    }
}
