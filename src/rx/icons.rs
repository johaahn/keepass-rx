use base64::{Engine, prelude::BASE64_STANDARD};
use infer;

mod kpxc {
    include!(concat!(env!("OUT_DIR"), "/kpxc_icons.rs"));
}

pub fn to_builtin_icon(icon_id: usize) -> Option<String> {
    if icon_id < kpxc::FILES.len() {
        Some(kpxc::FILES[icon_id].to_string())
    } else {
        None
    }
}

#[derive(Default, Clone)]
pub enum RxIcon {
    Builtin(usize),
    Image(Vec<u8>),
    #[default]
    None,
}

impl RxIcon {
    pub fn is_builtin(&self) -> bool {
        matches!(self, Self::Builtin(_))
    }

    pub fn icon_path(&self) -> Option<String> {
        if matches!(self, Self::None) {
            return None;
        }

        match self {
            RxIcon::None => None,
            RxIcon::Builtin(id) => to_builtin_icon(*id),
            RxIcon::Image(img_data) => infer::get(img_data).map(|k| {
                format!(
                    "data:{};base64,{}",
                    k.mime_type(),
                    BASE64_STANDARD.encode(img_data)
                )
            }),
        }
    }
}
