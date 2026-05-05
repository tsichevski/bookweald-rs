use crate::{fb2_parse, person::Person};
use ahash::HashMap;
use anyhow::Result;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use sqlx::PgPool;
use std::path::PathBuf;

pub async fn index(
    inputs: &[PathBuf],
    aliases: &Option<HashMap<String, Person>>,
    _overwrite: bool,
    _pool: &PgPool,
) -> Result<()> {
    // let existing_md5s: std::collections::HashSet<[u8; 16]> = db::load_existing_md5s(pool).await?;
    let _books: Vec<_> = inputs
        .par_iter()
        .map(|path| fb2_parse::parse_book_info(path, aliases))
        .collect();

    Ok(())
}
