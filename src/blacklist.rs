use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// [append file path comment] appends a line [basename(path)|comment] to the blacklist [file].
/// Creates parent directories if needed. Follows blacklist.rst format.
///
/// FIXME: change API: lines will be appended in batch after verification is complete.
pub fn append(file: &Path, path: &Path, comment: &str) -> Result<()> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Cannot create directory: {}", parent.display()))?;
    }
    let mut ch: File = OpenOptions::new().append(true).create(true).open(file)?;

    let basename = path.file_name().with_context(|| "Invalid path {path}")?;
    writeln!(ch, "{:?}|{comment}", basename)?;
    Ok(())
}

pub fn load(file: &Path) -> Result<HashMap<String, String>> {
    //   if Sys.file_exists path then
    if file.exists() {
        let mut table: HashMap<String, String> = HashMap::with_capacity(512);

        let input_file = File::open(file)?;
        let mut reader = BufReader::new(input_file);

        let mut line = String::new();

        while reader.read_line(&mut line)? != 0 {
            if line.starts_with("#") {
                continue;
            }

            let chunks: Vec<&str> = line.split('|').collect();
            match &chunks[..] {
                [file, msg, ..] => {
                    table.insert(file.to_string(), msg.to_string());
                }
                _ => anyhow::bail!("Invalid blacklist line: {}", line),
            }

            line.clear(); // reuse buffer
        }
        Ok(table)
    } else {
        anyhow::bail!("File does not exist: {}", file.display())
    }
}
