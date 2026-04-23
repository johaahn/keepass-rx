use std::cell::RefCell;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use actor_macro::observing_model;
use anyhow::{Result, anyhow};
use base64::{Engine, prelude::BASE64_STANDARD};
use infer::MatcherType;
use libsodium_rs::utils::SecureVec;
use log::{error, warn};
use qmeta_async::with_executor;
use qmetaobject::prelude::*;
use qmetaobject::{QVariantMap, SimpleListModel};
use uuid::Uuid;

use crate::crypto::MasterKey;
use crate::gui::utils::exported_attachments_path;
use crate::rx::RxAttachments;
use crate::{app::AppState, rx::virtual_hierarchy::VirtualHierarchyType};

#[derive(Clone, Default, SimpleListItem, Debug)]
#[allow(non_snake_case)]
pub struct RxUiAttachment {
    pub attachmentName: QString,
    pub attachmentSize: i32,
    pub attachmentMimeType: QString,
    pub attachmentViewType: QString,
}

fn convert_attachments(value: &RxAttachments, master_key: &MasterKey) -> Vec<RxUiAttachment> {
    value
        .iter()
        .map(|(name, attachment)| {
            let attachment_bytes = attachment.value_secure(master_key);
            let attachment_size = attachment_bytes
                .as_ref()
                .map(|val| val.len())
                .unwrap_or(0)
                .try_into()
                .ok()
                .unwrap_or(0);
            let attachment_mime_type = attachment_bytes
                .as_ref()
                .and_then(|val| infer::get(val).map(|kind| kind.mime_type().to_string()))
                .or_else(|| {
                    attachment_bytes.as_ref().and_then(|val| {
                        std::str::from_utf8(val).ok().map(|_| "text/plain".to_string())
                    })
                })
                .unwrap_or_else(|| "unknown".to_string());
            let attachment_view_type = attachment_bytes
                .as_ref()
                .map(|val| view_type_for_attachment(val))
                .unwrap_or_default();

            RxUiAttachment {
                attachmentName: QString::from(name.as_str()),
                attachmentSize: attachment_size,
                attachmentMimeType: attachment_mime_type.into(),
                attachmentViewType: attachment_view_type.into(),
            }
        })
        .collect()
}

fn view_type_for_attachment(bytes: &[u8]) -> &'static str {
    match infer::get(bytes) {
        Some(kind) if kind.matcher_type() == MatcherType::Text => "text",
        Some(kind) if kind.matcher_type() == MatcherType::Image => "image",
        None if std::str::from_utf8(bytes).is_ok() => "text",
        _ => "",
    }
}

fn export_result(
    ok: bool,
    path: &str,
    url: &str,
    file_name: &str,
    error: &str,
) -> QVariantMap {
    let mut map = QVariantMap::default();
    map.insert("ok".into(), ok.into());
    map.insert("path".into(), QString::from(path).into());
    map.insert("url".into(), QString::from(url).into());
    map.insert("fileName".into(), QString::from(file_name).into());
    map.insert("error".into(), QString::from(error).into());
    map
}

fn export_success(path: &Path, url: String, file_name: String) -> QVariantMap {
    export_result(true, &path.to_string_lossy(), &url, &file_name, "")
}

fn export_error(error: impl ToString) -> QVariantMap {
    let error = error.to_string();
    error!("Attachment export failed: {}", error);
    export_result(false, "", "", "", &error)
}

fn view_result(
    ok: bool,
    can_view: bool,
    view_type: &str,
    file_name: &str,
    mime_type: &str,
    text: &str,
    data_url: &str,
    error: &str,
) -> QVariantMap {
    let mut map = QVariantMap::default();
    map.insert("ok".into(), ok.into());
    map.insert("canView".into(), can_view.into());
    map.insert("viewType".into(), QString::from(view_type).into());
    map.insert("fileName".into(), QString::from(file_name).into());
    map.insert("mimeType".into(), QString::from(mime_type).into());
    map.insert("text".into(), QString::from(text).into());
    map.insert("dataUrl".into(), QString::from(data_url).into());
    map.insert("error".into(), QString::from(error).into());
    map
}

