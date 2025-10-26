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

#[macro_use]
extern crate cstr;
#[macro_use]
extern crate qmetaobject;

use actix::Actor;
use anyhow::Result;
use app::KeepassRxApp;
use cpp::cpp;
use gettextrs::{bindtextdomain, textdomain};
use gui::{KeepassRxActor, imported_databases_path, move_old_db};
use qmeta_async::with_executor;
use qmetaobject::{QObjectBox, QQuickStyle, QQuickView, qml_register_type};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use crate::gui::{KeepassRx, app_data_path};

mod app;
mod gui;
mod qrc;
mod rx;

fn main() -> Result<()> {
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
    qml_register_type::<KeepassRx>(cstr!("KeepassRx"), 1, 0, cstr!("KeepassRx"));

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
        let mut view = with_executor(|| -> Result<QQuickView> {
            // TODO store these in some global state?

            let keepassrx = Arc::new(QObjectBox::new(KeepassRx::new(last_db)));
            let keepassrx_actor = KeepassRxActor::new(&keepassrx).start();

            let app = KeepassRxApp {
                gui: keepassrx,
                gui_actor: keepassrx_actor,
            };

            let mut view = QQuickView::new();

            let engine = view.engine();
            engine.set_property("keepassrx".into(), app.gui.pinned().into());
            view.set_source("qrc:/qml/Main.qml".into());
            Ok(view)
        })
        .expect("app initialization failed");

        view.show();
        view.engine().exec();
    })
    .expect("running application");

    Ok(())
}

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
