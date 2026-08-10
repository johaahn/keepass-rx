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

// Because gui is an "optional feature," we should allow dead code and
// unused imports when building in debug mode without GUI enabled.
#![cfg_attr(debug_assertions, allow(dead_code, unused_imports))]

#[macro_use]
extern crate cstr;
#[macro_use]
extern crate qmetaobject;

use actix::Actor;
use anyhow::{Context, Result};
use cpp::cpp;
use gettextrs::{bindtextdomain, textdomain};
use log::{LevelFilter, error, info};
use qmeta_async::with_executor;
use qmetaobject::{QObjectBox, QString, QVariant, qml_register_type};
#[cfg(all(feature = "gui", not(feature = "sailfish")))]
use qmetaobject::{QQuickStyle, QQuickView, qml_register_enum};
use simplelog::{
    ColorChoice, CombinedLogger, ConfigBuilder, SharedLogger, TermLogger, TerminalMode,
    WriteLogger,
};
use std::env;
use std::fs::{File, OpenOptions, create_dir_all};
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::OnceLock;

pub use keepassrx::{crypto, license, rx};

#[cfg(feature = "gui")]
mod actor;
#[cfg(feature = "gui")]
mod app;
#[cfg(feature = "gui")]
mod gui;
#[cfg(feature = "gui")]
mod platform;
// The compiled-in QML resource bundle is only used by the Ubuntu Touch build.
// The Sailfish OS build loads QML from the installed filesystem tree.
#[cfg(all(feature = "gui", not(feature = "sailfish")))]
mod qrc;

#[cfg(feature = "gui")]
use crate::app::AppState;

const APP_ID: &str = "keepassrx.projectmoon";
static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

fn app_data_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_ID)
}

fn log_dir_path() -> PathBuf {
    app_data_path().join("logs")
}

fn append_panic_log(log_dir: &PathBuf, message: &str) {
    if create_dir_all(log_dir).is_err() {
        return;
    }

    let log_path = log_dir.join("panic.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "{message}");
    }
}

