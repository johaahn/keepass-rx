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
use cpp::cpp;
use gettextrs::{bindtextdomain, textdomain};
use gui::KeepassRxActor;
use qmeta_async::with_executor;
use qmetaobject::{QObjectBox, QQuickStyle, QmlEngine, qml_register_type};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use crate::gui::KeepassRX;

mod gui;
mod qrc;
mod rx;

fn main() {
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
    qml_register_type::<KeepassRX>(cstr!("KeepassRx"), 1, 0, cstr!("KeepassRx"));

    qmeta_async::run(|| {
        let engine = with_executor(|| -> Result<QmlEngine> {
            // TODO store these in some global state?
            let keepassrx = Arc::new(QObjectBox::new(KeepassRX::default()));
            let _keepassrx_actor = KeepassRxActor::new(&keepassrx).start();
            let mut engine = QmlEngine::new();
            engine.set_property("keepassrx".into(), keepassrx.pinned().into());
            engine.load_file("qrc:/qml/Main.qml".into());
            Ok(engine)
        })
        .expect("app initialization failed");

        engine.exec();
    })
    .expect("running application");
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
