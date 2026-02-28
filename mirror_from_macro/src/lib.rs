use proc_macro::TokenStream;
use quote::quote;
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Fields, Ident, Item, ItemStruct, LitInt, LitStr, Path as SynPath, Token,
};
use walkdir::WalkDir;

#[proc_macro_attribute]
pub fn mirror_from(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as MirrorArgs);
    let item = parse_macro_input!(item as ItemStruct);

    match expand(args, item) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[derive(Debug)]
struct MirrorArgs {
    target: SynPath,
    file: Option<LitStr>,
    roots: Vec<LitStr>,
    imports: Vec<LitStr>,
    max_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreferredPackage {
    name: String,
    version: Option<String>,
    source: Option<String>,
}

impl Parse for MirrorArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let target = input.parse::<SynPath>()?;

        let mut file = None;
        let mut roots = Vec::new();
        let mut imports = Vec::new();
        let mut max_depth = 12usize;

        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;

            if input.is_empty() {
                break;
            }

            let key = input.parse::<Ident>()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "file" => file = Some(input.parse::<LitStr>()?),
                "root" => roots.push(input.parse::<LitStr>()?),
                "import" => imports.push(input.parse::<LitStr>()?),
                "max_depth" => {
                    let lit = input.parse::<LitInt>()?;
                    max_depth = lit.base10_parse::<usize>()?;
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        key,
                        format!("unknown mirror_from option `{other}`"),
                    ));
                }
            }
        }

        Ok(Self {
            target,
            file,
            roots,
            imports,
            max_depth,
        })
    }
}