fn panic_payload_to_string(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

fn install_panic_hook(log_dir: PathBuf) {
    let _ = LOG_DIR.set(log_dir.clone());

    std::panic::set_hook(Box::new(move |panic_info| {
        let location = panic_info
            .location()
            .map(|loc| format!("{}:{}", loc.file(), loc.line()))
            .unwrap_or_else(|| "unknown location".to_string());
        let payload = panic_payload_to_string(panic_info.payload());
        let backtrace = std::backtrace::Backtrace::force_capture();
        let message = format!("panic at {location}: {payload}\n{backtrace}");

        error!("{message}");
        append_panic_log(&log_dir, &message);
    }));
}

fn init_logging() -> Result<()> {
    let log_dir = log_dir_path();
    create_dir_all(&log_dir).context("creating log directory")?;

    let config = ConfigBuilder::new().build();
    let file = File::create(log_dir.join("keepassrx.log")).context("creating keepassrx.log")?;

    let loggers: Vec<Box<dyn SharedLogger>> = vec![
        TermLogger::new(
            LevelFilter::Info,
            config.clone(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(LevelFilter::Debug, config, file),
    ];

    CombinedLogger::init(loggers).context("initializing logger")?;
    install_panic_hook(log_dir);

    Ok(())
}

#[cfg(feature = "gui")]
fn load_gui() -> Result<()> {
    use crate::gui::qml::{RxUiContainerStack, RxListItem, RxUiDatabase, RxUiEntry, RxUiLicenses};
    use crate::gui::utils::move_old_dirs_and_files;

    init_gettext();

    let uri = cstr!("keepassrx");

    qml_register_type::<RxUiDatabase>(uri, 1, 0, cstr!("RxUiDatabase"));
    qml_register_type::<RxUiContainerStack>(uri, 1, 0, cstr!("RxUiContainerStack"));
    qml_register_type::<RxUiEntry>(uri, 1, 0, cstr!("RxUiEntry"));
    qml_register_type::<RxUiLicenses>(uri, 1, 0, cstr!("RxUiLicenses"));
    qml_register_type::<RxListItem>(uri, 1, 0, cstr!("RxListItem"));

    #[cfg(not(feature = "sailfish"))]
    {
        use crate::gui::colors::RxColorType;
        use crate::gui::qml::RxItemType;
        use crate::gui::{RxDbType, RxGuiState, RxViewMode};
        use crate::rx::virtual_hierarchy::RxViewFeature;

        qml_register_enum::<RxItemType>(uri, 1, 0, cstr!("RxItemType"));
        qml_register_enum::<RxGuiState>(uri, 1, 0, cstr!("RxGuiState"));
        qml_register_enum::<RxViewMode>(uri, 1, 0, cstr!("RxViewMode"));
        qml_register_enum::<RxDbType>(uri, 1, 0, cstr!("RxDbType"));
        qml_register_enum::<RxViewFeature>(uri, 1, 0, cstr!("RxViewFeature"));
        qml_register_enum::<RxColorType>(uri, 1, 0, cstr!("RxColorType"));
    }

    if let Err(err) = move_old_dirs_and_files() {
        error!("Error during old app data migration: {}", err);
    }

    #[cfg(not(feature = "sailfish"))]
    boot_ubuntu_touch();

    #[cfg(feature = "sailfish")]
    boot_sailfish();

    Ok(())
}

/// Create the shared, platform-independent application objects: the
/// `SettingsBridge`, `AppState`, the `KeepassRx` GUI object, and the backing
/// actor. Must be called inside a `qmeta_async` executor context. Returns the
/// GUI object and the app holder; both must be kept alive for the lifetime of
/// the Qt event loop.
#[cfg(feature = "gui")]
fn create_app_objects() -> (
    Rc<QObjectBox<crate::gui::KeepassRx>>,
    Rc<crate::app::KeepassRxApp>,
) {
    use crate::app::KeepassRxApp;
    use crate::gui::KeepassRx;
    use crate::gui::actor::KeepassRxActor;
    use actix::Actor;
    use app::RxActors;
    use gui::settings::SettingsBridge;

    let settings_bridge = Rc::new(QObjectBox::new(SettingsBridge::default()));
    let app_state = Rc::new(QObjectBox::new(AppState::new(&settings_bridge)));
    let gui = Rc::new(QObjectBox::new(KeepassRx::new()));

    let global_app_actor = KeepassRxActor::new(&gui, &app_state).start();

    let app = Rc::new(KeepassRxApp {
        app_state,
        settings_bridge,
    });

    RxActors::set_app_actor(global_app_actor);
    (gui, app)
}

/// Ubuntu Touch / Lomiri bootstrap: Suru style + compiled-in QML resources
/// loaded through a bare `QQuickView`.
#[cfg(all(feature = "gui", not(feature = "sailfish")))]
fn boot_ubuntu_touch() {
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

    qmeta_async::run(|| {
        // We must return app here because it keeps the value alive
        // for the lifetime of qmeta_async::run. Without this, any
        // pointers inside app would be dropped and become null at
        // runtime.
        let (mut view, _app) = with_executor(|| -> Result<_> {
            let (gui, app) = create_app_objects();

            let mut view = QQuickView::new();
            let engine = view.engine();

            engine.set_property("keepassrx".into(), gui.pinned().into());
            engine.set_property(
                "keepassRxVersion".into(),
                QVariant::from(QString::from(env!("CARGO_PKG_VERSION"))),
            );
            engine.set_object_property("AppState".into(), app.app_state.pinned());
            engine.set_object_property("SettingsBridge".into(), app.settings_bridge.pinned());

            view.set_source("qrc:/qml/Main.qml".into());
            Ok((view, app))
        })
        .expect("app initialization failed");

        view.show();
        view.engine().exec();
    })
    .expect("running application");
}

/// Sailfish OS bootstrap: Silica app booted via sailo-rs' `QmlApp`
/// (QGuiApplication + QQuickView). QML is loaded from the installed filesystem
/// tree rather than a compiled-in resource bundle.
#[cfg(feature = "sailfish")]
fn boot_sailfish() {
    use crate::platform::QmlApp;

    qmeta_async::run(|| {
        // Keep `_app`/`_translator` alive for the event loop (see UT path).
        let (mut view, _app, _translator) = with_executor(|| -> Result<_> {
            use crate::gui::qml::RxTranslator;

            let (gui, app) = create_app_objects();
            let translator = Rc::new(QObjectBox::new(RxTranslator::default()));

            let mut view = QmlApp::application("harbour-keepassrx".into());
            view.set_title("KeePassRX".into());
            view.set_application_version(QString::from(env!("CARGO_PKG_VERSION")));
            let _ = view.install_default_translator();

            view.set_object_property("keepassrx".into(), gui.pinned());
            view.set_property(
                "keepassRxVersion".into(),
                QVariant::from(QString::from(env!("CARGO_PKG_VERSION"))),
            );
            view.set_object_property("AppState".into(), app.app_state.pinned());
            view.set_object_property("SettingsBridge".into(), app.settings_bridge.pinned());
            view.set_object_property("Tr".into(), translator.pinned());

            view.set_source(QmlApp::path_to("qml/harbour-keepassrx.qml".into()));
            Ok((view, app, translator))
        })
        .expect("app initialization failed");

        view.show_full_screen();
        view.exec();
    })
    .expect("running application");
}

#[cfg(feature = "gui")]
fn init_gettext() {
    let domain = "keepassrx.projectmoon";
    textdomain(domain).expect("Failed to set gettext domain");

    // On Sailfish OS the .mo catalogs are installed to a fixed system prefix,
    // and the process working directory is not the app install dir.
    #[cfg(feature = "sailfish")]
    let path = PathBuf::from("/usr/share/locale");

    #[cfg(not(feature = "sailfish"))]
    let path = {
        let mut app_dir_path =
            env::current_dir().expect("Failed to get the app working directory");
        if !app_dir_path.is_absolute() {
            app_dir_path = PathBuf::from("/usr");
        }
        app_dir_path.join("share/locale")
    };

    bindtextdomain(domain, path.to_str().unwrap()).expect("Failed to bind gettext domain");
}

fn main() -> Result<()> {
    if let Err(err) = init_logging() {
        eprintln!("keepassrx: failed to initialize file logging: {err}");
    }

    libsodium_rs::ensure_init()?;

    match () {
        #[cfg(feature = "gui")]
        () => load_gui()?,

        #[cfg(not(feature = "gui"))]
        () => info!("GUI not enabled."),
    }

    Ok(())
}
