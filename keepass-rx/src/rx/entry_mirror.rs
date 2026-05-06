use log::{debug, info};
use memoffset::offset_of;
use mirror_from_macro::mirror_from;
use std::collections::HashMap;

#[mirror_from(keepass::db::Entry, import = "keepass::db::*")]
struct EntryMirror;

unsafe fn attachments_map(
    entry: &keepass::db::Entry,
) -> &HashMap<String, keepass::db::AttachmentId> {
    let base = entry as *const keepass::db::Entry as *const u8;
    let offset = offset_of!(EntryMirror, attachments);
    info!(
        "Reading mirrored attachments map for entry {} at offset {}",
        entry.id().uuid(),
        offset
    );
    unsafe { &*(base.add(offset) as *const HashMap<String, keepass::db::AttachmentId>) }
}

pub(crate) trait NamedAttachmentsHack {
    fn named_attachments_hack(
        &self,
    ) -> impl Iterator<Item = (String, keepass::db::Attachment)> + '_;
}

impl<'a> NamedAttachmentsHack for keepass::db::EntryRef<'a> {
    fn named_attachments_hack(
        &self,
    ) -> impl Iterator<Item = (String, keepass::db::Attachment)> + '_ {
        let entry_uuid = self.id().uuid();
        debug!("Starting named attachment hack for entry {}", entry_uuid);
        let names = unsafe { attachments_map(self).keys().cloned().collect::<Vec<_>>() };

        debug!(
            "Collected {} mirrored attachment names for entry {}",
            names.len(),
            entry_uuid
        );

        names.into_iter().filter_map(move |name| {
            debug!(
                "Resolving mirrored attachment name for entry {}",
                entry_uuid
            );
            self.attachment_by_name(&name)
                .map(|attachment| (name, (*attachment).clone()))
        })
    }
}