fn expand(args: MirrorArgs, item: ItemStruct) -> syn::Result<proc_macro2::TokenStream> {
    let mirror_ident = item.ident;
    let mirror_vis = item.vis;

    if !matches!(item.fields, Fields::Unit) {
        return Err(syn::Error::new_spanned(
            &mirror_ident,
            "mirror_from expects a unit struct, like `struct EntryMirror;`",
        ));
    }

    let target_segments = args
        .target
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>();

    let target_struct = target_segments
        .last()
        .cloned()
        .ok_or_else(|| syn::Error::new_spanned(&args.target, "empty target path"))?;

    let source = if let Some(file) = args.file {
        SourceHit {
            path: resolve_relative_to_manifest(&file.value()),
            fields: None,
            score: 10_000,
        }
    } else {
        find_source_file(&target_segments, &target_struct, &args)?
    };

    let src = fs::read_to_string(&source.path).map_err(|e| {
        syn::Error::new_spanned(
            &mirror_ident,
            format!("failed reading {:?}: {e}", source.path),
        )
    })?;

    let parsed = syn::parse_file(&src).map_err(|e| {
        syn::Error::new_spanned(
            &mirror_ident,
            format!("failed parsing {:?}: {e}", source.path),
        )
    })?;

    let fields = match source.fields {
        Some(fields) => fields,
        None => find_struct_fields(&parsed.items, &target_struct).ok_or_else(|| {
            syn::Error::new_spanned(
                &mirror_ident,
                format!(
                    "found {:?}, but it did not contain `struct {}`",
                    source.path, target_struct
                ),
            )
        })?,
    };

    let imports = args
        .imports
        .iter()
        .map(|s| {
            let stmt = format!("use {};", s.value());
            syn::parse_str::<syn::ItemUse>(&stmt)
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(quote! {
        #(#imports)*

        #mirror_vis struct #mirror_ident {
            #fields
        }
    })
}

#[derive(Clone)]
struct SourceHit {
    path: PathBuf,
    fields: Option<syn::punctuated::Punctuated<syn::Field, syn::token::Comma>>,
    score: i32,
}

fn find_source_file(
    target_segments: &[String],
    target_struct: &str,
    args: &MirrorArgs,
) -> syn::Result<SourceHit> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));

    let snake = to_snake_case(target_struct);
    let candidate_file_names = [
        format!("{snake}.rs"),
        "mod.rs".to_string(),
        "lib.rs".to_string(),
    ];
    let preferred_package = target_segments
        .first()
        .and_then(|crate_name| resolve_preferred_package(&manifest_dir, crate_name));

    let mut roots = Vec::<PathBuf>::new();

    for root in &args.roots {
        roots.push(resolve_relative_to_manifest(&root.value()));
    }

    if let Ok(root) = env::var("MIRROR_FROM_ROOT") {
        roots.push(PathBuf::from(root));
    }

    roots.push(manifest_dir.clone());

    for ancestor in manifest_dir.ancestors().take(8) {
        roots.push(ancestor.join("vendor"));
        roots.push(ancestor.join("third_party"));
        roots.push(ancestor.join("crates"));
        roots.push(ancestor.join(".cargo"));
    }

    if let Ok(cargo_home) = env::var("CARGO_HOME") {
        let cargo_home = PathBuf::from(cargo_home);
        roots.push(cargo_home.join("git").join("checkouts"));
        roots.push(cargo_home.join("registry").join("src"));
    } else if let Ok(home) = env::var("HOME") {
        let cargo_home = PathBuf::from(home).join(".cargo");
        roots.push(cargo_home.join("git").join("checkouts"));
        roots.push(cargo_home.join("registry").join("src"));
    }

    roots.sort();
    roots.dedup();

    let mut hits = Vec::<SourceHit>::new();
    let mut seen_paths = HashSet::<PathBuf>::new();

    for root in roots {
        if !root.exists() {
            continue;
        }

        for entry in WalkDir::new(&root)
            .follow_links(false)
            .max_depth(args.max_depth)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();

            let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };

            if !candidate_file_names.iter().any(|n| n == file_name) {
                continue;
            }

            if !path_matches_target_namespace(path, target_segments, preferred_package.as_ref()) {
                continue;
            }

            let Ok(src) = fs::read_to_string(path) else {
                continue;
            };

            let Ok(parsed) = syn::parse_file(&src) else {
                continue;
            };

            let Some(fields) = find_struct_fields(&parsed.items, target_struct) else {
                continue;
            };

            let score = score_path(path, target_segments, &snake, preferred_package.as_ref());
            let path_buf = path.to_path_buf();

            if !seen_paths.insert(path_buf.clone()) {
                continue;
            }

            hits.push(SourceHit {
                path: path_buf,
                fields: Some(fields),
                score,
            });
        }
    }

    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));

    let Some(best) = hits.first().cloned() else {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "could not find source file for `{}`; try `#[mirror_from({}, file = \"path/to/source.rs\")]` or set MIRROR_FROM_ROOT",
                target_struct,
                target_segments.join("::"),
            ),
        ));
    };

    if hits.len() > 1 && hits[1].score == best.score {
        let mut msg = format!(
            "ambiguous source file for `{}`; add `file = \"...\"`. Candidates:\n",
            target_struct
        );

        for hit in hits.iter().take(8) {
            msg.push_str(&format!("  score {:>4}: {}\n", hit.score, hit.path.display()));
        }

        return Err(syn::Error::new(proc_macro2::Span::call_site(), msg));
    }

    Ok(best)
}

fn find_struct_fields(
    items: &[Item],
    target_struct: &str,
) -> Option<syn::punctuated::Punctuated<syn::Field, syn::token::Comma>> {
    for item in items {
        match item {
            Item::Struct(s) if s.ident == target_struct => {
                if let Fields::Named(named) = &s.fields {
                    return Some(named.named.clone());
                }

                return None;
            }
            Item::Mod(m) => {
                if let Some((_, inner_items)) = &m.content {
                    if let Some(fields) = find_struct_fields(inner_items, target_struct) {
                        return Some(fields);
                    }
                }
            }
            _ => {}
        }
    }

    None
}

fn path_matches_target_namespace(
    path: &Path,
    target_segments: &[String],
    preferred_package: Option<&PreferredPackage>,
) -> bool {
    let components = path
        .components()
        .map(|component| normalize(&component.as_os_str().to_string_lossy()))
        .collect::<Vec<_>>();

    let Some(crate_name) = target_segments.first().map(|segment| normalize(segment)) else {
        return false;
    };

    if !path_matches_crate_name(&components, &crate_name, preferred_package) {
        return false;
    }

    if target_segments.len() <= 2 {
        return true;
    }

    let module_segments = target_segments[1..target_segments.len() - 1]
        .iter()
        .map(|segment| normalize(segment))
        .collect::<Vec<_>>();

    path_contains_ordered_components(&components, &module_segments)
}

