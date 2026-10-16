use std::collections::{BTreeMap, HashSet};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Error, Result};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub version: u8,
    pub title: String,
    pub root: Node,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub tag: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub styles: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

pub const TAGS: &[&str] = &[
    "div", "section", "header", "nav", "main", "footer", "article", "aside", "h1", "h2", "h3",
    "h4", "h5", "h6", "p", "a", "button", "span", "ul", "ol", "li", "img", "input", "label",
    "form", "textarea", "strong", "em", "small", "hr", "br",
];

pub const ATTRIBUTES: &[&str] = &[
    "href",
    "src",
    "alt",
    "type",
    "placeholder",
    "title",
    "aria-label",
    "role",
    "name",
    "value",
];

impl Page {
    pub fn revision(&self) -> Result<String> {
        Ok(URL_SAFE_NO_PAD.encode(Sha256::digest(serde_json::to_vec(self)?)))
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != 1 || self.title.len() > 300 {
            return Err(Error::Invalid("Unsupported or invalid Ripple page."));
        }
        let mut ids = HashSet::new();
        validate_node(&self.root, &mut ids, 0)
    }
}

fn validate_node<'a>(node: &'a Node, ids: &mut HashSet<&'a str>, depth: usize) -> Result<()> {
    if depth > 32 || ids.len() >= 500 {
        return Err(Error::Invalid(
            "The page exceeds Ripple's prototype limits.",
        ));
    }
    if node.id.is_empty()
        || node.id.len() > 80
        || !node
            .id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        || !ids.insert(&node.id)
    {
        return Err(Error::Invalid("Every element needs a unique, stable ID."));
    }
    if !TAGS.contains(&node.tag.as_str()) {
        return Err(Error::Invalid("Unsupported element tag."));
    }
    if ["img", "input", "hr", "br"].contains(&node.tag.as_str())
        && (!node.children.is_empty() || !node.text.is_empty())
    {
        return Err(Error::Invalid(
            "A void element cannot contain children or text.",
        ));
    }
    if node.text.len() > 20_000 || node.styles.len() > 80 || node.attributes.len() > 20 {
        return Err(Error::Invalid("Element content is too large."));
    }
    for (key, value) in &node.attributes {
        if !ATTRIBUTES.contains(&key.as_str()) || value.len() > 2_000 {
            return Err(Error::Invalid("Unsupported element attribute."));
        }
        if (key == "href" || key == "src") && !safe_url(value) {
            return Err(Error::Invalid("Unsupported element URL."));
        }
    }
    for (key, value) in &node.styles {
        if key.is_empty()
            || !key.bytes().all(|c| c.is_ascii_lowercase() || c == b'-')
            || value.len() > 2_000
        {
            return Err(Error::Invalid("Use CSS properties in kebab-case."));
        }
    }
    for child in &node.children {
        validate_node(child, ids, depth + 1)?;
    }
    Ok(())
}

fn safe_url(value: &str) -> bool {
    !value.chars().any(char::is_control)
        && !value.chars().any(char::is_whitespace)
        && (!value.contains(':') || value.starts_with("https://") || value.starts_with("http://"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page(root: serde_json::Value) -> Page {
        serde_json::from_value(json!({"version": 1, "title": "Test", "root": root})).unwrap()
    }

    #[test]
    fn rejects_duplicate_ids_and_executable_markup() {
        assert!(
            page(json!({"id":"root","tag":"div","children":[{"id":"root","tag":"p"}]}))
                .validate()
                .is_err()
        );
        assert!(
            page(json!({"id":"root","tag":"script"}))
                .validate()
                .is_err()
        );
        assert!(
            page(json!({"id":"root","tag":"a","attributes":{"href":"javascript:alert(1)"}}))
                .validate()
                .is_err()
        );
        assert!(
            page(json!({"id":"root","tag":"button","attributes":{"onclick":"alert(1)"}}))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn accepts_nested_page_and_rejects_children_in_void_elements() {
        page(json!({"id":"root","tag":"main","children":[{"id":"title","tag":"h1","text":"Ripple"}]})).validate().unwrap();
        assert!(
            page(json!({"id":"root","tag":"img","text":"invalid"}))
                .validate()
                .is_err()
        );
    }
}
