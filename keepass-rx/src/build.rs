/* Copyright (C) 2018 Olivier Goffart <ogoffart@woboq.com>
Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense,
and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:
The above copyright notice and this permission notice shall be included in all copies or substantial
portions of the Software.
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES
OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */
#![cfg_attr(debug_assertions, allow(dead_code, unused_imports))]

use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;

/// Perform qmake query
#[cfg(feature = "gui")]
fn qmake_query(qmake: &str, args: &str, var: &str) -> String {
    let mut qmake_cmd_list: Vec<&str> = qmake.split(' ').collect();
    qmake_cmd_list.push("-query");
    qmake_cmd_list.push(var);

    if !args.is_empty() {
        qmake_cmd_list.append(&mut args.split(' ').collect());
    }

    String::from_utf8(
        Command::new(qmake_cmd_list[0])
            .args(&qmake_cmd_list[1..])
            .output()
            .expect("Failed to execute qmake. Make sure 'qmake' is in your path")
            .stdout,
    )
    .expect("UTF-8 conversion failed")
}

/// Generates the shell command to call for qmake
#[cfg(feature = "gui")]
fn qmake_call() -> String {
    env::var("QMAKE").unwrap_or(String::from("qmake"))
}

/// Generates the arguments for the call to qmake
#[cfg(feature = "gui")]
fn qmake_args() -> String {
    env::var("QMAKE_ARGS").unwrap_or_default()
}

fn output_kpxc_icons() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let asset_dir = Path::new(&manifest_dir).join("assets/icons");
    let mut entries = Vec::new();

    let walkdir = WalkDir::new(asset_dir).sort_by(|a, b| a.file_name().cmp(b.file_name()));

    for entry in walkdir {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            entries.push(entry.path().file_name().unwrap().to_os_string());
        }
    }

    assert_eq!(
        entries.len(),
        69,
        "Expected 69 db icons, but found {}",
        entries.len()
    );

    let out = format!(
        "pub const FILES: &[&str; {}] = &{:?};",
        entries.len(),
        entries
    );
    fs::write(Path::new(&out_dir).join("kpxc_icons.rs"), out).unwrap();
}

/// Generate gettext translation files
fn update_language_files() {
    let pot_file = "../po/keepassrx.projectmoon.pot";
    let source_files = source_files();

    let mut child = Command::new("xgettext")
        .args([
            &format!("--output={}", pot_file),
            "--language=javascript",
            "--qt",
            "--keyword=tr",
            "--keyword=tr:1,2",
            "--keyword=QT_TR_NOOP",
            "--keyword=QT_TR_NOOP:1,2",
            "--add-comments=i18n",
            "--from-code=UTF-8",
        ])
        .args(&source_files)
        .spawn()
        .unwrap();

    let exit_status = child.wait().unwrap();
    assert!(exit_status.code() == Some(0));

    for po_file in po_files() {
        let po_file_name = po_file
            .to_str()
            .expect("po language file name contains invalid characters");

        let mut child = Command::new("msgmerge")
            .args(["--update", po_file_name, pot_file])
            .spawn()
            .unwrap();

        let exit_status = child.wait().unwrap();
        assert!(exit_status.code() == Some(0));

        let install_dir = env::var("INSTALL_DIR").expect("No env var INSTALL_DIR provided");
        let lang = po_file.file_stem().unwrap().to_str().unwrap();
        let mo_dir = format!("{install_dir}/share/locale/{lang}/LC_MESSAGES");
        let mo_file = format!("{}/keepassrx.projectmoon.mo", mo_dir);

        fs::create_dir_all(&mo_dir)
            .expect("Failed to create directory for compiled language files");

        let mut child = Command::new("msgfmt")
            .args([po_file_name, "-o", &mo_file])
            .spawn()
            .unwrap();

        let exit_status = child.wait().unwrap();
        assert!(exit_status.code() == Some(0));
    }
}

/// Obtains a list of all QML files
fn source_files() -> Vec<PathBuf> {
    // Directory in which to search for QML files and file extension
    let mut qml = walk_dir(PathBuf::from("qml"), "qml");
    let mut src = walk_dir(PathBuf::from("src"), "rs");

    let mut files = vec![];
    files.append(&mut qml);
    files.append(&mut src);
    files
}

/// Obtains a list of all translation files
fn po_files() -> Vec<PathBuf> {
    // Directory in which to search for translation files
    walk_dir(PathBuf::from("../po"), "po")
}

/// Recursively searches for files in a directory and
/// returns a list of paths to the files
fn walk_dir(dir: PathBuf, ext: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in fs::read_dir(dir.clone())
        .expect(&format!(
            "Failed to iterate over directory: {:?}",
            dir.file_name()
        ))
        .filter_map(Result::ok)
    {
        if entry.file_type().unwrap().is_dir() {
            files.append(&mut walk_dir(entry.path(), ext));
        } else if let Some(file_ext) = entry.path().extension() {
            if file_ext.to_str().unwrap() == ext {
                files.push(entry.path())
            }
        }
    }

    files
}

#[cfg(feature = "gui")]
fn main() {
    output_kpxc_icons();
    update_language_files();

    let qmake_cmd = qmake_call();
    let args = qmake_args();

    let qt_include_path = qmake_query(&qmake_cmd, &args, "QT_INSTALL_HEADERS");
    let qt_library_path = qmake_query(&qmake_cmd, &args, "QT_INSTALL_LIBS");

    cpp_build::Config::new()
        .include(qt_include_path.trim())
        .build("src/main.rs");

    let macos_lib_search = if cfg!(target_os = "macos") {
        "=framework"
    } else {
        ""
    };
    let lib_framework = if cfg!(target_os = "macos") { "" } else { "5" };
    let qt_library_path = qt_library_path.trim();

    println!("cargo:rerun-if-changed=src/main.rs");
    println!("cargo:rerun-if-changed=src/rx/icons.rs");
    println!("cargo:rerun-if-changed=src/rx/virtual_hierarchy.rs");
    println!("cargo:rerun-if-changed=../po/*");

    println!("cargo:rustc-link-search{macos_lib_search}={qt_library_path}");
    println!("cargo:rustc-link-lib{macos_lib_search}=Qt{lib_framework}Widgets");
    println!("cargo:rustc-link-lib{macos_lib_search}=Qt{lib_framework}Gui");
    println!("cargo:rustc-link-lib{macos_lib_search}=Qt{lib_framework}Core");
    println!("cargo:rustc-link-lib{macos_lib_search}=Qt{lib_framework}Quick");
    println!("cargo:rustc-link-lib{macos_lib_search}=Qt{lib_framework}Qml");
    println!("cargo:rustc-link-lib{macos_lib_search}=Qt{lib_framework}QuickControls2");
    println!("cargo:rustc-link-lib{macos_lib_search}=Qt{lib_framework}WebEngine");
}

#[cfg(not(feature = "gui"))]
fn main() {
    output_kpxc_icons();
}
