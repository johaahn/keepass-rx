use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use indexmap::{IndexMap, IndexSet};
use std::{cmp::Ordering, ops::Deref, rc::Rc};
use unicase::UniCase;
use uuid::Uuid;

use super::{RxContainedRef, RxDatabase, RxEntry, RxGroup, RxTag, RxTemplate};

#[cfg(feature = "gui")]
use qmetaobject::{QEnum, QMetaType, QString};

#[cfg_attr(feature = "gui", derive(QEnum))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum RxSearchType {
    CaseInsensitive,
    #[default]
    Fuzzy,
}

#[cfg(feature = "gui")]
fn search_type_from_string(qval: &QString) -> RxSearchType {
    match qval.to_string().as_str() {
        "CaseInsensitive" => RxSearchType::CaseInsensitive,
        "Fuzzy" => RxSearchType::Fuzzy,
        _ => panic!("Invalid search type: {}", qval),
    }
}

#[cfg(feature = "gui")]
fn search_type_to_string(search_type: &RxSearchType) -> QString {
    match search_type {
        RxSearchType::CaseInsensitive => "CaseInsensitive",
        RxSearchType::Fuzzy => "Fuzzy",
    }
    .into()
}

#[cfg(feature = "gui")]
impl QMetaType for RxSearchType {
    const CONVERSION_FROM_STRING: Option<fn(&QString) -> Self> = Some(search_type_from_string);
    const CONVERSION_TO_STRING: Option<fn(&Self) -> QString> = Some(search_type_to_string);
}

// Types of searches. Each struct can implement the Search trait
// separately.
pub(super) struct CaseInsensitiveSearch<T>(pub T);
pub(super) struct FuzzySearch<T>(pub T);

impl<T> Deref for CaseInsensitiveSearch<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Deref for FuzzySearch<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Determine if self matches a term, in some fashion (e.g. exact,
/// case insensitive, fuzzy, ... vector?).
pub(super) trait Search {
    fn matches(&self, term: &str) -> bool;
}

impl Search for CaseInsensitiveSearch<&RxTag> {
    fn matches(&self, term: &str) -> bool {
        UniCase::new(&self.name).to_folded_case().contains(term)
    }
}

impl Search for FuzzySearch<&RxTag> {
    fn matches(&self, term: &str) -> bool {
        SkimMatcherV2::default()
            .fuzzy_match(&self.name, term)
            .is_some()
    }
}

impl Search for CaseInsensitiveSearch<&RxGroup> {
    fn matches(&self, term: &str) -> bool {
        UniCase::new(&self.name).to_folded_case().contains(term)
    }
}

impl Search for FuzzySearch<&RxGroup> {
    fn matches(&self, term: &str) -> bool {
        SkimMatcherV2::default()
            .fuzzy_match(&self.name, term)
            .is_some()
    }
}

impl Search for CaseInsensitiveSearch<&RxTemplate> {
    fn matches(&self, term: &str) -> bool {
        UniCase::new(&self.name).to_folded_case().contains(term)
    }
}

impl Search for FuzzySearch<&RxTemplate> {
    fn matches(&self, term: &str) -> bool {
        SkimMatcherV2::default()
            .fuzzy_match(&self.name, term)
            .is_some()
    }
}

impl Search for CaseInsensitiveSearch<&RxEntry> {
    fn matches(&self, term: &str) -> bool {
        let username = self.username().and_then(|u| {
            u.value()
                .map(|secret| UniCase::new(secret).to_folded_case())
        });

        let url = self.url().and_then(|u| {
            u.value()
                .map(|secret| UniCase::new(secret).to_folded_case())
        });

        let title = self.title().and_then(|u| {
            u.value()
                .map(|secret| UniCase::new(secret).to_folded_case())
        });

        let contains_username = username.map(|u| u.contains(term)).unwrap_or(false);
        let contains_url = url.map(|u| u.contains(term)).unwrap_or(false);
        let contains_title = title.map(|t| t.contains(term)).unwrap_or(false);

        contains_username || contains_url || contains_title
    }
}

impl Search for FuzzySearch<&RxEntry> {
    fn matches(&self, term: &str) -> bool {
        let username = self.username().and_then(|u| {
            u.value()
                .map(|secret| UniCase::new(secret).to_folded_case())
        });

        let url = self.url().and_then(|u| {
            u.value()
                .map(|secret| UniCase::new(secret).to_folded_case())
        });

        let title = self.title().and_then(|u| {
            u.value()
                .map(|secret| UniCase::new(secret).to_folded_case())
        });

        let matcher = SkimMatcherV2::default();

        let username_matches = username
            .map(|ref u| matcher.fuzzy_match(u, term).is_some())
            .unwrap_or(false);

        let url_matches = url
            .map(|ref u| matcher.fuzzy_match(u, term).is_some())
            .unwrap_or(false);

        let title_matches = title
            .map(|ref t| matcher.fuzzy_match(t, term).is_some())
            .unwrap_or(false);

        username_matches || url_matches || title_matches
    }
}

impl Search for CaseInsensitiveSearch<&RxContainedRef> {
    fn matches(&self, term: &str) -> bool {
        match self.deref() {
            RxContainedRef::Entry(entry) => {
                CaseInsensitiveSearch(entry.as_ref()).matches(term)
            }
            RxContainedRef::Group(group) => {
                CaseInsensitiveSearch(group.as_ref()).matches(term)
            }
            RxContainedRef::Template(template) => {
                CaseInsensitiveSearch(template.as_ref()).matches(term)
            }
            RxContainedRef::Tag(tag) => CaseInsensitiveSearch(tag).matches(term),
            RxContainedRef::VirtualRoot(_) => true,
        }
    }
}

impl Search for FuzzySearch<&RxContainedRef> {
    fn matches(&self, term: &str) -> bool {
        match self.deref() {
            RxContainedRef::Entry(entry) => FuzzySearch(entry.as_ref()).matches(term),
            RxContainedRef::Group(group) => FuzzySearch(group.as_ref()).matches(term),
            RxContainedRef::Template(template) => FuzzySearch(template.as_ref()).matches(term),
            RxContainedRef::Tag(tag) => FuzzySearch(tag).matches(term),
            RxContainedRef::VirtualRoot(_) => true,
        }
    }
}

pub fn search_contained_ref(
    contained_ref: &RxContainedRef,
    search_type: RxSearchType,
    term: &str,
) -> bool {
    match search_type {
        RxSearchType::CaseInsensitive => CaseInsensitiveSearch(contained_ref).matches(term),
        RxSearchType::Fuzzy => FuzzySearch(contained_ref).matches(term),
    }
}
