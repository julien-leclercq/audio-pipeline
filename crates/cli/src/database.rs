#![allow(dead_code)]
#![allow(unused_imports)]

use anyhow::Context;
use rusqlite::{self, OpenFlags};

pub use self::migrations::{create_migration_table, run_migrations};

// pub fn init(path: String) -> anyhow::Result<()> {
//     let conn = rusqlite::Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_CREATE)
//         .context("could not open sqlite connection")?;
//     create_migration_table(&conn)?;
//     run_migrations(&conn)?;
//     Ok(())
// }

pub(crate) mod migrations {
    // use once_cell::sync::Lazy;
    use rusqlite::Connection;

    // pub static MIGRATIONS: Lazy<Vec<&str>> = Lazy::new(|| Vec::new());

    pub fn create_migration_table(_conn: &Connection) -> anyhow::Result<()> {
        todo!("cannot init database yet!")
    }

    pub fn run_migrations(_conn: &Connection) -> anyhow::Result<()> {
        todo!("cannot run migration yet!")
    }
}