fn path_matches_crate_name(
    components: &[String],
    crate_name: &str,
    preferred_package: Option<&PreferredPackage>,
) -> bool {
    if let Some(preferred_package) = preferred_package {
        if let Some(version) = &preferred_package.version {
            let package_dir = format!("{crate_name}-{}", normalize(version));
            if components.iter().any(|component| component == &package_dir) {
                return true;
            }
        }
    }

    components.iter().any(|component| {
        component == crate_name
            || component == &format!("{crate_name}.rs")
            || component.starts_with(&format!("{crate_name}-"))
    })
}

fn path_contains_ordered_components(components: &[String], required: &[String]) -> bool {
    if required.is_empty() {
        return true;
    }

    let mut start = 0usize;

    for required_component in required {
        let Some(offset) = components[start..]
            .iter()
            .position(|component| component == required_component)
        else {
            return false;
        };

        start += offset + 1;
    }

    true
}

fn score_path(
    path: &Path,
    target_segments: &[String],
    snake: &str,
    preferred_package: Option<&PreferredPackage>,
) -> i32 {
    let mut score = 0;
    let normalized_path = normalize(&path.display().to_string());
    let path_components = path
        .components()
        .map(|component| normalize(&component.as_os_str().to_string_lossy()))
        .collect::<Vec<_>>();
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(normalize)
        .unwrap_or_default();

    if path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|name| name == format!("{snake}.rs"))
    {
        score += 100;
    }

    if normalized_path.contains("/src/") {
        score += 10;
    }

    for (index, segment) in target_segments.iter().enumerate() {
        let n = normalize(segment);

        if index + 1 == target_segments.len() {
            if file_stem == n {
                score += 40;
            }
            continue;
        }

        if path_components.iter().any(|component| component == &n) {
            score += 40;
        } else if path_components
            .iter()
            .any(|component| component.starts_with(&format!("{n}-")))
        {
            score += 15;
        }
    }

    if let Some(preferred_package) = preferred_package {
        score += package_match_score(&normalized_path, preferred_package);
    }

    score
}

fn resolve_preferred_package(manifest_dir: &Path, crate_name: &str) -> Option<PreferredPackage> {
    let manifest_package = parse_manifest_dependency(&manifest_dir.join("Cargo.toml"), crate_name);
    let lockfile_package = manifest_dir
        .ancestors()
        .map(|dir| dir.join("Cargo.lock"))
        .filter(|path| path.is_file())
        .filter_map(|lockfile| parse_lockfile_package(&lockfile, crate_name))
        .max_by_key(|package| lockfile_package_compatibility(manifest_package.as_ref(), package));

    match (manifest_package, lockfile_package) {
        (Some(mut manifest), Some(lockfile)) => {
            if manifest.version.is_none() {
                manifest.version = lockfile.version;
            }

            if preferred_source_is_more_specific(lockfile.source.as_deref(), manifest.source.as_deref()) {
                manifest.source = lockfile.source;
            }

            Some(manifest)
        }
        (Some(manifest), None) => Some(manifest),
        (None, Some(lockfile)) => Some(lockfile),
        (None, None) => None,
    }
}

fn parse_manifest_dependency(manifest: &Path, crate_name: &str) -> Option<PreferredPackage> {
    let src = fs::read_to_string(manifest).ok()?;
    let mut in_dependencies = false;

    for line in src.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') {
            in_dependencies = matches!(
                trimmed,
                "[dependencies]" | "[build-dependencies]" | "[dev-dependencies]"
            );
            continue;
        }

        if !in_dependencies || trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        let Some((name, value)) = trimmed.split_once('=') else {
            continue;
        };

        if name.trim() != crate_name {
            continue;
        }

        let value = value.trim();

        if let Some(version) = parse_toml_quoted_string(value) {
            return Some(PreferredPackage {
                name: crate_name.to_string(),
                version: extract_semverish_version(version),
                source: Some("registry+".to_string()),
            });
        }

        if !value.starts_with('{') {
            continue;
        }

        let version =
            extract_key_from_inline_table(value, "version").and_then(extract_semverish_version);
        let source = if extract_key_from_inline_table(value, "git").is_some() {
            Some("git+".to_string())
        } else if extract_key_from_inline_table(value, "path").is_some() {
            Some("path+".to_string())
        } else {
            Some("registry+".to_string())
        };

        return Some(PreferredPackage {
            name: crate_name.to_string(),
            version,
            source,
        });
    }

    None
}

