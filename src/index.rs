use crate::person::Person;
use ahash::HashMap;
use anyhow::Result;
use std::path::PathBuf;

pub fn index(
    inputs: &[PathBuf],
    aliases: Option<HashMap<String, Person>>,
    overwrite: bool,
) -> Result<()> {
    Ok(())
}
