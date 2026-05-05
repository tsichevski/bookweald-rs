use crate::{book, db, fb2_parse, person::Person};
use ahash::HashMap;
use anyhow::Result;
use crossbeam_channel as cb;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use sqlx::PgPool;
use std::path::PathBuf;

pub async fn index(
    inputs: &[PathBuf],
    aliases: &Option<HashMap<String, Person>>,
    overwrite: bool,
    pool: &PgPool,
) -> Result<()> {
    let existing_md5s: std::collections::HashSet<[u8; 16]> = db::load_existing_md5s(pool).await?;

    let (tx, rx) = cb::bounded(8000);

    // Parallel producer (blocking section)
    std::thread::scope(|s| {
        let tx_clone = tx.clone();
        s.spawn(move || {
            inputs.par_iter().for_each(|path| {
                let book: book::Book = match fb2_parse::parse_book_info(path, aliases) {
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

                let _ = tx_clone.send((book, md5));
            });
        });
    });
    drop(tx);

    // Consumer: batched DB inserts (async)

    for (book, _md5) in rx {
        tracing::debug!("Received {}", book.title);
    }

    Ok(())
}