fn parse_lockfile_package(lockfile: &Path, crate_name: &str) -> Option<PreferredPackage> {
    let src = fs::read_to_string(lockfile).ok()?;

    let mut in_matching_package = false;
    let mut version = None;
    let mut source = None;

    for line in src.lines() {
        let trimmed = line.trim();

        if trimmed == "[[package]]" {
            if in_matching_package {
                break;
            }

            in_matching_package = false;
            version = None;
            source = None;
            continue;
        }

        if let Some(name) = parse_toml_string_value(trimmed, "name") {
            if in_matching_package {
                break;
            }

            in_matching_package = name == crate_name;
            continue;
        }

        if !in_matching_package {
            continue;
        }

        if let Some(value) = parse_toml_string_value(trimmed, "version") {
            version = Some(value.to_string());
            continue;
        }

        if let Some(value) = parse_toml_string_value(trimmed, "source") {
            source = Some(value.to_string());
        }
    }

    if in_matching_package {
        Some(PreferredPackage {
            name: crate_name.to_string(),
            version,
            source,
        })
    } else {
        None
    }
}

fn parse_toml_string_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key} = \"");
    let value = line.strip_prefix(&prefix)?;
    value.strip_suffix('"')
}

fn parse_toml_quoted_string(value: &str) -> Option<&str> {
    value.strip_prefix('"')?.strip_suffix('"')
}

fn extract_key_from_inline_table<'a>(value: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("{key} = ");
    let tail = value.split(&needle).nth(1)?.trim_start();
    let field = tail.split_once(',').map(|(head, _)| head).unwrap_or(tail).trim();
    parse_toml_quoted_string(field)
}

fn extract_semverish_version(input: &str) -> Option<String> {
    let chars = input.chars().collect::<Vec<_>>();

    for start in 0..chars.len() {
        if !chars[start].is_ascii_digit() {
            continue;
        }

        let mut end = start;
        while end < chars.len() && (chars[end].is_ascii_digit() || chars[end] == '.') {
            end += 1;
        }

        let candidate = chars[start..end].iter().collect::<String>();
        if candidate.split('.').count() >= 2 {
            return Some(candidate);
        }
    }

    None
}

fn preferred_source_is_more_specific(candidate: Option<&str>, current: Option<&str>) -> bool {
    let candidate = candidate.unwrap_or_default();
    let current = current.unwrap_or_default();

    source_specificity(candidate) > source_specificity(current)
}

fn lockfile_package_compatibility(
    manifest_package: Option<&PreferredPackage>,
    lockfile_package: &PreferredPackage,
) -> usize {
    let mut score = source_specificity(lockfile_package.source.as_deref().unwrap_or_default());

    if let Some(manifest_package) = manifest_package {
        if source_kind(manifest_package.source.as_deref()) == source_kind(lockfile_package.source.as_deref()) {
            score += 100;
        }

        if manifest_package.version.is_some() && manifest_package.version == lockfile_package.version {
            score += 50;
        }
    }

    score
}

fn source_kind(source: Option<&str>) -> &'static str {
    let source = source.unwrap_or_default();

    if source.starts_with("git+") {
        "git"
    } else if source.starts_with("registry+") {
        "registry"
    } else if source.starts_with("path+") {
        "path"
    } else {
        ""
    }
}

fn source_specificity(source: &str) -> usize {
    let mut score = 0usize;

    if !source.is_empty() {
        score += 1;
    }
    if source.contains("://") {
        score += 1;
    }
    if source.contains('?') {
        score += 1;
    }
    if source.contains('#') {
        score += 2;
    }

    score
}

