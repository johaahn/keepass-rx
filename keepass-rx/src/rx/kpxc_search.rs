use deunicode::deunicode_with_tofu_cow;
use regex::RegexBuilder;
use unicase::UniCase;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{RxDatabase, RxEntry};

#[derive(Debug, Clone)]
struct QueryToken {
    operator: Option<String>,
    term: QueryTokenTerm,
}

#[derive(Debug, Clone)]
enum QueryTokenTerm {
    Basic(String),
    Regex(String),
    Negated(String),
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

    let token_term = match term {
        term if is_regex => QueryTokenTerm::Regex(term),
        term if is_negated => QueryTokenTerm::Negated(term),
        term => QueryTokenTerm::Basic(term),
    };

    Some(QueryToken {
        operator: field,
        term: token_term,
    })
}

fn operator_field(entry: &RxEntry, op: &str, db: &RxDatabase) -> Vec<Zeroizing<String>> {
    match op.to_lowercase().as_str() {
        "title" | "t" => entry.title().and_then(|v| v.value()).into_iter().collect(),
        "user" | "u" => entry
            .username()
            .and_then(|v| v.value())
            .into_iter()
            .collect(),
        "url" => entry.url().and_then(|v| v.value()).into_iter().collect(),
        "notes" | "n" => entry.notes().and_then(|v| v.value()).into_iter().collect(),
        "password" | "p" | "pw" => entry
            .password()
            .and_then(|v| v.value())
            .into_iter()
            .collect(),
        "group" | "g" => db
            .get_group(entry.parent_group)
            .map(|g| Zeroizing::new(g.name.to_string()))
            .into_iter()
            .collect(),
        "tag" | "tags" => entry
            .tags()
            .iter()
            .map(|t| Zeroizing::new(t.to_string()))
            .collect(),
        "uuid" => vec![Zeroizing::new(entry.uuid.to_string())],
        "is" => {
            let is_expired = entry.is_expired();
            let is_weak = entry.is_password_weak();

            let mut vals = vec![];
            if is_expired {
                vals.push(Zeroizing::new("expired".to_string()));
            }
            if is_weak {
                vals.push(Zeroizing::new("weak".to_string()));
            }
            vals
        }
        _ => vec![],
    }
}

fn entry_default_fields(entry: &RxEntry, db: &RxDatabase) -> Vec<Zeroizing<String>> {
    let title = entry.title().and_then(|v| v.value());
    let username = entry.username().and_then(|v| v.value());
    let url = entry.url().and_then(|v| v.value());
    let notes = entry.notes().and_then(|v| v.value());
    let tags = entry.tags().iter().map(|t| Zeroizing::new(t.to_string()));
    let group_name = db
        .get_group(entry.parent_group)
        .map(|group| Zeroizing::new(group.name.clone()));

    vec![title, username, url, notes, group_name]
        .into_iter()
        .flatten()
        .chain(tags)
        .collect()
}

fn term_matches(token: &QueryToken, value: &str) -> bool {
    match token.term {
        QueryTokenTerm::Regex(ref regex) => RegexBuilder::new(regex)
            .case_insensitive(true)
            .build()
            .map(|regex| regex.is_match(value))
            .unwrap_or(false),
        QueryTokenTerm::Negated(ref term) => !normalized(value).contains(&normalized(term)),
        QueryTokenTerm::Basic(ref term) => normalized(value).contains(&normalized(term)),
    }
}

fn entry_matches(db: &RxDatabase, entry: &RxEntry, tokens: &[QueryToken]) -> bool {
    tokens.iter().all(|token| {
        let haystack = match token.operator.as_deref() {
            Some(op) => operator_field(entry, op, db),
            None => entry_default_fields(entry, db),
        };

        haystack.iter().any(|field| term_matches(token, field))
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
        .filter_map(|entry| match entry_matches(db, entry.as_ref(), &tokens) {
            true => Some(entry.uuid),
            false => None,
        })
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
        entry1.fields.insert(
            "Title".into(),
            keepass::db::Value::Unprotected("Gmail".into()),
        );
        entry1.fields.insert(
            "UserName".into(),
            keepass::db::Value::Unprotected("alice".into()),
        );
        entry1.fields.insert(
            "URL".into(),
            keepass::db::Value::Unprotected("gmail.com".into()),
        );

        let mut entry2 = keepass::db::Entry::new();
        entry2.fields.insert(
            "Title".into(),
            keepass::db::Value::Unprotected("GitLab".into()),
        );
        entry2.fields.insert(
            "UserName".into(),
            keepass::db::Value::Unprotected("bob".into()),
        );

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
