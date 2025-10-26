# KeepassRX

<p align="center">
	<img src="./assets/banner.svg" width="600">
	<br/>
	<i>Password manager for Ubuntu Touch compatible with KeePass databases.</i>
</p>

KeePass password manager app, aiming to be a complete local password
management solution for Ubuntu Touch. This is **alpha quality
software** with a defined development roadmap for new functionality
and features.

**Current Status:** Functional **readonly** opening of KeePass
databases, with additional features beyond the original app.

## Roadmap

The development of this app will follow this plan.

## 0.x - Minimal Viable Product

An app that is rough around the edges but ready for daily use.
 - [X] Open databases.
 - [X] Copy usernames, passwords, and TOTP codes.
 - [X] Updated for 24.04 Noble.
 - [X] App settings and previous database persisted.
 - [X] Opening database does not lock up main GUI thread.
 - [X] Load all groups and entries.

## 1.x - Functional Parity

Achieve functional **readonly** parity with the original Keepass app
for Ubuntu Touch, plus a few extras.
 - [X] Display icons for password entries.
 - [ ] Support all app settings (that we want to support).
 - [ ] Key file support.
 - [ ] Close database after period of time.

## 2.x - Enhanced Features

Add additional feaures beyond what the original app supported, but
still only supporting read-only database access.
 - [X] Securely zero out memory when closing database.
 - [X] Support Steam OTP codes.
 - [x] Clear clipboard after 30 seconds (partially implemented).
 - [x] "Easy lock" to re-open database without typing the whole password.
 - [ ] Improved password search UX (across groups).
 - [ ] New UI layout to support nested groups.
 - [ ] Display custom fields and values.
 - [ ] Improved database selection UX.
   - [X] Support adding multiple databases.
   - [ ] Make a clear distinction between "imported" databases (from
         ContentHub) and "synced" databases (via external unconfined
         program).

## 3.x - Writable Databases

Implement updating and saving of databases, either using the
experimental kdbx4 save feature of keepass-rs, or via the KeepassXC
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