fn package_match_score(normalized_path: &str, preferred_package: &PreferredPackage) -> i32 {
    let mut score = 0;
    let package_name = normalize(&preferred_package.name);

    if normalized_path.contains(&format!("/{package_name}-")) {
        score += 10;
    }

    if let Some(version) = &preferred_package.version {
        let package_version = normalize(version);

        if normalized_path.contains(&format!("/{package_name}-{package_version}/")) {
            score += 400;
        } else if normalized_path.contains(&format!("/{package_name}-")) {
            score -= 80;
        }
    }

    if let Some(source) = &preferred_package.source {
        if source.starts_with("registry+") {
            if normalized_path.contains("/registry/src/") {
                score += 80;
            }

            if normalized_path.contains("/git/checkouts/") {
                score -= 120;
            }
        } else if source.starts_with("path+") {
            if normalized_path.contains("/registry/src/") || normalized_path.contains("/git/checkouts/") {
                score -= 120;
            }
        } else if source.starts_with("git+") {
            if normalized_path.contains("/git/checkouts/") {
                score += 80;
            }

            if normalized_path.contains("/registry/src/") {
                score -= 120;
            }

            if let Some(rev) = source.split('#').nth(1) {
                let short_rev = normalize(&rev[..rev.len().min(7)]);
                if normalized_path.contains(&format!("/{short_rev}/")) {
                    score += 500;
                } else if normalized_path.contains("/git/checkouts/") {
                    score -= 100;
                }
            }
        }
    }

    score
}

fn resolve_relative_to_manifest(path: &str) -> PathBuf {
    let p = PathBuf::from(path);

    if p.is_absolute() {
        return p;
    }

    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));

    manifest_dir.join(p)
}

fn normalize(s: &str) -> String {
    s.to_ascii_lowercase().replace('\\', "/").replace('_', "-")
}

