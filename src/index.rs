use crate::{fb2_parse, person::Person};
use ahash::HashMap;
use anyhow::Result;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::path::PathBuf;

pub fn index(
    inputs: &[PathBuf],
    aliases: &Option<HashMap<String, Person>>,
    _overwrite: bool,
) -> Result<()> {
    let _books: Vec<_> = inputs
        .par_iter()
        .map(|path| fb2_parse::parse_book_info(path, aliases))
        .collect();
    Ok(())
}