fn view_error(error: impl ToString) -> QVariantMap {
    let error = error.to_string();
    error!("Attachment view failed: {}", error);
    view_result(false, false, "", "", "", "", "", &error)
}

fn sanitize_export_file_name(value: &str) -> String {
    let mut clean: String = value
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect();

    clean = clean.trim().trim_matches('.').chars().take(180).collect();

    if clean.is_empty() {
        "attachment".to_string()
    } else {
        clean
    }
}

fn unique_export_path(file_name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_micros())
        .unwrap_or_default();

    exported_attachments_path().join(format!("{}_{}", now, file_name))
}

fn percent_encode_file_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    let mut encoded = String::from("file://");

    for byte in path.as_bytes() {
        let ch = *byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-' | '_' | '.' | '~') {
            encoded.push(ch);
        } else {
            encoded.push_str(&format!("%{:02X}", byte));
        }
    }

    encoded
}

fn write_export_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;

    file.write_all(bytes)?;
    file.flush()?;
    Ok(())
}

/// A QObject that is wired to interact with a database entry via the
/// app actor.
#[observing_model]
#[derive(Default, QObject)]
#[allow(dead_code, non_snake_case)]
pub struct RxUiEntry {
    pub(super) base: qt_base_class!(trait QObject),

    pub(super) entryUuid: qt_property!(QString),

    // TOTP
    pub(super) currentTotp: qt_property!(QString; NOTIFY currentTotpChanged),
    pub(super) currentTotpValidFor: qt_property!(QString; NOTIFY currentTotpValidForChanged),
    pub(super) currentTotpChanged: qt_signal!(),
    pub(super) currentTotpValidForChanged: qt_signal!(),
    pub(super) updateTotp: qt_method!(fn(&mut self)),

    // Attachments
    pub(super) attachments: qt_property!(RefCell<SimpleListModel<RxUiAttachment>>; NOTIFY attachmentsChanged),
    pub(super) attachmentsChanged: qt_signal!(),
    pub(super) loadAttachments: qt_method!(fn(&mut self)),
    pub(super) exportAttachment:
        qt_method!(fn(&self, attachment_name: QString) -> QVariantMap),
    pub(super) cleanupExportedAttachment: qt_method!(fn(&self, path: QString)),
    pub(super) viewAttachment: qt_method!(fn(&self, attachment_name: QString) -> QVariantMap),
}

#[allow(dead_code, non_snake_case)]
impl RxUiEntry {
    fn init_from_state(&mut self, _: &AppState) {}
    fn init_from_view(&mut self, _: &VirtualHierarchyType) {}

    #[with_executor]
    pub fn loadAttachments(&mut self) {
        let load = || -> Result<()> {
            let entry_uuid = Uuid::from_str(&self.entryUuid.to_string())?;
            let app_state = self
                ._app
                .as_pinned()
                .ok_or_else(|| anyhow!("Unable to get app state"))?;

            let app_state = app_state.borrow();
            let maybe_db = app_state.curr_db_ref();
            let db = maybe_db?;

            let maybe_entry = db.get_entry(entry_uuid);
            let maybe_attach = maybe_entry.as_ref().map(|ent| &ent.attachments);

            let attachments = maybe_attach
                .map(|att| convert_attachments(att, db.master_key()))
                .unwrap_or_default();

            self.attachments.borrow_mut().reset_data(attachments);
            Ok(())
        };

        if let Err(err) = load() {
            error!("Unable to load attachments: {}", err);
        }
    }

    #[with_executor]
    pub fn exportAttachment(&self, attachment_name: QString) -> QVariantMap {
        let export = || -> Result<QVariantMap> {
            let attachment_name = attachment_name.to_string();
            let attachment_bytes = self.attachment_bytes(&attachment_name)?;

            let file_name = sanitize_export_file_name(&attachment_name);
            let export_dir = exported_attachments_path();
            fs::create_dir_all(&export_dir)?;

            let path = unique_export_path(&file_name);
            write_export_file(&path, &attachment_bytes)?;
            let url = percent_encode_file_path(&path);

            Ok(export_success(&path, url, file_name))
        };

        export().unwrap_or_else(export_error)
    }

