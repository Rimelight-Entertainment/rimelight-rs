//! Serde types for the published JSON (MAPPING section 1).
//!
//! Block props stay raw JSON on purpose: a page with an unknown block type or an unknown inline node must still
//! parse. Rich text is only interpreted inside the mapping (`richtext`, `normalize`), never here.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// A block in the published tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub id: String,
    #[serde(rename = "type")]
    pub block_type: String,
    pub props: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Block>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_templated: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub templated_props: Option<TemplatedProps>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_roles: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_permission: Option<String>,
    /// Absent means inherit the page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
}

/// `templatedProps`: every prop locked (`true`), or the listed prop keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TemplatedProps {
    All(bool),
    Keys(Vec<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Internal,
    Public,
}

/// Inline text marks. The derived order is the canonical order used by the normal form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mark {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Code,
}

/// One inline node. Delivery-only fields (`color`, `href`) are ignored when parsing and never produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum InlineNode {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        marks: Vec<Mark>,
    },
    Link {
        url: String,
        children: Vec<InlineNode>,
        #[serde(
            default,
            rename = "openInNewTab",
            skip_serializing_if = "Option::is_none"
        )]
        open_in_new_tab: Option<bool>,
    },
    Mention {
        #[serde(rename = "entryId")]
        entry_id: String,
        collection: String,
        slug: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    Emoji {
        emoji: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shortcode: Option<String>,
    },
    Icon {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
}

/// Rich text: a node list, or a per-locale map of node lists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RichTextValue {
    Nodes(Vec<InlineNode>),
    Localized(BTreeMap<String, Vec<InlineNode>>),
}

/// Page metadata (`meta` in the published JSON).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageMeta {
    pub slug: String,
    #[serde(rename = "type")]
    pub page_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_version: Option<u64>,
    pub title: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<BTreeMap<String, String>>>,
}

/// Page content: the block tree and the page properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageContent {
    pub blocks: Vec<Block>,
    pub properties: Map<String, Value>,
}

/// A whole published page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub meta: PageMeta,
    pub content: PageContent,
}
