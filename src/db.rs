use crate::book::Book;
use ahash::{HashSet, HashSetExt};
use anyhow::{Context, Result};
use futures_util::TryStreamExt;
use sqlx::PgPool;
use sqlx::Row;
use std::sync::OnceLock;
use tracing::*;

pub type DbPool = sqlx::PgPool;

static DB_POOL: OnceLock<PgPool> = OnceLock::new(); // Application-managed R/W pool

pub async fn connect_async(
    host: &str,
    port: u16,
    user: &str,
    password: &str,
    dbname: &str,
) -> Result<PgPool> {
    let url = format!(
        "postgres://{}:{}@{}:{}/{}",
        user, password, host, port, dbname
    );
    let pool = PgPool::connect(&url)
        .await
        .context("Failed to connect (R/W user)")?;
    info!("DB pool created");
    Ok(pool)
}

/// Application stores the returned pool
pub fn store_pool(pool: PgPool) {
    DB_POOL.set(pool).expect("Pool already stored");
}

/// Schema init receives admin pool as parameter (elevated DDL user)
pub async fn init_schema(admin_pool: &PgPool, overwrite: bool) -> Result<(), sqlx::Error> {
    let mut tx = admin_pool.begin().await?;

    if overwrite {
        // Always replace
        sqlx::query("DROP TABLE IF EXISTS book_authors CASCADE")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DROP TABLE IF EXISTS books CASCADE")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DROP TABLE IF EXISTS persons CASCADE")
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query(
        r#"
CREATE TABLE books (
    id          SERIAL PRIMARY KEY,
    md5_hash    BYTEA NOT NULL UNIQUE,
    ext_id      TEXT,
    version     TEXT,
    title       TEXT NOT NULL,
    encoding    TEXT NOT NULL,
    lang        TEXT,
    genre       TEXT,
    filename    TEXT NOT NULL,
    CHECK (octet_length(md5_hash) = 16)
)"#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query("ALTER TABLE books OWNER TO books")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        r#"
            CREATE TABLE persons (
                id SERIAL PRIMARY KEY,
                normalized_name TEXT NOT NULL UNIQUE
            )
            "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query("ALTER TABLE persons OWNER TO books")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        r#"
            CREATE TABLE book_authors (
                book_id INTEGER REFERENCES books(id) ON DELETE CASCADE,
                person_id INTEGER REFERENCES persons(id) ON DELETE CASCADE,
                PRIMARY KEY (book_id, person_id)
            )
            "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query("ALTER TABLE book_authors OWNER TO books")
        .execute(&mut *tx)
        .await?;

    sqlx::query("CREATE INDEX idx_books_md5_hash ON books(md5_hash)")
        .execute(&mut *tx)
        .await?;
    sqlx::query("CREATE INDEX idx_persons_norm ON persons(normalized_name)")
        .execute(&mut *tx)
        .await?;

    tx.commit().await
}

/// Load all existing canonical MD5 digests from the books table.
/// Used for fast incremental indexing.
pub async fn load_existing_md5s(pool: &DbPool) -> Result<HashSet<[u8; 16]>> {
    let mut md5s = HashSet::new();

    let mut rows = sqlx::query("SELECT md5_hash FROM books").fetch(pool);

    while let Some(row) = rows.try_next().await? {
        let md5_bytes: Vec<u8> = row.try_get("md5_hash")?;
        if md5_bytes.len() == 16 {
            // FIXME: can I consume md5_bytes instead of making copy?
            let mut arr = [0u8; 16];
            arr.copy_from_slice(&md5_bytes);
            md5s.insert(arr);
        }
    }

    info!("Loaded {} existing MD5 fingerprints from DB", md5s.len());
    Ok(md5s)
}

pub async fn insert_batch(pool: &PgPool, batch: &[(Book, [u8; 16])]) -> Result<()> {
    let n = batch.len();
    let mut md5_hashs: Vec<&[u8]> = Vec::with_capacity(n);
    let mut ext_ids: Vec<Option<&str>> = Vec::with_capacity(n);
    let mut versions: Vec<Option<&str>> = Vec::with_capacity(n);
    let mut titles: Vec<&str> = Vec::with_capacity(n);
    let mut langs: Vec<Option<&str>> = Vec::with_capacity(n);
    let mut genres: Vec<Option<&str>> = Vec::with_capacity(n);
    let mut filenames: Vec<&str> = Vec::with_capacity(n);
    let mut encodings: Vec<&str> = Vec::with_capacity(n);

    for (b, md5) in batch {
        md5_hashs.push(md5.as_slice());
        ext_ids.push(b.ext_id.as_deref());
        versions.push(b.version.as_deref());
        titles.push(&b.title);
        langs.push(b.lang.as_deref());
        genres.push(b.genre.as_deref());
        filenames.push(&b.filename);
        encodings.push(&b.encoding);
    }

    sqlx::query(
        "INSERT INTO books (md5_hash, ext_id, version, title, lang, genre, filename, encoding)
         SELECT * FROM UNNEST($1::bytea[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[], $7::text[], $8::text[])"
    )
    .bind(md5_hashs)
    .bind(ext_ids)
    .bind(versions)
    .bind(titles)
    .bind(langs)
    .bind(genres)
    .bind(filenames)
    .bind(encodings)
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connect_store_and_init_schema() {
        let admin_pool = connect_async("localhost", 5432, "admin", "admin", "books")
            .await
            .expect("Failed to connect as admin");

        init_schema(&admin_pool, true)
            .await
            .expect("Failed to replace schema");

        // // Connect using async version inside the test
        let rw_pool = connect_async("localhost", 5432, "books", "books", "books")
            .await
            .expect("Failed to connect R/W user");
        store_pool(rw_pool);

        // Verify
        let pool = DB_POOL.get().expect("R/W pool should be stored");

        let books_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books")
            .fetch_one(pool)
            .await
            .expect("Query books failed");

        let persons_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM persons")
            .fetch_one(pool)
            .await
            .expect("Query persons failed");

        assert_eq!(books_count, 0);
        assert_eq!(persons_count, 0);

        println!("✅ connect / store_pool / init_schema test passed");
    }

    #[tokio::test]
    async fn test_connect_invalid() {
        let result = connect_async("localhost", 5432, "baduser", "wrongpass", "nonexistent").await;
        assert!(result.is_err());
    }
}