    fn attachment_bytes(&self, attachment_name: &str) -> Result<SecureVec<u8>> {
        let entry_uuid = Uuid::from_str(&self.entryUuid.to_string())
            .map_err(|err| anyhow!("Invalid entry UUID for attachment: {}", err))?;
        let app_state = self
            ._app
            .as_pinned()
            .ok_or_else(|| anyhow!("Unable to get app state"))?;
        let app_state = app_state.borrow();
        let db = app_state.curr_db_ref()?;
        let entry = db
            .get_entry(entry_uuid)
            .ok_or_else(|| anyhow!("Entry not found"))?;
        let attachment = entry
            .attachments
            .get(attachment_name)
            .ok_or_else(|| anyhow!("Attachment not found: {}", attachment_name))?;

        attachment
            .value_secure(db.master_key())
            .ok_or_else(|| anyhow!("Unable to read attachment data"))
    }

    #[with_executor]
    pub fn viewAttachment(&self, attachment_name: QString) -> QVariantMap {
        let view = || -> Result<QVariantMap> {
            let attachment_name = attachment_name.to_string();
            let attachment_bytes = self.attachment_bytes(&attachment_name)?;
            let file_name = sanitize_export_file_name(&attachment_name);
            let inferred = infer::get(&attachment_bytes);

            match inferred {
                Some(kind) if kind.matcher_type() == MatcherType::Text => {
                    let text = std::str::from_utf8(&attachment_bytes)
                        .map_err(|err| anyhow!("Unable to decode text attachment: {}", err))?;
                    Ok(view_result(
                        true,
                        true,
                        "text",
                        &file_name,
                        kind.mime_type(),
                        text,
                        "",
                        "",
                    ))
                }
                Some(kind) if kind.matcher_type() == MatcherType::Image => {
                    let data_url = format!(
                        "data:{};base64,{}",
                        kind.mime_type(),
                        BASE64_STANDARD.encode(&*attachment_bytes)
                    );
                    Ok(view_result(
                        true,
                        true,
                        "image",
                        &file_name,
                        kind.mime_type(),
                        "",
                        &data_url,
                        "",
                    ))
                }
                Some(kind) => Ok(view_result(
                    true,
                    false,
                    "",
                    &file_name,
                    kind.mime_type(),
                    "",
                    "",
                    "",
                )),
                None => match std::str::from_utf8(&attachment_bytes) {
                    Ok(text) => Ok(view_result(
                        true,
                        true,
                        "text",
                        &file_name,
                        "text/plain",
                        text,
                        "",
                        "",
                    )),
                    Err(_) => Ok(view_result(true, false, "", &file_name, "", "", "", "")),
                },
            }
        };

        view().unwrap_or_else(view_error)
    }

    #[with_executor]
    pub fn cleanupExportedAttachment(&self, path: QString) {
        let path = path.to_string();
        if path.trim().is_empty() {
            return;
        }

        let cleanup = || -> Result<()> {
            let export_dir = exported_attachments_path();
            let export_dir = export_dir.canonicalize()?;
            let path = PathBuf::from(path);

            if !path.exists() {
                return Ok(());
            }

            let path = path.canonicalize()?;
            if !path.starts_with(&export_dir) {
                return Err(anyhow!(
                    "Refusing to clean up attachment export outside export directory: {}",
                    path.display()
                ));
            }

            fs::remove_file(&path)?;
            Ok(())
        };

        if let Err(err) = cleanup() {
            warn!("Unable to clean up exported attachment: {}", err);
        }
    }

    #[with_executor]
    pub fn updateTotp(&mut self) {
        let app_state = self._app.as_pinned().expect("No app state");
        let app_state = app_state.borrow();

        let maybe_db = app_state.curr_db_ref();

        let totp = maybe_db.and_then(|db| db.get_totp(&self.entryUuid.to_string()));

        if let Ok(totp) = totp {
            let totp_code = QString::from(totp.code);
            let valid_for = QString::from(totp.valid_for);

            if totp_code != self.currentTotp {
                self.currentTotp = totp_code;
                self.currentTotpChanged();
            }
            if valid_for != self.currentTotpValidFor {
                self.currentTotpValidFor = valid_for;
                self.currentTotpValidForChanged();
            }
        }
    }
}
