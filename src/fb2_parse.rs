use anyhow::{Context, Result, bail};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read};
use std::path::Path;

use crate::book::Book;
use crate::person::{Person, normalize, person_create_exn};
use ahash::HashMap;

fn apply_aliases<'a>(p: &'a Person, aliases: &'a Option<HashMap<String, Person>>) -> &'a Person {
    match aliases {
        None => p,
        Some(table) => {
            let key = &p.id;
            match table.get(key) {
                None => p,
                Some(ap) => {
                    tracing::debug!("{} replaced by alias {}", key, &ap.id);
                    &ap
                }
            }
        }
    }
}

pub fn parse_book_info(path: &Path, aliases: &Option<HashMap<String, Person>>) -> Result<Book> {
    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .context(format!(
            "Invalid UTF-8 filename or missing stem: {}",
            path.display()
        ))?
        .to_string();

    // ── Create reader (handles ZIP) ──
    let mut reader: Reader<Box<dyn BufRead>> = if path.extension().and_then(|e| e.to_str())
        == Some("zip")
    {
        let zip_file =
            File::open(path).with_context(|| format!("Failed to open ZIP: {}", path.display()))?;

        let mut archive = zip::ZipArchive::new(zip_file).context("Failed to open ZIP archive")?;

        let mut fb2_entry = archive
            .by_index(0)
            .context("ZIP archive contains no files")?;

        // Read entire entry into memory
        let mut content = Vec::new();
        fb2_entry
            .read_to_end(&mut content)
            .context("Failed to read FB2 from ZIP")?;

        Reader::from_reader(Box::new(Cursor::new(content)))
    } else {
        let file =
            File::open(path).with_context(|| format!("Failed to open FB2: {}", path.display()))?;

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

    // Helper that consumes the current name fields and appends an unique author.
    let append_current_author_unique =
        |last: &mut Option<String>,
         first: &mut Option<String>,
         middle: &mut Option<String>,
         authors: &mut Vec<Person>| {
            match (&last, &first, &middle) {
                // Skip authors with only middlename set
                (None, _, None) => {
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
                            let candidate = apply_aliases(&candidate, aliases);
                            if !authors.iter().any(|y| y.id == candidate.id) {
                                authors.push(candidate.clone());
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
                path_stack.push(e.name().as_ref().to_vec());

                let path_slice: Vec<&[u8]> = path_stack.iter().map(Vec::as_slice).collect();

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
                if let Some(t) = path_stack.pop() {
                    if t == b"description" {
                        break;
                    }
                }
            }

            Ok(Event::Text(e)) => {
                let text = e.decode()?; // Assume text is already trimmed by XML parser
                if text.is_empty() {
                    buf.clear();
                    continue;
                }

                let path_slice: Vec<&[u8]> = path_stack.iter().map(Vec::as_slice).collect();
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

            Ok(Event::Eof) => anyhow::bail!(format!(
                "No <description> found in FB2 file {}",
                path.display()
            )),
            Err(e) => return Err(e).context(format!("XML parse error in {}", path.display())),
            _ => {}
        }
        buf.clear();
    }

    let title = title.ok_or_else(|| anyhow::anyhow!("No <book-title> found in FB2 file"))?;

    // Check title is not empty after all non-alphanumeric characters removed
    if !&title.chars().any(char::is_alphanumeric) {
        bail!("Book title normalizes to empty: '{}'", &title);
    }

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
