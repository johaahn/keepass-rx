# KeepassRX

KeePass password manager app, aiming to be a complete local password
management solution for Ubuntu Touch. This is **alpha quality
software** with a defined development roadmap for new functionality
and features.

**Current Status:** Functional readonly opening of KeePass databases.

## Roadmap

The development of this app will follow this plan.

## 0.x - Minimal Viable Product

An app that is rough around the edges but ready for daily use.
 - [X] Open databases.
 - [X] Copy usernames, passwords, and TOTP codes.
 - [X] Updated for 24.04 Noble.
 - [ ] App settings working and persisted.
 - [ ] Key file support (might already work).
 - [ ] Close database after period of time.
 - [ ] Opening database does not lock up main GUI thread.

## 1.x - Functional Parity

Achieve functional **readonly** parity with the original Keepass app
for Ubuntu Touch, plus a few extras.
 - [X] Support Steam OTP codes.
 - [ ] Display icons for password entries.
 - [ ] "Easy lock" to re-open database without typing the whole password.
 - [ ] Improved password search UX (across groups).

## 2.x - Writable Databases

Implement updating and saving of databases, either using the
experimental kdbx4 save features of keepass-rs, or via the KeepassXC
CLI.

## Development

Built on the work of the [original Keepass app][original] by David
Ventura.

## License

Copyright (C) 2025 projectmoon, David Ventura

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License version 3, as published by the
Free Software Foundation.

This program is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranties of MERCHANTABILITY, SATISFACTORY
QUALITY, or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
for more details.

You should have received a copy of the GNU General Public License along with
this program. If not, see <http://www.gnu.org/licenses/>.

[original]: https://github.com/DavidVentura/Keepass
