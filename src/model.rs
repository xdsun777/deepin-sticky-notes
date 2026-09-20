use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum BlockType {
    Text,
    Todo,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Block {
    pub id: String,
    pub block_type: BlockType,
    pub content: String,
    pub checked: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum NoteColor {
    Yellow,
    White,
    Blue,
}

impl Default for NoteColor {
    fn default() -> Self {
        Self::Yellow
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Note {
    pub id: String,
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub always_on_top: bool,
    pub color: NoteColor,
    pub blocks: Vec<Block>,
}

impl Note {
    pub fn new(id: String, position: (i32, i32)) -> Self {
        Self {
            id,
            position,
            size: (280, 360),
            always_on_top: false,
            color: NoteColor::default(),
            blocks: vec![Block {
                id: "block-1".to_string(),
                block_type: BlockType::Text,
                content: String::new(),
                checked: false,
            }],
        }
    }

    pub fn plain_text(&self) -> String {
        self.blocks
            .iter()
            .map(|block| match block.block_type {
                BlockType::Text => block.content.clone(),
                BlockType::Todo => format!(
                    "- [{}] {}",
                    if block.checked { 'x' } else { ' ' },
                    block.content
                ),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn from_text(&mut self, text: &str) {
        self.blocks = text
            .lines()
            .enumerate()
            .map(|(index, line)| {
                let trimmed = line.trim_start();
                let (block_type, content, checked) = if trimmed.starts_with("- [ ] ")
                    || trimmed.starts_with("- [x] ")
                    || trimmed.starts_with("- [X] ")
                {
                    (
                        BlockType::Todo,
                        trimmed[6..].to_string(),
                        trimmed.as_bytes()[3].eq_ignore_ascii_case(&b'x'),
                    )
                } else {
                    (BlockType::Text, line.to_string(), false)
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
                checked: false,
            });
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
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
        assert!(!note.blocks[1].checked);
        assert!(note.blocks[2].checked);
        assert_eq!(note.plain_text(), "标题\n- [ ] 买牛奶\n- [x] 写代码");
    }

    #[test]
    fn empty_text_keeps_an_editable_block() {
        let mut note = Note::new("test".into(), (0, 0));
        note.from_text("");
        assert_eq!(note.blocks.len(), 1);
        assert!(note.blocks[0].content.is_empty());
    }
}
