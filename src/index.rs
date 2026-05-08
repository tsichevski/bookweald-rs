use crate::{book, db, fb2_parse, person::Person};
use ahash::{HashMap, HashMapExt, HashSet};
use anyhow::Result;
use crossbeam_channel as cb;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use sqlx::PgPool;
use std::path::PathBuf;

const BATCH_SIZE: usize = 1000;

pub async fn index(
    inputs: Vec<PathBuf>,
    aliases: &Option<HashMap<String, Person>>,
    overwrite: bool,
    pool: &PgPool,
) -> Result<()> {
    let existing_md5s: HashSet<[u8; 16]> = db::load_existing_md5s(pool).await?;

    let (tx, rx) = cb::bounded(8192);

    let tx_clone = tx.clone();
    let aliases_clone = aliases.clone();
    tokio::task::spawn_blocking(move || {
        inputs.par_iter().for_each(|path| {
            let book: book::Book = match fb2_parse::parse_book_info(path, &aliases_clone) {
                Ok(book) => book,
                Err(e) => {
                    tracing::warn!(?path, "parse failed: {}", e);
                    return;
                }
            };

            let md5 = book::book_digest(&book);

            if !overwrite && existing_md5s.contains(&md5) {
                return;
            }

            if let Err(e) = tx_clone.send((book, md5)) {
                tracing::error!("Channel send failed: {}.", e);
            }
        });
    });
    drop(tx);

    // md5/filename dictionary for all newly loaded books. Used to detect duplicates
    let mut new_md5s: HashMap<[u8; 16], String> = HashMap::new();

    // Collect some books to add them in batch
    let mut batch = Vec::with_capacity(BATCH_SIZE);
    for (book, md5) in rx {
        if new_md5s.contains_key(&md5) {
            tracing::warn!(
                "Ignoring a book with duplicate md5 sum: '{}', the previously added book with same md5 sum '{}' ",
                &book.filename,
                &new_md5s.get(&md5).expect("This must never happen!")
            );
            continue;
        }
        new_md5s.insert(md5.clone(), book.filename.clone());
        batch.push((book, md5));
        if batch.len() >= BATCH_SIZE {
            db::insert_batch(pool, &batch).await?;
            batch.clear();
            tracing::debug!("Committed batch of {} books", BATCH_SIZE);
        }
    }

    if !batch.is_empty() {
        db::insert_batch(pool, &batch).await?;
    }

    Ok(())
}
