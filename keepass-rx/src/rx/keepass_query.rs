use deunicode::deunicode_with_tofu_cow;
use regex::RegexBuilder;
use unicase::UniCase;
use uuid::Uuid;

use super::{RxDatabase, RxEntry};

#[derive(Debug, Clone)]
struct QueryToken {
    field: Option<String>,
    term: String,
    is_regex: bool,
    is_negated: bool,
}

fn normalized(value: &str) -> String {
    UniCase::new(deunicode_with_tofu_cow(value, "u{FFFD}"))
        .to_folded_case()
        .to_string()
}

fn tokenize(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut curr = String::new();
    let mut in_quotes = false;
    let mut escaped = false;

    for ch in input.chars() {
        if escaped {
            if ch == '"' || ch == '\\' {
                curr.push(ch);
            } else {
                curr.push('\\');
                curr.push(ch);
            }
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !curr.trim().is_empty() {
                    out.push(curr.trim().to_string());
                }
                curr.clear();
            }
            c => curr.push(c),
        }
    }

    if escaped {
        curr.push('\\');
    }

    if !curr.trim().is_empty() {
        out.push(curr.trim().to_string());
    }

    out
}

fn parse_token(token: &str) -> Option<QueryToken> {
    let mut is_regex = false;
    let mut is_negated = false;

    let mut idx = 0usize;
    for (i, ch) in token.char_indices() {
        match ch {
            '+' => idx = i + 1,
            '-' => {
                is_negated = true;
                idx = i + 1;
            }
            '*' => {
                is_regex = true;
                idx = i + 1;
            }
            _ => break,
        }
    }

    let rest = token.get(idx..).unwrap_or("");
    if rest.is_empty() {
        return None;
    }

    let mut split = rest.splitn(2, ':');
    let maybe_field = split.next();
    let maybe_term = split.next();

    let (field, term) = match (maybe_field, maybe_term) {
        (Some(field), Some(term)) if !field.is_empty() => {
            (Some(field.to_ascii_lowercase()), term.to_string())
        }
        _ => (None, rest.to_string()),
    };

    if term.is_empty() {
        return None;
    }

    Some(QueryToken {
        field,
        term,
        is_regex,
        is_negated,
    })
}

fn entry_field(entry: &RxEntry, field: &str, db: &RxDatabase) -> Vec<String> {
    match field {
        "title" => entry
            .title()
            .and_then(|v| v.value().map(|v| v.to_string()))
            .into_iter()
            .collect(),
        "user" | "username" => entry
            .username()
            .and_then(|v| v.value().map(|v| v.to_string()))
            .into_iter()
            .collect(),
        "url" => entry
            .url()
            .and_then(|v| v.value().map(|v| v.to_string()))
            .into_iter()
            .collect(),
        "notes" => entry
            .notes()
            .and_then(|v| v.value().map(|v| v.to_string()))
            .into_iter()
            .collect(),
        "group" => db
            .get_group(entry.parent_group)
            .map(|g| g.name.to_string())
            .into_iter()
            .collect(),
        "tag" | "tags" => entry.tags().iter().map(|t| t.to_string()).collect(),
        "uuid" => vec![entry.uuid.to_string()],
        "is" => {
            let is_expired = entry.is_expired();
            let is_weak = entry.password_is_weak();

            let mut vals = vec![];
            if is_expired {
                vals.push("expired".to_string());
            }
            if is_weak {
                vals.push("weak".to_string());
            }
            vals
        }
        _ => vec![],
    }
}

fn entry_default_fields(entry: &RxEntry, db: &RxDatabase) -> Vec<String> {
    let mut fields = vec![];

    if let Some(title) = entry.title().and_then(|v| v.value().map(|v| v.to_string())) {
        fields.push(title);
    }
    if let Some(user) = entry
        .username()
        .and_then(|v| v.value().map(|v| v.to_string()))
    {
        fields.push(user);
    }
    if let Some(url) = entry.url().and_then(|v| v.value().map(|v| v.to_string())) {
        fields.push(url);
    }
    if let Some(notes) = entry.notes().and_then(|v| v.value().map(|v| v.to_string())) {
        fields.push(notes);
    }
    if let Some(group) = db.get_group(entry.parent_group) {
        fields.push(group.name.to_string());
    }
    fields.extend(entry.tags().iter().map(|t| t.to_string()));

    fields
}

fn term_matches(token: &QueryToken, value: &str) -> bool {
    if token.is_regex {
        RegexBuilder::new(&token.term)
            .case_insensitive(true)
            .build()
            .map(|regex| regex.is_match(value))
            .unwrap_or(false)
    } else {
        normalized(value).contains(&normalized(&token.term))
    }
}

fn entry_matches(db: &RxDatabase, entry: &RxEntry, tokens: &[QueryToken]) -> bool {
    tokens.iter().all(|token| {
        let haystack: Vec<String> = match token.field.as_deref() {
            Some(field) => entry_field(entry, field, db),
            None => entry_default_fields(entry, db),
        };

        let does_match = haystack.iter().any(|field| term_matches(token, field));

        if token.is_negated {
            !does_match
        } else {
            does_match
        }
    })
}

pub fn evaluate_saved_search(db: &RxDatabase, query: &str) -> Vec<Uuid> {
    let tokens: Vec<_> = tokenize(query)
        .into_iter()
        .flat_map(|token| parse_token(&token))
        .collect();

    if tokens.is_empty() {
        return vec![];
    }

    db.all_entries_iter()
        .filter(|entry| entry_matches(db, entry.as_ref(), &tokens))
        .map(|entry| entry.uuid)
        .collect()
}

#[cfg(test)]
mod tests {
    use keepass::db::Node;
    use keyring::set_default_credential_builder;
    use zeroize::Zeroizing;

    use crate::rx::ZeroableDatabase;

    use super::*;

    fn test_db() -> RxDatabase {
        set_default_credential_builder(keyring::mock::default_credential_builder());

        let mut db = keepass::db::Database::new(Default::default());
        let mut root = keepass::db::Group::new("Root");
        let mut child = keepass::db::Group::new("Email");

        let mut entry1 = keepass::db::Entry::new();
        entry1
            .fields
            .insert("Title".into(), keepass::db::Value::Unprotected("Gmail".into()));
        entry1
            .fields
            .insert("UserName".into(), keepass::db::Value::Unprotected("alice".into()));
        entry1
            .fields
            .insert("URL".into(), keepass::db::Value::Unprotected("gmail.com".into()));

        let mut entry2 = keepass::db::Entry::new();
        entry2
            .fields
            .insert("Title".into(), keepass::db::Value::Unprotected("GitLab".into()));
        entry2
            .fields
            .insert("UserName".into(), keepass::db::Value::Unprotected("bob".into()));

        child.add_child(Node::Entry(entry1));
        root.add_child(Node::Group(child));
        root.add_child(Node::Entry(entry2));
        db.root = root;

        RxDatabase::new(Zeroizing::new(ZeroableDatabase(db)))
    }

    #[test]
    fn field_query_matches_expected_entry() {
        let db = test_db();
        let results = evaluate_saved_search(&db, "title:Gmail user:alice");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn negated_query_filters_entries() {
        let db = test_db();
        let results = evaluate_saved_search(&db, "title:Git -user:bob");
        assert_eq!(results.len(), 0);
    }
}
