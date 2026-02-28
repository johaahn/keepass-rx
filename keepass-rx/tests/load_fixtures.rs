use std::{fs::File, path::PathBuf};

use keepass::{Database, DatabaseKey, config::DatabaseVersion};
use keepassrx::rx::{RxDatabase, ZeroableDatabase};
use keyring::set_default_credential_builder;
use zeroize::Zeroizing;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

struct FixtureCase {
    name: &'static str,
    password: &'static str,
    version: DatabaseVersion,
    root_name: &'static str,
    root_subgroups: usize,
    root_entries: usize,
    total_entries: usize,
}

#[test]
fn loads_supported_database_formats_into_rx_database() {
    set_default_credential_builder(keyring::mock::default_credential_builder());

    let cases = [
        FixtureCase {
            name: "test_db_kdb_with_password.kdb",
            password: "foobar",
            version: DatabaseVersion::KDB(2),
            root_name: "Root",
            root_subgroups: 3,
            root_entries: 0,
            total_entries: 5,
        },
        FixtureCase {
            name: "test_db_with_password.kdbx",
            password: "demopass",
            version: DatabaseVersion::KDB3(1),
            root_name: "sample",
            root_subgroups: 3,
            root_entries: 2,
            total_entries: 6,
        },
        FixtureCase {
            name: "test_db_kdbx4_with_password_argon2.kdbx",
            password: "demopass",
            version: DatabaseVersion::KDB4(0),
            root_name: "Root",
            root_subgroups: 0,
            root_entries: 2,
            total_entries: 2,
        },
        FixtureCase {
            name: "test_db_kdbx41_with_password_aes.kdbx",
            password: "demopass",
            version: DatabaseVersion::KDB4(1),
            root_name: "Database",
            root_subgroups: 2,
            root_entries: 4,
            total_entries: 4,
        },
    ];

    for case in cases {
        let mut file = File::open(fixture_path(case.name)).expect("open fixture");
        let db = Database::open(
            &mut file,
            DatabaseKey::new().with_password(case.password),
        )
        .expect("open keepass db");

        assert_eq!(db.config.version, case.version, "{}", case.name);

        let rx_db =
            RxDatabase::new(Zeroizing::new(ZeroableDatabase(db))).expect("load rx database");
        let root = rx_db.root_group();

        assert_eq!(root.name, case.root_name, "{}", case.name);
        assert_eq!(root.subgroups.len(), case.root_subgroups, "{}", case.name);
        assert_eq!(root.entries.len(), case.root_entries, "{}", case.name);
        assert_eq!(rx_db.all_entries_iter().count(), case.total_entries, "{}", case.name);
    }
}
