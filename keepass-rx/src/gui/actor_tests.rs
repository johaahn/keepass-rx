use actix::prelude::*;
use keepass::DatabaseKey;
use keepass::db::EntryId;
use qmeta_async::with_executor;
use qmetaobject::{QObjectBox, QString, QmlEngine};
use secstr::SecUtf8;
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::actor::{
    CheckLockingStatus, EncryptMasterPassword, KeepassRxActor, OpenDatabase, SaveCredential,
    SaveEntry, StoreMasterPassword,
};
use super::settings::SettingsBridge;
use super::utils::imported_databases_path;
use super::{KeepassRx, RxDbType};
use crate::app::AppState;
use crate::rx::open::read_database;
use crate::rx::{RxEntryEdit, RxSourceFile};

const FIXTURE: &str = "test_db_kdbx4_with_password_argon2.kdbx";
const PASSWORD: &str = "demopass";
const SHORT_CODE: &str = "demop";
const NEW_PASSWORD: &str = "changed-by-the-actor-test";
const STEP_TIMEOUT: Duration = Duration::from_secs(60);

type Events = Rc<RefCell<Vec<String>>>;

struct Observed {
    rejected_events: Vec<String>,
    rejected_bytes_unchanged: bool,
    saved_events: Vec<String>,
    lockout_events: Vec<String>,
    saved_password: Option<String>,
    backup_is_original: bool,
    gui_saving_after: bool,
}

fn record(events: &Events, name: &str) -> impl FnMut(String) + use<> {
    let events = events.clone();
    let name = name.to_string();
    move |detail| events.borrow_mut().push(format!("{name}{detail}"))
}

fn connect_signals(gui: &Rc<QObjectBox<KeepassRx>>, events: &Events) {
    use qmetaobject::connections::connect;

    let pinned = gui.pinned();
    let object = pinned.get_or_create_cpp_object();
    let borrowed = pinned.borrow();

    let mut saved = record(events, "entrySaved");
    let mut rejected = record(events, "entrySaveAuthRejected:");
    let mut limit = record(events, "codeLimitReached");
    let mut locking = record(events, "lockingStatus:");
    let mut failed = record(events, "entrySaveFailed");
    let mut opened = record(events, "databaseOpened");

    unsafe {
        connect(
            object,
            borrowed.entrySaved.to_cpp_representation(&*borrowed),
            move |_: &QString| saved(String::new()),
        );
        connect(
            object,
            borrowed
                .entrySaveAuthRejected
                .to_cpp_representation(&*borrowed),
            move |left: &i32| rejected(left.to_string()),
        );
        connect(
            object,
            borrowed.codeLimitReached.to_cpp_representation(&*borrowed),
            move || limit(String::new()),
        );
        connect(
            object,
            borrowed
                .lockingStatusReceived
                .to_cpp_representation(&*borrowed),
            move |status: &String| locking(status.clone()),
        );
        connect(
            object,
            borrowed.entrySaveFailed.to_cpp_representation(&*borrowed),
            move |_: &QString, _: &bool| failed(String::new()),
        );
        connect(
            object,
            borrowed.databaseOpened.to_cpp_representation(&*borrowed),
            move || opened(String::new()),
        );
    }
}

fn first_entry_edit(app_state: &Rc<QObjectBox<AppState>>) -> Result<RxEntryEdit, String> {
    let app_state = app_state.pinned();
    let app_state = app_state.borrow();
    let db = app_state.curr_db_ref().map_err(|err| err.to_string())?;
    let entry = db
        .all_entries_iter()
        .next()
        .ok_or("database has no entries")?;
    RxEntryEdit::from_values(entry.uuid, &entry.edit_values())
        .map_err(|err| format!("{err:?}"))
}

fn password_on_disk(path: &Path, uuid: Uuid) -> Result<Option<String>, String> {
    let key = DatabaseKey::new().with_password(PASSWORD);
    let (db, _): (_, RxSourceFile) =
        read_database(path, key).map_err(|err| err.to_string())?;
    let entry = db
        .entry(EntryId::from_uuid(uuid))
        .ok_or("entry missing on disk")?;
    Ok(entry
        .fields
        .get("Password")
        .map(|value| value.get().as_str().to_string()))
}

async fn step<M>(name: &str, request: Request<KeepassRxActor, M>) -> Result<M::Result, String>
where
    M: Message + Send + 'static,
    M::Result: Send,
    KeepassRxActor: Handler<M>,
{
    tokio::time::timeout(STEP_TIMEOUT, request)
        .await
        .map_err(|_| format!("{name} timed out"))?
        .map_err(|err| format!("{name} mailbox error: {err}"))
}

