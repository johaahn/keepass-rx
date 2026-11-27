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

## test_db_kpxc_full.kdbx

`test_db_kpxc_full.kdbx` is a KDBX 4.1 database that exercises many KeePass
elements: Meta custom data (`KPXC_BROWSER_*`, `KPXC_DECRYPTION_TIME_PREFERENCE`),
a custom icon on one entry and one group, an entry with three history items, a
binary attachment, custom string fields, entry and group custom data, tags,
auto-type with an association, a recycle bin, deleted objects, an expiry time
and nested groups. It is KDBX 4.1 because the source has a
`PreviousParentGroup` and a custom data item with a modification time
(KeePassXC writes 4.0 otherwise, which keepass-rs 0.17 cannot save). It is used to check what a save drops.

It was built from the hand-written `test_db_kpxc_full.xml` with KeePassXC 2.8.0:

    printf 'demopass\ndemopass\n' | keepassxc-cli import -p test_db_kpxc_full.xml test_db_kpxc_full.kdbx

Password: `demopass`.

`keepassxc-cli import` crashes (segfault) on `Protected="True"` values in plain
XML, so the source has none. Only the `Password` fields are protected, through
`ProtectPassword` in `Meta/MemoryProtection`. No custom string field is
protected. KeePassXC rewrote some values on import (tags separated by commas,
standard icon ids replaced by the custom icon, extra `_LAST_MODIFIED` custom
data), so the kdbx, not the XML, is the fixture.
