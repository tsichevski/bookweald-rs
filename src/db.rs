use anyhow::{Context, Result};
use futures_util::TryStreamExt;
use sqlx::PgPool;
use sqlx::Row;
use std::collections::HashSet;
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
    created_at  TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    CHECK (octet_length(md5_hash) = 16)
)
            "#,
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
    let mut md5s = HashSet::with_capacity(500_000); // pre-allocate for 1M scale

    let mut rows = sqlx::query("SELECT md5_hash FROM books").fetch(pool);

    while let Some(row) = rows.try_next().await? {
        let md5_bytes: Vec<u8> = row.try_get("md5")?;
        if md5_bytes.len() == 16 {
            let mut arr = [0u8; 16];
            arr.copy_from_slice(&md5_bytes);
            md5s.insert(arr);
        }
    }

    info!("Loaded {} existing MD5 fingerprints from DB", md5s.len());
    Ok(md5s)
}

// /// Find or insert person (UPSERT, like OCaml)
// pub fn find_or_insert_person(person: &Person) -> Result<String, sqlx::Error> {  // returns DB id as string
//     let pool = DB_POOL.get().ok_or(...) ?;
//     let rt = tokio::runtime::Runtime::new()?;

//     rt.block_on(async {
//         let row = sqlx::query!(
//             r#"INSERT INTO persons (normalized_name)
//                VALUES ($1)
//                ON CONFLICT (normalized_name) DO UPDATE SET normalized_name = EXCLUDED.normalized_name
//                RETURNING id::text"#,
//             person.normalized_name
//         ).fetch_one(pool).await?;
//         Ok(row.id)
//     })
// }

// /// Find or insert book + link authors (core operation from OCaml)
// pub fn find_or_insert_book(book: &Book) -> Result<String, sqlx::Error> {
//     let pool = DB_POOL.get().ok_or(...) ?;
//     let rt = tokio::runtime::Runtime::new()?;

//     rt.block_on(async {
//         let mut tx = pool.begin().await?;

//         // Insert book if not exists by md5_hash
//         let book_row = sqlx::query!(
//             r#"INSERT INTO books (md5_hash, title /* + other fields */)
//                VALUES ($1, $2 /* + values */)
//                ON CONFLICT (md5_hash) DO UPDATE SET md5_hash = EXCLUDED.md5_hash
//                RETURNING id::text"#,
//             book.md5_hash, book.title /* ... */
//         ).fetch_one(&mut *tx).await?;

//         let book_id = book_row.id;

//         // Link authors (find_or_insert each + insert links)
//         for author in &book.authors {  // assume Book has authors: Vec<Person>
//             let person_id = find_or_insert_person(author)?;  // recursive call ok inside tx if adjusted
//             sqlx::query!(
//                 "INSERT INTO book_authors (book_id, person_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
//                 book_id.parse::<i32>()?, person_id.parse::<i32>()?
//             ).execute(&mut *tx).await?;
//         }

//         tx.commit().await?;
//         Ok(book_id)
//     })
// }

// /// Delete book (by md5_hash)
// pub fn delete_book(book: &Book) -> Result<String, sqlx::Error> {
//     // similar blocking + query pattern
//     // ...
//     todo!("Implement delete matching OCaml")
// }

// Add close, drop_schema, etc. as needed in the same file

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

    #[test]
    fn test_connect_invalid() {
        let result = connect("localhost", 5432, "baduser", "wrongpass", "nonexistent");
        assert!(result.is_err());
    }
}
