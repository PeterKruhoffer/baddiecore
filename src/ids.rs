//! Identifiers and site paths. Each becomes its type only through `parse`, which
//! deserialization uses too, so a value from a request, a file or the database is
//! checked once at that boundary. Distinct ID types keep a component ID from being
//! used where a page ID belongs.
use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! item_id {
    ($name:ident, $error:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub(crate) fn new() -> Self {
                Self(Uuid::new_v4().to_string())
            }

            /// Accepts 1 to 255 characters, not all whitespace, to fit the `VARCHAR(255)` key.
            pub fn parse(value: &str) -> Option<Self> {
                (!value.trim().is_empty() && value.chars().count() <= 255)
                    .then(|| Self(value.to_owned()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = &'static str;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::parse(&value).ok_or($error)
            }
        }

        impl FromStr for $name {
            type Err = &'static str;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value).ok_or($error)
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl From<$name> for mysql::Value {
            fn from(id: $name) -> Self {
                id.0.into()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }
    };
}

item_id!(PageId, "page ids must contain 1 to 255 characters");
item_id!(TemplateId, "template ids must contain 1 to 255 characters");
item_id!(
    ComponentId,
    "component ids must contain 1 to 255 characters"
);

/// A canonical, case-sensitive absolute site path such as `/` or `/news/today`:
/// at most 2048 bytes, segments of ASCII letters, digits, `-` and `_`, and never
/// under a path the server reserves.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SitePath(String);

impl SitePath {
    pub fn root() -> Self {
        Self("/".into())
    }

    pub fn parse(value: &str) -> Option<Self> {
        if value.len() > 2048 {
            return None;
        }
        if value == "/" {
            return Some(Self::root());
        }
        let segments: Vec<_> = value.strip_prefix('/')?.split('/').collect();
        let valid = segments.iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        }) && !matches!(
            segments[0].to_ascii_lowercase().as_str(),
            "admin" | "api" | "health" | "assets" | "media"
        );
        valid.then(|| Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// True for paths strictly below `parent` by whole segments: `/news/a` is below
    /// `/news`, but `/newspaper` is not.
    pub fn is_descendant_of(&self, parent: &SitePath) -> bool {
        self != parent
            && (parent.0 == "/"
                || self
                    .0
                    .strip_prefix(&parent.0)
                    .is_some_and(|rest| rest.starts_with('/')))
    }

    /// Moves this path from below `from` to the same place below `to`.
    /// Returns `None` when it is not below `from` or the result is not a valid path.
    pub fn rebase(&self, from: &SitePath, to: &SitePath) -> Option<Self> {
        let rest = &self.0[from.0.len()..];
        self.is_descendant_of(from)
            .then(|| format!("{}{rest}", to.0.trim_end_matches('/')))
            .and_then(|path| Self::parse(&path))
    }
}

const PATH_ERROR: &str = "slug must be a safe canonical path of at most 2048 bytes";

impl TryFrom<String> for SitePath {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).ok_or(PATH_ERROR)
    }
}

impl FromStr for SitePath {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value).ok_or(PATH_ERROR)
    }
}

impl From<SitePath> for String {
    fn from(path: SitePath) -> Self {
        path.0
    }
}

impl From<SitePath> for mysql::Value {
    fn from(path: SitePath) -> Self {
        path.0.into()
    }
}

impl fmt::Display for SitePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl PartialEq<str> for SitePath {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for SitePath {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_canonical_and_avoid_reserved_prefixes() {
        for good in ["/", "/a", "/news/2026_q1", "/Admin-x", "/adminx"] {
            assert!(SitePath::parse(good).is_some(), "{good}");
        }
        let long = format!("/{}", "a".repeat(2048));
        for bad in [
            "", "a", "//a", "/a/", "/a//b", "/a/../b", "/x?q", "/ab c", "/é", "/admin", "/API/x",
            "/media", &long,
        ] {
            assert!(SitePath::parse(bad).is_none(), "{bad}");
        }
        assert!(serde_json::from_str::<SitePath>(r#""/admin""#).is_err());
    }

    #[test]
    fn descendants_match_whole_segments_and_rebase() {
        let p = |s| SitePath::parse(s).unwrap();
        assert!(p("/news/a").is_descendant_of(&p("/news")));
        assert!(!p("/newspaper").is_descendant_of(&p("/news")));
        assert!(!p("/news").is_descendant_of(&p("/news")));
        assert!(p("/news").is_descendant_of(&SitePath::root()));
        assert_eq!(
            p("/news/a/b").rebase(&p("/news"), &p("/blog")),
            Some(p("/blog/a/b"))
        );
        assert_eq!(
            p("/news/a").rebase(&p("/news"), &SitePath::root()),
            Some(p("/a"))
        );
        assert_eq!(p("/newspaper").rebase(&p("/news"), &p("/blog")), None);
    }

    #[test]
    fn ids_reject_blank_and_oversized_values() {
        assert!(PageId::parse("home").is_some());
        assert!(PageId::parse(&"é".repeat(255)).is_some());
        for bad in ["", "  ", &"a".repeat(256)] {
            assert!(PageId::parse(bad).is_none());
        }
        assert!(serde_json::from_str::<ComponentId>(r#""""#).is_err());
    }
}