fn to_snake_case(s: &str) -> String {
    let mut out = String::new();

    for (i, ch) in s.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mirror_args() {
        let args = syn::parse_str::<MirrorArgs>(
            "keepass::db::Entry, file = \"entry.rs\", root = \"vendor/a\", root = \"vendor/b\", import = \"keepass::db::*\", import = \"std::collections::*\", max_depth = 20",
        )
        .expect("args parse");

        assert_eq!(args.target.segments.last().unwrap().ident, "Entry");
        assert_eq!(args.file.as_ref().unwrap().value(), "entry.rs");
        assert_eq!(args.roots.len(), 2);
        assert_eq!(args.imports.len(), 2);
        assert_eq!(args.max_depth, 20);
    }

    #[test]
    fn rejects_unknown_option() {
        let err = syn::parse_str::<MirrorArgs>("keepass::db::Entry, nope = \"x\"").unwrap_err();
        assert!(err.to_string().contains("unknown mirror_from option"));
    }

    #[test]
    fn finds_struct_fields_in_nested_module() {
        let file = syn::parse_file(
            r#"
            mod outer {
                pub struct Entry {
                    pub alpha: String,
                    pub beta: usize,
                }
            }
            "#,
        )
        .expect("file parse");

        let fields = find_struct_fields(&file.items, "Entry").expect("fields");
        let names = fields
            .iter()
            .map(|field| field.ident.as_ref().unwrap().to_string())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn rejects_non_unit_struct_input() {
        let args = syn::parse_str::<MirrorArgs>("keepass::db::Entry").expect("args");
        let item = syn::parse_str::<ItemStruct>("struct EntryMirror {}").expect("struct");
        let err = expand(args, item).unwrap_err();

        assert!(err.to_string().contains("expects a unit struct"));
    }

    #[test]
    fn scores_expected_paths() {
        let path = Path::new("/tmp/vendor/keepass/src/db/types/entry.rs");
        let score = score_path(
            path,
            &["keepass".into(), "db".into(), "Entry".into()],
            "entry",
            None,
        );

        assert!(score >= 170);
    }

    #[test]
    fn snake_case_handles_camel_case() {
        assert_eq!(to_snake_case("EntryMirror"), "entry_mirror");
        assert_eq!(to_snake_case("URLValue"), "u_r_l_value");
    }

    #[test]
    fn prefers_db_component_over_xml_db_substring_match() {
        let db_path = Path::new("/tmp/keepass-0.10.6/src/db/types/entry.rs");
        let xml_db_path = Path::new("/tmp/keepass-0.10.6/src/format/xml_db/entry.rs");
        let target = &["keepass".into(), "db".into(), "Entry".into()];

        let db_score = score_path(db_path, target, "entry", None);
        let xml_db_score = score_path(xml_db_path, target, "entry", None);

        assert!(db_score > xml_db_score);
    }

    #[test]
    fn prefers_selected_registry_version_from_lockfile() {
        let preferred = PreferredPackage {
            name: "keepass".into(),
            version: Some("0.12.4".into()),
            source: Some("registry+https://github.com/rust-lang/crates.io-index".into()),
        };
        let target = &["keepass".into(), "db".into(), "Entry".into()];

        let selected = Path::new(
            "/tmp/.cargo/registry/src/index.crates.io-xyz/keepass-0.12.4/src/db/types/entry.rs",
        );
        let old = Path::new(
            "/tmp/.cargo/registry/src/index.crates.io-xyz/keepass-0.10.6/src/db/types/entry.rs",
        );
        let git = Path::new(
            "/tmp/.cargo/git/checkouts/keepass-rs-hash/rev/src/db/types/entry.rs",
        );

        let selected_score = score_path(selected, target, "entry", Some(&preferred));
        let old_score = score_path(old, target, "entry", Some(&preferred));
        let git_score = score_path(git, target, "entry", Some(&preferred));

        assert!(selected_score > old_score);
        assert!(selected_score > git_score);
    }

    #[test]
    fn parses_matching_package_from_lockfile() {
        let temp_dir = env::temp_dir().join(format!(
            "mirror_from_macro_test_{}",
            std::process::id()
        ));
        let lockfile = temp_dir.join("Cargo.lock");
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        fs::write(
            &lockfile,
            r#"
[[package]]
name = "other"
version = "1.0.0"
source = "registry+https://example.invalid"

[[package]]
name = "keepass"
version = "0.12.4"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#,
        )
        .expect("write lockfile");

        let package = parse_lockfile_package(&lockfile, "keepass").expect("package");
        assert_eq!(package.name, "keepass");
        assert_eq!(package.version.as_deref(), Some("0.12.4"));
        assert_eq!(
            package.source.as_deref(),
            Some("registry+https://github.com/rust-lang/crates.io-index")
        );

        let _ = fs::remove_file(lockfile);
        let _ = fs::remove_dir(temp_dir);
    }

    #[test]
    fn parses_manifest_dependency_inline_table() {
        let temp_dir = env::temp_dir().join(format!(
            "mirror_from_macro_manifest_test_{}",
            std::process::id()
        ));
        let manifest = temp_dir.join("Cargo.toml");
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        fs::write(
            &manifest,
            r#"
[package]
name = "demo"

[dependencies]
keepass = { version = "0.12.4", features = ["totp"] }
"#,
        )
        .expect("write manifest");

        let package = parse_manifest_dependency(&manifest, "keepass").expect("package");
        assert_eq!(package.version.as_deref(), Some("0.12.4"));
        assert_eq!(package.source.as_deref(), Some("registry+"));

        let _ = fs::remove_file(manifest);
        let _ = fs::remove_dir(temp_dir);
    }

    #[test]
    fn manifest_dependency_beats_stale_lockfile() {
        let temp_dir = env::temp_dir().join(format!(
            "mirror_from_macro_resolve_test_{}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let manifest_path = temp_dir.join("Cargo.toml");
        let lockfile_path = temp_dir.join("Cargo.lock");

        fs::write(
            &manifest_path,
            r#"
[package]
name = "demo"

[dependencies]
keepass = { version = "0.12.4", features = ["totp"] }
"#,
        )
        .expect("write manifest");
        fs::write(
            &lockfile_path,
            r#"
[[package]]
name = "keepass"
version = "0.8.14"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#,
        )
        .expect("write lockfile");

        let package = resolve_preferred_package(&temp_dir, "keepass").expect("package");
        assert_eq!(package.version.as_deref(), Some("0.12.4"));
        assert_eq!(
            package.source.as_deref(),
            Some("registry+https://github.com/rust-lang/crates.io-index")
        );

        let _ = fs::remove_file(manifest_path);
        let _ = fs::remove_file(lockfile_path);
        let _ = fs::remove_dir(temp_dir);
    }

    #[test]
    fn merges_manifest_with_more_specific_lockfile_git_source() {
        let temp_dir = env::temp_dir().join(format!(
            "mirror_from_macro_git_resolve_test_{}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let manifest_path = temp_dir.join("Cargo.toml");
        let lockfile_path = temp_dir.join("Cargo.lock");

        fs::write(
            &manifest_path,
            r#"
[package]
name = "demo"

[dependencies]
keepass = { git = "https://github.com/sseemayer/keepass-rs", branch = "303-kdbx41-support" }
"#,
        )
        .expect("write manifest");
        fs::write(
            &lockfile_path,
            r#"
[[package]]
name = "keepass"
version = "0.0.0-placeholder-version"
source = "git+https://github.com/sseemayer/keepass-rs?branch=303-kdbx41-support#0e924b43b81878fe310d9c8dd4a7b1779ebadef5"
"#,
        )
        .expect("write lockfile");

        let package = resolve_preferred_package(&temp_dir, "keepass").expect("package");
        assert_eq!(package.version.as_deref(), Some("0.0.0-placeholder-version"));
        assert_eq!(
            package.source.as_deref(),
            Some("git+https://github.com/sseemayer/keepass-rs?branch=303-kdbx41-support#0e924b43b81878fe310d9c8dd4a7b1779ebadef5")
        );

        let _ = fs::remove_file(manifest_path);
        let _ = fs::remove_file(lockfile_path);
        let _ = fs::remove_dir(temp_dir);
    }

    #[test]
    fn prefers_locked_git_revision_checkout() {
        let preferred = PreferredPackage {
            name: "keepass".into(),
            version: Some("0.0.0-placeholder-version".into()),
            source: Some(
                "git+https://github.com/sseemayer/keepass-rs?branch=303-kdbx41-support#0e924b43b81878fe310d9c8dd4a7b1779ebadef5"
                    .into(),
            ),
        };
        let target = &["keepass".into(), "db".into(), "Entry".into()];

        let selected = Path::new(
            "/tmp/cargo/git/checkouts/keepass-rs-deb45182a93bbe71/0e924b4/src/db/types/entry.rs",
        );
        let stale = Path::new(
            "/tmp/cargo/git/checkouts/keepass-rs-b78f56cfd0c013ed/7528c9d/src/db/types/entry.rs",
        );

        let selected_score = score_path(selected, target, "entry", Some(&preferred));
        let stale_score = score_path(stale, target, "entry", Some(&preferred));

        assert!(selected_score > stale_score);
    }

    #[test]
    fn rejects_unrelated_entry_files_early() {
        let preferred = PreferredPackage {
            name: "keepass".into(),
            version: Some("0.12.4".into()),
            source: Some("registry+https://github.com/rust-lang/crates.io-index".into()),
        };
        let target = &["keepass".into(), "db".into(), "Entry".into()];

        assert!(path_matches_target_namespace(
            Path::new(
                "/tmp/.cargo/registry/src/index.crates.io-xyz/keepass-0.12.4/src/db/types/entry.rs"
            ),
            target,
            Some(&preferred),
        ));
        assert!(!path_matches_target_namespace(
            Path::new("/tmp/.cargo/registry/src/index.crates.io-xyz/tar-0.4.45/src/entry.rs"),
            target,
            Some(&preferred),
        ));
        assert!(!path_matches_target_namespace(
            Path::new(
                "/tmp/.cargo/registry/src/index.crates.io-xyz/tokio-1.52.2/src/runtime/time_alt/entry.rs"
            ),
            target,
            Some(&preferred),
        ));
    }
}
