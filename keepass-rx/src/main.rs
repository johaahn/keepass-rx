/*
 * Copyright (C) 2025 projectmoon
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; version 3.
 *
 * keepassrx is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

// Because gui is an "optional feature," we should allow dead code and
// unused imports when building in debug mode without GUI enabled.
#![cfg_attr(debug_assertions, allow(dead_code, unused_imports))]

#[macro_use]
extern crate cstr;
#[macro_use]
extern crate qmetaobject;

extern crate libsodium_rs;

use actix::Actor;
use anyhow::Result;
use app::KeepassRxApp;
use cpp::cpp;
use gettextrs::{bindtextdomain, textdomain};
use qmeta_async::with_executor;
use qmetaobject::{QObjectBox, QQuickStyle, QQuickView, qml_register_enum, qml_register_type};
use std::env;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

mod actor;
mod crypto;
mod rx;

#[cfg(feature = "gui")]
mod app;
#[cfg(feature = "gui")]
mod gui;
#[cfg(feature = "gui")]
mod qrc;

#[cfg(feature = "gui")]
use crate::app::AppState;

#[cfg(feature = "gui")]
use crate::gui::{
    KeepassRx, RxGuiState,
    actor::KeepassRxActor,
    models::{RxItemType, RxListItem},
    utils::{app_data_path, imported_databases_path, move_old_db},
};

#[cfg(feature = "gui")]
fn load_gui() -> Result<()> {
    use gui::{RxViewMode, colors::RxColorType, models::RxUiFeature, qml::RxUiEntry};

    init_gettext();

    unsafe {
        cpp! {{
            #include <QtCore/QCoreApplication>
            #include <QtCore/QString>
        }}
        cpp! {[]{
            QCoreApplication::setApplicationName(QStringLiteral("keepassrx.projectmoon"));
            QCoreApplication::setOrganizationName(QStringLiteral("keepassrx.projectmoon"));
            QCoreApplication::setOrganizationDomain(QStringLiteral("keepassrx.projectmoon"));
        }}
    }

    QQuickStyle::set_style("Suru");
    qrc::load();
    let uri = cstr!("keepassrx");
    qml_register_type::<RxUiEntry>(uri, 1, 0, cstr!("RxUiEntry"));
    qml_register_type::<RxListItem>(uri, 1, 0, cstr!("RxListItem"));
    qml_register_enum::<RxItemType>(uri, 1, 0, cstr!("RxItemType"));
    qml_register_enum::<RxGuiState>(uri, 1, 0, cstr!("RxGuiState"));
    qml_register_enum::<RxViewMode>(uri, 1, 0, cstr!("RxViewMode"));
    qml_register_enum::<RxUiFeature>(uri, 1, 0, cstr!("RxUiFeature"));
    qml_register_enum::<RxColorType>(uri, 1, 0, cstr!("RxColorType"));

    // Load last db
    // TODO this is a hack and should be more properly done with QT
    // settings.
    let last_db_file = app_data_path().join("last-db");
    let last_db = match last_db_file {
        file if file.exists() => Some(std::fs::read_to_string(file)?),
        _ => None,
    };

    // Check to make sure last DB actually exists.
    let last_db = match last_db {
        Some(db) if imported_databases_path().join(db.clone()).exists() => Some(db),
        _ => None,
    };

    // "Data migration": Move any db.kdbx from the data directory to imported.
    move_old_db();

    qmeta_async::run(|| {
        // We must return app here because it keeps the value alive
        // for the lifetime of qmeta_async::run. Without this, any
        // pointers inside app would be dropped and become at runtime.
        let (mut view, _app) = with_executor(|| -> Result<_> {
            let app_state = Arc::new(QObjectBox::new(AppState::new()));
            let gui = Arc::new(QObjectBox::new(KeepassRx::new(last_db)));

            let gui_actor = KeepassRxActor::new(&gui, &app_state).start();

            let app = Rc::new(KeepassRxApp {
                app_state: app_state.clone(),
                gui_actor: gui_actor,
            });

            let mut view = QQuickView::new();
            let engine = view.engine();

            engine.set_property("keepassrx".into(), gui.pinned().into());
            engine.set_object_property("AppState".into(), app.app_state.pinned());

            view.set_source("qrc:/qml/Main.qml".into());
            Ok((view, app))
        })
        .expect("app initialization failed");

        view.show();
        view.engine().exec();
    })
    .expect("running application");

    Ok(())
}

#[cfg(feature = "gui")]
fn init_gettext() {
    let domain = "keepassrx.projectmoon";
    textdomain(domain).expect("Failed to set gettext domain");

    let mut app_dir_path =
        env::current_dir().expect("Failed to get the app working directory");
    if !app_dir_path.is_absolute() {
        app_dir_path = PathBuf::from("/usr");
    }

    let path = app_dir_path.join("share/locale");

    bindtextdomain(domain, path.to_str().unwrap()).expect("Failed to bind gettext domain");
}

fn main() -> Result<()> {
    libsodium_rs::ensure_init()?;

    match () {
        #[cfg(feature = "gui")]
        () => load_gui()?,

        #[cfg(not(feature = "gui"))]
        () => println!("GUI not enabled."),
    }

    Ok(())
}
