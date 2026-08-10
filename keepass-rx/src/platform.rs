/*
 * Copyright (C) 2025 projectmoon
 *
 * This program is free software: you can redistribute it and/or
 * modify it under the terms of the GNU Affero General Public License
 * as published by the Free Software Foundation; version 3.
 *
 * keepassrx is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

//! Sailfish OS application bootstrap abstraction.
//!
//! On the SFOS build (`sailfish` feature) this re-exports the `QmlApp` wrapper
//! from `sailo-rs` (the Rust equivalent of C++ `SailfishApp`: a
//! `QGuiApplication` + `QQuickView` set up with the Silica theme). On any other
//! build it provides a no-op stub with the same surface that panics if actually
//! run, so the crate still type-checks without the `sailfish` feature. This
//! mirrors Whisperfish's `platform.rs`.

#[cfg(feature = "sailfish")]
mod sailfish_inner {
    pub use sailors::sailfishapp::{QQmlEngine, QmlApp};
}

#[cfg(not(feature = "sailfish"))]
mod sailfish_inner {
    use qmetaobject::qttypes::{QString, QVariant};

    // No-op implementation of QmlApp used when the `sailfish` feature is off.
    // It mirrors the real API surface but panics on exec()/engine(). The
    // Ubuntu Touch build never touches this (it boots via QQuickView directly).

    pub struct QmlApp;
    pub struct QQmlEngine;

    impl QmlApp {
        pub fn application(_app: String) -> Self {
            QmlApp
        }

        pub fn path_to(path: String) -> String {
            path
        }

        pub fn set_property(&mut self, _name: QString, _value: QVariant) {}
        pub fn set_object_property<T>(&mut self, _name: QString, _item: T) {}
        pub fn set_title(&mut self, _title: QString) {}
        pub fn set_application_version(&mut self, _version: QString) {}
        pub fn install_default_translator(&mut self) -> Option<()> {
            Some(())
        }
        pub fn set_quit_on_last_window_closed(&mut self, _quit: bool) {}
        pub fn promote_gui_app_to_qml_context(&mut self, _name: QString) {}
        pub fn set_source(&mut self, _source: String) {}
        pub fn show_full_screen(&mut self) {}
        pub fn exec(self) {
            panic!(
                "keepassrx was compiled without the `sailfish` feature. \
                 The Sailfish OS bootstrap is unavailable."
            );
        }
        pub fn engine(&mut self) -> &mut QQmlEngine {
            panic!(
                "keepassrx was compiled without the `sailfish` feature. \
                 The Sailfish OS bootstrap is unavailable."
            );
        }
    }
}

pub use self::sailfish_inner::*;
