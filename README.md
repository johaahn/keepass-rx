# KeepassRX

<p align="center">
	<img src="./assets/banner.svg" width="600">
	<br/>
	<i>Password manager for Ubuntu Touch compatible with KeePass databases.</i>
</p>

[![Matrix Chat](https://img.shields.io/matrix/keepass-rx:agnos.is?label=matrix&server_fqdn=matrix.org)][matrix-room]

KeePass password manager app, aiming to be a complete local password
management solution for Ubuntu Touch. This is **alpha quality
software** with a defined development roadmap for new functionality
and features.

**Current Status:** Functional **readonly** opening of KeePass
databases, with additional features and numerous UI improvements
beyond the original app.

## Roadmap

The development of this app will follow this plan. These are
milestones, not version numbers.

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
 - [x] Support all app settings (that we want to support).
 - [ ] Key file support.

## 2.x - Enhanced Features

Add additional feaures beyond what the original app supported, but
still only supporting read-only database access.
 - [X] Securely zero out memory when closing database.
 - [X] Support Steam OTP codes.
 - [x] Clear clipboard after 30 seconds (partially implemented).
 - [x] "Easy lock" to re-open database without typing the whole password.
 - [ ] Improved password search UX (across groups).
 - [ ] Support opening attachments on entries.
 - [x] New UI layout to support groups
   - [x] Main page becomes the root group + entries list.
   - [x] Display entries under a group.
 - [x] Display custom fields and values.
 - [x] Improved database selection UX (multiple databases).
 - [x] Database accent color and name (from KeePassXC).
 - [x] Render built-in KeePass icons.

## 3.x - Writable Databases and Beyond

Implement updating and saving of databases, either using the
experimental kdbx4 save feature of keepass-rs, or via the KeepassXC
CLI.
 - [x] Support KeePassDX templates (i.e. show credit card entries as a
       credit card, etc).
 - [ ] YubiKey challenge-response? (requires support in OS)
 - [ ] Make a clear distinction between "imported" databases (from
       ContentHub) and "synced" databases (via external unconfined
       program).
 - [ ] Edit and Save Databases
   - [ ] Safely save databases.
   - [ ] Edit basic fields.
   - [ ] Create entries.
   - [ ] Delete entries.
   - [ ] Create groups.
   - [ ] Delete groups.
   - [ ] Move entries between groups.

## Development

Built on the work of the [original Keepass app][original] by David
Ventura.

## Translating

[KeePassRX can be translated on Hosted Weblate][translate]. The
localization platform of this project is sponsored by Hosted Weblate
via their free hosting plan for Libre and Open Source Projects.

## License

_Copyright (C) 2025 projectmoon_
_Portions copyright (C) 2021 David Ventura_
_Portions copyright (C) 2019-2025 Ruben De Smet, Markus Törnqvist_

This application is primarily licensed under the AGPLv3 license, but
some files are licensed under different terms. For full details, see
[COPYING][copying].

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.

[original]: https://github.com/DavidVentura/Keepass
[translate]: https://hosted.weblate.org/projects/ubports/keepassrx/
[matrix-room]: https://matrix.to/#/#keepass-rx:agnos.is
[copying]: ./COPYING
