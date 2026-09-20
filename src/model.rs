//! Core data model for the sticky notes application.
//!
//! The model mirrors `docs/03-开发实现路径.md` exactly: a [`Note`] is a list of
//! [`Block`]s, where each block is either free-form [`BlockType::Text`] or a
//! [`BlockType::Todo`] item with an optional checked state.

use serde::{Deserialize, Serialize};

/// The kind of a [`Block`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum BlockType {
    Text,
    Todo,
}

/// A single content block: a text line or a todo line.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Block {
    pub id: String,
    pub block_type: BlockType,
    pub content: String,
    /// Only meaningful for [`BlockType::Todo`]; `None` for text blocks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
}

/// A sticky note and everything needed to restore it on startup.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Note {
    pub id: String,
    /// Logical screen position of the top-left corner.
    pub position: (i32, i32),
    /// Logical window size in pixels.
    pub size: (u32, u32),
    pub always_on_top: bool,
    #[serde(default)]
    pub blocks: Vec<Block>,
}

impl Note {
    pub fn new(id: String, position: (i32, i32)) -> Self {
        let block_id = format!("{id}-block-0");
        Self {
            id,
            position,
            size: (280, 360),
            always_on_top: false,
            blocks: vec![Block {
                id: block_id,
                block_type: BlockType::Text,
                content: String::new(),
                checked: None,
            }],
        }
    }

    /// Serializes the block list back to the plain-text form used when the note
    /// content is edited as a single document. Todo blocks are rendered as
    /// `- [ ] content` / `- [x] content`.
    #[allow(dead_code)]
    pub fn plain_text(&self) -> String {
        self.blocks
            .iter()
            .map(|block| match block.block_type {
                BlockType::Text => block.content.clone(),
                BlockType::Todo => format!(
                    "- [{}] {}",
                    if block.checked.unwrap_or(false) { 'x' } else { ' ' },
                    block.content
                ),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Rebuilds the block list from a plain-text document, recognizing
    /// `- [ ] ` / `- [x] ` / `- [X] ` prefixes as todo blocks.
    #[allow(dead_code)]
    pub fn from_text(&mut self, text: &str) {
        self.blocks = text
            .lines()
            .enumerate()
            .map(|(index, line)| {
                let trimmed = line.trim_start();
                let is_todo = trimmed.starts_with("- [ ] ")
                    || trimmed.starts_with("- [x] ")
                    || trimmed.starts_with("- [X] ");
                let (block_type, content, checked) = if is_todo {
                    (
                        BlockType::Todo,
                        trimmed[6..].to_string(),
                        Some(trimmed.as_bytes()[3].eq_ignore_ascii_case(&b'x')),
                    )
                } else {
                    (BlockType::Text, line.to_string(), None)
                };
                Block {
                    id: format!("block-{index}"),
                    block_type,
                    content,
                    checked,
                }
            })
            .collect();
        if self.blocks.is_empty() {
            self.blocks.push(Block {
                id: "block-0".into(),
                block_type: BlockType::Text,
                content: String::new(),
                checked: None,
            });
        }
    }
}

/// The serialized application state.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AppState {
    pub notes: Vec<Note>,
}

#[cfg(test)]
mod tests {
    use super::{BlockType, Note};

    #[test]
    fn parses_and_serializes_mixed_text() {
        let mut note = Note::new("test".into(), (0, 0));
        note.from_text("标题\n- [ ] 买牛奶\n- [x] 写代码");
        assert_eq!(note.blocks.len(), 3);
        assert_eq!(note.blocks[1].block_type, BlockType::Todo);
        assert_eq!(note.blocks[1].checked, Some(false));
        assert_eq!(note.blocks[2].checked, Some(true));
        assert_eq!(note.plain_text(), "标题\n- [ ] 买牛奶\n- [x] 写代码");
    }

    #[test]
    fn empty_text_keeps_an_editable_block() {
        let mut note = Note::new("test".into(), (0, 0));
        note.from_text("");
        assert_eq!(note.blocks.len(), 1);
        assert!(note.blocks[0].content.is_empty());
        assert_eq!(note.blocks[0].checked, None);
    }
}
