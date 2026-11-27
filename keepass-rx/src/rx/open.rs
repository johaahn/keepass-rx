use anyhow::Result;
use keepass::{Database, DatabaseKey};
use std::{fs, path::Path};
use zeroize::Zeroizing;

use super::{RxDatabase, RxDbFormat, RxSourceFile, ZeroableDatabase, save::sha256_of};
pub fn read_database(path: &Path, key: DatabaseKey) -> Result<(Database, RxSourceFile)> {
    let bytes = Zeroizing::new(fs::read(path)?);
    let sha256 = sha256_of(&bytes);
    let db = Database::parse(&bytes, key)?;
    let source = RxSourceFile {
        path: path.to_path_buf(),
        format: RxDbFormat::from_version(&db.config.version),
        sha256,
    };
    Ok((db, source))
}
pub fn build_rx_database(db: Database, source: RxSourceFile) -> Result<RxDatabase> {
    let rx_db = RxDatabase::new(Zeroizing::new(ZeroableDatabase(db)))?;
    Ok(rx_db.with_source(source))
}

pub fn open_database(path: &Path, key: DatabaseKey) -> Result<(RxDatabase, RxSourceFile)> {
    let (db, source) = read_database(path, key)?;
    let rx_db = build_rx_database(db, source.clone())?;
    Ok((rx_db, source))
}
