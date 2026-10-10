//! Tiptap JSON stored in the existing string-valued block fields. No raw HTML.
use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{ApiError, Result, validate_url};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    Bold,
    Italic,
    Underline,
    Strike,
    Heading,
    BulletList,
    OrderedList,
    Blockquote,
    Link,
}

fn default_features() -> Vec<Feature> {
    use Feature::*;
    vec![
        Bold,
        Italic,
        Underline,
        Strike,
        Heading,
        BulletList,
        OrderedList,
        Blockquote,
        Link,
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Icon {
    pub id: String,
    pub label: String,
    pub src: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_features")]
    pub features: Vec<Feature>,
    #[serde(default)]
    pub icons: Vec<Icon>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            features: default_features(),
            icons: vec![],
        }
    }
}

impl Config {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.features.iter().collect::<HashSet<_>>().len() != self.features.len() {
            return Err(ApiError::bad("richtext features must be unique"));
        }
        let mut ids = HashSet::new();
        for icon in &self.icons {
            if icon.id.is_empty()
                || icon.id.len() > 200
                || !icon
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                || !ids.insert(&icon.id)
                || icon.label.trim().is_empty()
            {
                return Err(ApiError::bad(
                    "richtext icons need unique safe IDs and labels",
                ));
            }
            validate_url(&icon.src)?;
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Node {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    content: Vec<Node>,
    text: Option<String>,
    #[serde(default)]
    attrs: Map<String, Value>,
    #[serde(default)]
    marks: Vec<Mark>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mark {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    attrs: Map<String, Value>,
}

fn invalid() -> ApiError {
    ApiError::bad("invalid richtext document or disabled formatting")
}

pub(crate) fn validate(value: &str, config: Option<&Config>) -> Result<bool> {
    // Legacy copy stays plain text when a textarea becomes a richtext field.
    // Editor saves thereafter always use a JSON document.
    if !value.trim_start().starts_with('{') {
        return Ok(!value.trim().is_empty());
    }
    let node: Node = serde_json::from_str(value).map_err(|_| invalid())?;
    if node.kind != "doc" {
        return Err(invalid());
    }
    validate_node(&node, config.unwrap_or(&Config::default()), 0)
}

fn validate_node(node: &Node, config: &Config, depth: usize) -> Result<bool> {
    use Feature::*;
    if depth > 20 {
        return Err(invalid());
    }
    let feature = match node.kind.as_str() {
        "heading" => Some(Heading),
        "bulletList" => Some(BulletList),
        "orderedList" => Some(OrderedList),
        "blockquote" => Some(Blockquote),
        _ => None,
    };
    if feature.is_some_and(|f| !config.features.contains(&f)) {
        return Err(invalid());
    }
    let block = |n: &Node| {
        matches!(
            n.kind.as_str(),
            "paragraph" | "heading" | "bulletList" | "orderedList" | "blockquote"
        )
    };
    let inline = |n: &Node| matches!(n.kind.as_str(), "text" | "hardBreak" | "icon");
    let valid_children = match node.kind.as_str() {
        "doc" => depth == 0 && !node.content.is_empty() && node.content.iter().all(block),
        "paragraph" | "heading" => node.content.iter().all(inline),
        "blockquote" => !node.content.is_empty() && node.content.iter().all(block),
        "bulletList" | "orderedList" => {
            !node.content.is_empty() && node.content.iter().all(|n| n.kind == "listItem")
        }
        "listItem" => {
            node.content.first().is_some_and(|n| n.kind == "paragraph")
                && node.content.iter().all(block)
        }
        "text" => node.content.is_empty() && node.text.as_ref().is_some_and(|t| !t.is_empty()),
        "hardBreak" | "icon" => node.content.is_empty(),
        _ => false,
    };
    if !valid_children
        || (node.kind != "text" && node.text.is_some())
        || (!inline(node) && !node.marks.is_empty())
    {
        return Err(invalid());
    }
    match node.kind.as_str() {
        "heading"
            if node.attrs.len() == 1
                && matches!(node.attrs.get("level").and_then(Value::as_u64), Some(2 | 3)) => {}
        "orderedList"
            if node
                .attrs
                .keys()
                .all(|k| matches!(k.as_str(), "start" | "type"))
                && node
                    .attrs
                    .get("start")
                    .is_none_or(|v| v.as_u64().is_some_and(|n| n > 0 && n <= i32::MAX as u64))
                && node.attrs.get("type").is_none_or(|v| {
                    v.is_null() || matches!(v.as_str(), Some("1" | "a" | "A" | "i" | "I"))
                }) => {}
        "icon"
            if node.attrs.len() == 1
                && node
                    .attrs
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| config.icons.iter().any(|i| i.id == id)) => {}
        "heading" | "orderedList" | "icon" => return Err(invalid()),
        _ if !node.attrs.is_empty() => return Err(invalid()),
        _ => {}
    }
    let mut marks = HashSet::new();
    for mark in &node.marks {
        let feature = match mark.kind.as_str() {
            "bold" => Bold,
            "italic" => Italic,
            "underline" => Underline,
            "strike" => Strike,
            "link" => Link,
            _ => return Err(invalid()),
        };
        if !config.features.contains(&feature) || !marks.insert(feature) {
            return Err(invalid());
        }
        if feature == Link {
            if !mark
                .attrs
                .keys()
                .all(|k| matches!(k.as_str(), "href" | "target" | "rel" | "class" | "title"))
                || mark.attrs.values().any(|v| !v.is_null() && !v.is_string())
            {
                return Err(invalid());
            }
            validate_url(
                mark.attrs
                    .get("href")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid)?,
            )?;
        } else if !mark.attrs.is_empty() {
            return Err(invalid());
        }
    }
    let mut has_content =
        node.kind == "icon" || node.text.as_ref().is_some_and(|t| !t.trim().is_empty());
    for child in &node.content {
        has_content |= validate_node(child, config, depth + 1)?;
    }
    Ok(has_content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn document(inline: Value) -> String {
        json!({"type":"doc","content":[{"type":"paragraph","content":[inline]}]}).to_string()
    }

    #[test]
    fn validates_real_content_and_legacy_text_without_accepting_empty_markup() {
        assert!(validate("Existing <b>literal</b> copy", None).unwrap());
        assert!(!validate(&document(json!({"type":"text","text":" \n "})), None).unwrap());
        assert!(!validate(&document(json!({"type":"hardBreak"})), None).unwrap());
        assert!(
            validate(
                &document(json!({"type":"text","text":"Copy","marks":[{"type":"bold"}]})),
                None
            )
            .unwrap()
        );
        assert!(validate("{broken", None).is_err());
    }

    #[test]
    fn disabled_features_and_unregistered_icons_are_rejected() {
        let mut config = Config {
            features: vec![Feature::Italic],
            icons: vec![Icon {
                id: "star".into(),
                label: "Star".into(),
                src: "/assets/star.svg".into(),
            }],
        };
        let bold = document(json!({"type":"text","text":"Copy","marks":[{"type":"bold"}]}));
        assert!(validate(&bold, Some(&config)).is_err());
        assert!(
            validate(
                &document(json!({"type":"icon","attrs":{"id":"star"}})),
                Some(&config)
            )
            .unwrap()
        );
        assert!(
            validate(
                &document(json!({"type":"icon","attrs":{"id":"other"}})),
                Some(&config)
            )
            .is_err()
        );
        config.icons[0].src = "javascript:alert(1)".into();
        assert!(config.validate().is_err());
        config.icons[0].src = "//evil.test/icon.svg".into();
        assert!(config.validate().is_err());
        config.icons[0].src = "/assets/star.svg".into();
        config.icons.push(config.icons[0].clone());
        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_unsafe_links_unknown_nodes_attributes_and_wrong_structure() {
        for href in [
            "javascript:alert(1)",
            "//evil.test",
            "https://good.test\\@evil.test",
            "java\nscript:alert(1)",
        ] {
            assert!(validate(&document(json!({"type":"text","text":"Link","marks":[{"type":"link","attrs":{"href":href}}]})), None).is_err());
        }
        for href in ["/about", "https://example.com/path?q=1"] {
            assert!(validate(&document(json!({"type":"text","text":"Link","marks":[{"type":"link","attrs":{"href":href,"target":"_blank","rel":"noopener noreferrer","class":null}}]})), None).unwrap());
        }
        for node in [
            json!({"type":"script","text":"alert(1)"}),
            json!({"type":"text","text":"Copy","attrs":{"onclick":"evil"}}),
            json!({"type":"paragraph"}),
            json!({"type":"text","text":"Copy","marks":[{"type":"code"}]}),
        ] {
            assert!(validate(&document(node), None).is_err());
        }
        assert!(
            validate(
                &json!({"type":"doc","content":[{"type":"heading","attrs":{"level":1}}]})
                    .to_string(),
                None
            )
            .is_err()
        );
        let mut nested = json!({"type":"paragraph"});
        for _ in 0..22 {
            nested = json!({"type":"blockquote","content":[nested]});
        }
        assert!(validate(&json!({"type":"doc","content":[nested]}).to_string(), None).is_err());
    }
}
