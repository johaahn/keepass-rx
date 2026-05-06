Fixtures used by `keepass-rx` load tests.

Source provenance:
- `test_db_kdb_with_password.kdb`
- `test_db_with_password.kdbx`
- `test_db_kdbx4_with_password_argon2.kdbx`
- `test_db_kdbx41_with_password_aes.kdbx`

These four files were copied from the locally cached `keepass-rs` test resources at commit `0e924b43b81878fe310d9c8dd4a7b1779ebadef5`.

Passwords:
- `test_db_kdb_with_password.kdb`: `foobar`
- `test_db_with_password.kdbx`: `demopass`
- `test_db_kdbx4_with_password_argon2.kdbx`: `demopass`
- `test_db_kdbx41_with_password_aes.kdbx`: `demopass`
- `test_saved_searches.kdbx`: `somePassw0rd`

`test_saved_searches.kdbx` is the existing project fixture that exercises saved-search loading.
