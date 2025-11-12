use include_directory::{Dir, include_directory};

static PROJECT_DIR: Dir<'_> = include_directory!("$CARGO_MANIFEST_DIR/assets/icons");

pub fn to_builtin_icon(icon_id: usize) -> Option<String> {
    let glob = "**/*.svg";
    for entry in PROJECT_DIR.find(glob).unwrap() {
        let filename = entry.path().file_name();
        let found = filename
            .and_then(|name| name.to_str())
            .map(|name| name.contains(&format!("C{}", icon_id)))
            .unwrap_or(false);

        if found {
            return filename
                .and_then(|name| name.to_str())
                .map(|name| name.to_string());
        }
    }

    None
}