async fn drive(
    gui: Rc<QObjectBox<KeepassRx>>,
    app_state: Rc<QObjectBox<AppState>>,
    events: Events,
    db_path: std::path::PathBuf,
    backup_path: std::path::PathBuf,
) -> Result<Observed, String> {
    let original = fs::read(&db_path).map_err(|err| err.to_string())?;

    for _ in 0..200 {
        if gui.pinned().borrow().actor.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let addr = gui
        .pinned()
        .borrow()
        .actor
        .clone()
        .ok_or("actor never started")?;

    step(
        "store password",
        addr.send(StoreMasterPassword {
            master_password: SecUtf8::from(PASSWORD),
        }),
    )
    .await?;
    step(
        "open database",
        addr.send(OpenDatabase {
            db_name: FIXTURE.to_string(),
            db_type: RxDbType::Imported,
        }),
    )
    .await?
    .map_err(|err| err.to_string())?;
    step("encrypt password", addr.send(EncryptMasterPassword)).await?;

    let mut edit = first_entry_edit(&app_state)?;
    let uuid = edit.uuid;
    edit.password = Zeroizing::new(NEW_PASSWORD.to_string());
    events.borrow_mut().clear();

    step(
        "save with wrong code",
        addr.send(SaveEntry {
            edit: edit.clone(),
            credential: SaveCredential::from_input("wrong"),
        }),
    )
    .await?;
    let rejected_events = std::mem::take(&mut *events.borrow_mut());
    let rejected_bytes_unchanged =
        fs::read(&db_path).map_err(|err| err.to_string())? == original;

    step(
        "save with right code",
        addr.send(SaveEntry {
            edit,
            credential: SaveCredential::from_input(SHORT_CODE),
        }),
    )
    .await?;
    let saved_events = std::mem::take(&mut *events.borrow_mut());

    events.borrow_mut().clear();
    for _ in 0..3 {
        step(
            "save with wrong code again",
            addr.send(SaveEntry {
                edit: first_entry_edit(&app_state)?,
                credential: SaveCredential::from_input("wrong"),
            }),
        )
        .await?;
    }
    step("check locking status", addr.send(CheckLockingStatus)).await?;
    let lockout_events = std::mem::take(&mut *events.borrow_mut());

    let backup_is_original = fs::read(&backup_path).is_ok_and(|bytes| bytes == original);
    let saved_password = password_on_disk(&db_path, uuid)?;
    let gui_saving_after = gui.pinned().borrow().saving;

    Ok(Observed {
        rejected_events,
        rejected_bytes_unchanged,
        saved_events,
        lockout_events,
        saved_password,
        backup_is_original,
        gui_saving_after,
    })
}

#[test]
fn saving_after_the_password_is_encrypted_works() {
    let data_home = tempfile::tempdir().expect("tempdir");
    unsafe {
        std::env::set_var("XDG_DATA_HOME", data_home.path().join("data"));
        std::env::set_var("XDG_CONFIG_HOME", data_home.path().join("config"));
        std::env::set_var("QT_QPA_PLATFORM", "offscreen");
    }
    fs::create_dir_all(data_home.path().join("config")).expect("config dir");

    libsodium_rs::ensure_init().expect("libsodium");
    crate::crypto::test_keyring::install();

    let imported = imported_databases_path();
    fs::create_dir_all(&imported).expect("imported dir");
    let db_path = imported.join(FIXTURE);
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(FIXTURE),
        &db_path,
    )
    .expect("copy fixture");
    let backup_path = super::utils::app_data_path()
        .join("backups")
        .join("imported")
        .join(format!("{FIXTURE}.bak"));

    let engine = Rc::new(QmlEngine::new());
    let result: Rc<RefCell<Option<Result<Observed, String>>>> = Rc::new(RefCell::new(None));

    qmeta_async::run({
        let engine = engine.clone();
        let result = result.clone();
        move || {
            let events: Events = Rc::default();
            let (gui, app_state, _settings) = with_executor(|| {
                let settings = Rc::new(QObjectBox::new(SettingsBridge::default()));
                let app_state = Rc::new(QObjectBox::new(AppState::new(&settings)));
                let gui = Rc::new(QObjectBox::new(KeepassRx::new()));
                KeepassRxActor::new(&gui, &app_state).start();
                connect_signals(&gui, &events);
                (gui, app_state, settings)
            });

            let quitter = engine.clone();
            with_executor(|| {
                tokio::task::spawn_local(async move {
                    let outcome = drive(gui, app_state, events, db_path, backup_path).await;
                    *result.borrow_mut() = Some(outcome);
                    quitter.quit();
                });
            });
            engine.exec();
        }
    })
    .expect("running the qmeta-async executor");

    let observed = result
        .borrow_mut()
        .take()
        .expect("the driver did not finish")
        .expect("driving the actor failed");

    assert_eq!(observed.rejected_events, vec!["entrySaveAuthRejected:2"]);
    assert!(observed.rejected_bytes_unchanged);
    assert_eq!(observed.saved_events, vec!["entrySaved"]);
    assert_eq!(
        observed.lockout_events,
        vec![
            "entrySaveAuthRejected:2",
            "entrySaveAuthRejected:1",
            "codeLimitReached",
            "lockingStatus:unset",
        ]
    );
    assert_eq!(observed.saved_password.as_deref(), Some(NEW_PASSWORD));
    assert!(observed.backup_is_original);
    assert!(!observed.gui_saving_after);
}
