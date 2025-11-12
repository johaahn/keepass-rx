mod kpxc {
    include!(concat!(env!("OUT_DIR"), "/kpxc_icons.rs"));
}

fn stuff() {}

pub fn to_builtin_icon(icon_id: usize) -> Option<String> {
    use kpxc::FILES;

    if icon_id < FILES.len() {
        Some(FILES[icon_id].to_string())
    } else {
        None
    }
}
