use crate::harness::*;
use base64::Engine;
use chrono::{DateTime, NaiveDate};
use keepassrx::rx::NoHooks;
use std::collections::BTreeMap;

struct Element {
    attributes: Vec<(String, String)>,
    text: String,
}

type Elements = BTreeMap<String, Element>;

fn epoch_seconds(text: &str) -> Option<i64> {
    let text = text.trim();
    if let Ok(time) = DateTime::parse_from_rfc3339(text) {
        return Some(time.timestamp());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text)
        .ok()?;
    let seconds = i64::from_le_bytes(bytes.try_into().ok()?);
    let origin = NaiveDate::from_ymd_opt(1, 1, 1)?.and_hms_opt(0, 0, 0)?;
    Some(origin.and_utc().timestamp() + seconds)
}

fn child_text(node: roxmltree::Node, name: &str) -> Option<String> {
    node.children()
        .find(|c| c.has_tag_name(name))
        .map(|c| c.text().unwrap_or_default().trim().to_string())
}

fn discriminator(node: roxmltree::Node, path: &str, index: usize) -> String {
    let name = node.tag_name().name();
    match name {
        "Entry" | "Group" | "DeletedObject" | "Icon" => {
            let uuid = child_text(node, "UUID").unwrap_or_default();
            if !path.contains("/History[") {
                return uuid;
            }
            let modified = node
                .children()
                .find(|c| c.has_tag_name("Times"))
                .and_then(|t| child_text(t, "LastModificationTime"))
                .unwrap_or_default();
            let modified = epoch_seconds(&modified).map_or(modified, |s| s.to_string());
            format!("{uuid}@{modified}")
        }
        "String" | "Binary" | "Item" if node.children().any(|c| c.has_tag_name("Key")) => {
            child_text(node, "Key").unwrap_or_default()
        }
        _ => index.to_string(),
    }
}

fn collect(node: roxmltree::Node, path: &str, out: &mut Elements) {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for child in node.children().filter(|c| c.is_element()) {
        let name = child.tag_name().name().to_string();
        let index = counts.entry(name.clone()).or_insert(0);
        let key = format!("{path}/{name}[{}]", discriminator(child, path, *index));
        *index += 1;
        let mut attributes: Vec<(String, String)> = child
            .attributes()
            .map(|a| (a.name().to_string(), a.value().to_string()))
            .collect();
        attributes.sort();
        let text = if child.children().any(|c| c.is_element()) {
            String::new()
        } else {
            child.text().unwrap_or_default().trim().to_string()
        };
        out.insert(key.clone(), Element { attributes, text });
        collect(child, &key, out);
    }
}

fn elements(xml: &str) -> Elements {
    let doc = roxmltree::Document::parse(xml).expect("xml");
    let mut out = Elements::new();
    collect(doc.root(), "", &mut out);
    out
}

fn leaf(key: &str) -> &str {
    let last = key.rsplit('/').next().unwrap_or(key);
    last.split('[').next().unwrap_or(last)
}

fn is_standard_icon_next_to_custom_icon(key: &str, elements: &Elements, text: &str) -> bool {
    if leaf(key) != "IconID" || text != "0" {
        return false;
    }
    let parent = key.rsplit_once('/').map_or("", |(parent, _)| parent);
    elements.keys().any(|k| {
        k.strip_prefix(parent)
            .is_some_and(|rest| rest.starts_with("/CustomIconUUID["))
    })
}

fn lost(before: &Elements, after: &Elements) -> Vec<String> {
    let mut problems = vec![];
    for (key, old) in before {
        if old.attributes.is_empty() && old.text.is_empty() {
            continue;
        }
        if is_standard_icon_next_to_custom_icon(key, before, &old.text) {
            continue;
        }
        let Some(new) = after.get(key) else {
            problems.push(format!("missing {key}"));
            continue;
        };
        if new.attributes != old.attributes {
            problems.push(format!(
                "attributes {key}: {:?} -> {:?}",
                old.attributes, new.attributes
            ));
        }
    }
    problems
}

#[test]
fn a_no_op_save_keeps_every_element_of_the_full_fixture() {
    let vault = TestVault::from_fixture(FULL);
    let before = elements(&vault.xml());
    assert!(before.len() > 100, "{}", before.len());
    vault.save(&vault.source(), None, &NoHooks).expect("save");
    let after = elements(&vault.xml());
    let problems = lost(&before, &after);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
