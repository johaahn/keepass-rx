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
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use walkdir::WalkDir;

fn generate_licenses_rs() -> io::Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let out_file = out_dir.join("licenses.rs");

    // Check if `cargo about` is installed
    let about_available = Command::new("cargo")
        .arg("about")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);

    if about_available {
        let manifest_dir =
            PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
        let about_template = manifest_dir.parent().unwrap().join("about.hbs");

        let out = File::create(&out_file)?;
        let status = Command::new("cargo")
            .arg("about")
            .arg("generate")
            .arg(&about_template)
            .stdout(out)
            .status()?;

        match status.success() {
            true => return Ok(()),
            false => panic!("Failed to generate licenses.rs"),
        }
    } else {
        // Fallback: write an empty License array
        let mut file = File::create(&out_file)?;
        writeln!(
            file,
            "// Auto-generated fallback: cargo about not found\n\
         pub static LICENSES: &[License] = &[];"
        )?;
        Ok(())
    }
}

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

fn generate_zxcvbn_dictionary() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let source_dir = manifest_dir.join("src/zxcvbn");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let generator_source = source_dir.join("dict-generate.cpp");
    let words = [
        "words-eng_wiki.txt",
        "words-female.txt",
        "words-male.txt",
        "words-passwd.txt",
        "words-surname.txt",
        "words-tv_film.txt",
    ];
    let generator = out_dir.join("zxcvbn-dictgen");
    let header = out_dir.join("dict-src.h");

    // This program runs during the build, so it must be built for the host,
    // even when the C library is cross-compiled for the target.
    let compiler = env::var_os("HOST_CXX").unwrap_or_else(|| "c++".into());
    let status = Command::new(compiler)
        .arg("-std=c++11")
        .arg(&generator_source)
        .arg("-o")
        .arg(&generator)
        .status()
        .expect("Failed to run C++ compiler for zxcvbn dictionary generator");
    assert!(
        status.success(),
        "Failed to compile zxcvbn dictionary generator"
    );

    let mut command = Command::new(generator);
    command.arg("-o").arg(&header);
    for word in words {
        command.arg(source_dir.join(word));
    }
    let status = command
        .status()
        .expect("Failed to run zxcvbn dictionary generator");
    assert!(
        status.success(),
        "Failed to generate zxcvbn dictionary header"
    );
}

fn compile_kpxc_zxcvbn() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    cc::Build::new()
        .file("src/zxcvbn/zxcvbn.c")
        .include("src/zxcvbn")
        .include(out_dir)
        .compile("kpxc_zxcvbn");
}

fn build_zxcvbn() {
    generate_zxcvbn_dictionary();
    println!("cargo:rerun-if-changed=src/zxcvbn/dict-generate.cpp");
    for word in [
        "words-eng_wiki.txt",
        "words-female.txt",
        "words-male.txt",
        "words-passwd.txt",
        "words-surname.txt",
        "words-tv_film.txt",
    ] {
        println!("cargo:rerun-if-changed=src/zxcvbn/{word}");
    }
    println!("cargo:rerun-if-env-changed=HOST_CXX");
    println!("cargo:rerun-if-changed=src/zxcvbn/zxcvbn.c");
    println!("cargo:rerun-if-changed=src/zxcvbn/zxcvbn.h");
    compile_kpxc_zxcvbn();
}

/// Generate gettext translation files
fn update_language_files() {
    // INSTALL_DIR is provided by clickable (the Ubuntu Touch build). Outside of
    // clickable (e.g. the Sailfish OS / mb2 build, or a plain `cargo build`) it
    // is absent; in that case the RPM's %install step is responsible for
    // compiling and installing the .mo files, so skip generation here rather
    // than panicking or mutating the source .po tree.
    if env::var("INSTALL_DIR").is_err() {
        return;
    }

    let pot_file = "../po/keepassrx.projectmoon.pot";
    let source_files = source_files();

    let mut child = Command::new("xgettext")
        .args([
            &format!("--output={}", pot_file),
            "--language=javascript",
            "--qt",
            "--keyword=tr",
            "--keyword=tr:1,2",
            "--keyword=ctr:1c,2",
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

    add_safe_html_type_comments(&PathBuf::from(&pot_file));

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

fn add_safe_html_type_comments(pot_path: &Path) {
    let text = fs::read_to_string(pot_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", pot_path.display(), e));

    let mut units: Vec<String> = Vec::new();
    let mut cur = String::new();
    for line in text.split_inclusive('\n') {
        if line.starts_with("#:") {
            if !cur.is_empty() {
                units.push(std::mem::take(&mut cur));
            }
        }
        cur.push_str(line);
    }
    if !cur.is_empty() {
        units.push(cur);
    }

    let mut out = String::new();
    for unit in units {
        if let Some((loc, rest)) = unit.split_once('\n') {
            if loc.starts_with("#:")
                && contains_html_tag(&extractmsgid(&unit))
                && !unit.contains("#~ type: auto-safe-html")
            {
                out.push_str(loc);
                out.push_str("\n");
                out.push_str("#~ type: auto-safe-html\n");
                out.push_str(rest);
                continue;
            }
        }
        out.push_str(&unit);
    }

    fs::write(pot_path, out)
        .unwrap_or_else(|e| panic!("failed to write {}: {}", pot_path.display(), e));
}

/// Returns true iff `s` contains `<` immediately followed by an ASCII letter —
/// i.e. the start of an HTML tag, mirroring Weblate's own detection.
fn contains_html_tag(s: &str) -> bool {
    s.split('<')
        .skip(1)
        .any(|r| r.chars().next().map_or(false, |c| c.is_ascii_alphabetic()))
}

/// Reconstructs a unit's `msgid` for HTML detection. We anchor on the `msgid:`
/// line itself (skipping any preceding `msgctxt` block and its `"` continuation
/// lines), then append every following `"` line until the `msgstr` line.
fn extractmsgid(unit: &str) -> String {
    let mut lines = unit.split('\n');
    let mut msgid = String::new();
    let mut started = false;
    for line in lines.by_ref() {
        if started {
            if line.starts_with("msgstr") {
                break;
            }
            if let Some(v) = line.strip_prefix('"') {
                msgid.push_str(v);
            }
        } else if let Some(v) = line.strip_prefix("msgid ") {
            msgid.push_str(v);
            started = true;
        }
    }
    msgid
}

/// Obtains a list of all QML files
fn source_files() -> Vec<PathBuf> {
    // Directory in which to search for QML files and file extension
    let mut qml = walk_dir(PathBuf::from("qml"), "qml");
    let mut src = walk_dir(PathBuf::from("src"), "rs");

    let mut files = vec![];
    files.append(&mut qml);
    files.append(&mut src);

    // The Sailfish OS QML tree translates via the `Tr.tr`/`Tr.ctr` gettext
    // bridge, so its strings must be extracted too.
    if PathBuf::from("qml-sfos").is_dir() {
        files.append(&mut walk_dir(PathBuf::from("qml-sfos"), "qml"));
    }

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
    generate_licenses_rs().expect("Unable to generate licenses");
    output_kpxc_icons();
    build_zxcvbn();
    update_language_files();

    let qmake_cmd = qmake_call();
    let args = qmake_args();

    let qt_include_path = qmake_query(&qmake_cmd, &args, "QT_INSTALL_HEADERS");
    let qt_library_path = qmake_query(&qmake_cmd, &args, "QT_INSTALL_LIBS");
    let qt_include_path = qt_include_path.trim();

    let mut cfg = cpp_build::Config::new();
    cfg.include(qt_include_path);

    // Sailfish OS ships an old Qt 5.6. Point the compiler at QtCore explicitly
    // and silence -Wdeprecated-copy noise that its headers trigger (matches
    // the Whisperfish setup).
    if cfg!(feature = "sailfish") {
        cfg.include(format!("{}/QtCore", qt_include_path))
            .flag("-Wno-deprecated-copy");
    }

    cfg.build("src/main.rs");

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
    println!("cargo:rerun-if-changed=../about.hbs");
    println!("cargo:rerun-if-changed=qml/");

    println!("cargo:rustc-link-search{macos_lib_search}={qt_library_path}");

    // Qt modules needed on every platform.
    println!("cargo:rustc-link-lib{macos_lib_search}=Qt{lib_framework}Gui");
    println!("cargo:rustc-link-lib{macos_lib_search}=Qt{lib_framework}Core");
    println!("cargo:rustc-link-lib{macos_lib_search}=Qt{lib_framework}Quick");
    println!("cargo:rustc-link-lib{macos_lib_search}=Qt{lib_framework}Qml");

    // Ubuntu Touch (Lomiri, Qt 5.15) additionally uses Widgets, the
    // QtQuickControls2 style engine, and WebEngine. None of these exist on
    // Sailfish OS' Qt 5.6, and the Silica UI does not need them.
    if !cfg!(feature = "sailfish") {
        println!("cargo:rustc-link-lib{macos_lib_search}=Qt{lib_framework}Widgets");
        println!("cargo:rustc-link-lib{macos_lib_search}=Qt{lib_framework}QuickControls2");
        println!("cargo:rustc-link-lib{macos_lib_search}=Qt{lib_framework}WebEngine");
    }
}

#[cfg(not(feature = "gui"))]
fn main() {
    output_kpxc_icons();
    build_zxcvbn();
    generate_licenses_rs().expect("Unable to generate licenses");
    println!("cargo:rerun-if-changed=../about.hbs");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    const POT: &str = "msgid \"\"\nmsgstr \"\"\n\"Content-Type: text/plain; charset=UTF-8\\n\"\n\n#: foo:1\nmsgid \"<b>hi</b>\"\nmsgstr \"\"\n\n#: bar:2\nmsgid \"plain text\"\nmsgstr \"\"\n\n#: baz:3\nmsgctxt \"foo\"\nmsgid \"<i>italic</i>\"\n\"extra\"\nmsgstr \"\"\n#~ type: auto-safe-html\n";

    fn write_tmp(pot: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("keepass_rx_build_test_{}.pot", std::process::id()));
        fs::write(&path, pot)
            .unwrap_or_else(|e| panic!("failed to write {}: {}", path.display(), e));
        path
    }

    #[test]
    fn add_safe_html_type_comments_marks_html_units() {
        let path = write_tmp(POT);
        add_safe_html_type_comments(&path);
        let out = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));

        let count = out.matches("#~ type: auto-safe-html").count();
        assert_eq!(count, 2, "marker count = {}; out = {}", count, out);

        let lines: Vec<&str> = out.split('\n').collect();

        let foo = lines.iter().position(|l| *l == "#: foo:1").unwrap();
        assert_eq!(
            lines[foo + 1],
            "#~ type: auto-safe-html",
            "marker not right after #: foo:1\nout = {}",
            out
        );

        let baz = lines.iter().position(|l| *l == "#: baz:3").unwrap();
        assert_eq!(
            lines[baz + 1],
            r#"msgid "<i>italic</i>""#,
            "marker inserted mid-unit\nout = {}",
            out
        );
        let existing = lines
            .iter()
            .position(|l| *l == "#~ type: auto-safe-html")
            .unwrap();
        let baz_msgid = lines
            .iter()
            .position(|l| *l == r#"msgid "<i>italic</i>""#)
            .unwrap();
        assert!(
            existing > baz_msgid,
            "existing marker should stay after msgid\nout = {}",
            out
        );

        let bar = lines.iter().position(|l| *l == "#: bar:2").unwrap();
        assert_eq!(
            lines[bar + 1],
            r#"msgid "plain text""#,
            "plain unit changed\nout = {}",
            out
        );
        assert!(
            !out.contains("#: bar:2\n#~ type: auto-safe-html"),
            "plain unit got a marker\nout = {}",
            out
        );

        add_safe_html_type_comments(&path);
        let out2 = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
        let count2 = out2.matches("#~ type: auto-safe-html").count();
        assert_eq!(
            count2, 2,
            "second run changed marker count = {}; out2 = {}",
            count2, out2
        );

        fs::remove_file(&path)
            .unwrap_or_else(|e| panic!("failed to delete {}: {}", path.display(), e));
    }

    #[test]
    fn contains_html_tag() {
        assert!(contains_html_tag("<a href='x'>"));
        assert!(contains_html_tag("<b>bold</b>"));
        assert!(contains_html_tag("<br/>"));
        assert!(!contains_html_tag("C < 5"));
        assert!(!contains_html_tag("no html"));
    }

    #[test]
    fn extractmsgid_ignores_msgctxt() {
        let unit = "msgctxt \"foo\"\nmsgid \"the actual id\"\n\"continuation\"\nmsgstr \"\"\n";
        assert_eq!(extractmsgid(unit), "the actual id\"\ncontinuation\"");
    }
}
