//! Global application state and orchestration.
//!
//! `App` owns the list of open note windows, the tray icon, the global hotkey
//! and the debounced save timer. It is the single place where the Slint UI and
//! the persisted [`crate::model`] meet.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use slint::{ComponentHandle, Model, ModelRc, Timer, TimerMode, VecModel, Weak};

use crate::model::{AppState, Block, BlockType, Note};
use crate::{hotkey, storage, tray};

slint::include_modules!();

/// Logical height of a single block row, used to translate a drag distance
/// into a number of rows moved.
const ROW_HEIGHT: f32 = 32.0;
const DEBOUNCE_MS: u64 = 500;
/// Window within which two tray clicks count as a double-click (new note).
const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(350);
/// Transparent margin around the note body reserved for its drop shadow. Must
/// match `shadow-margin` in `note_window.slint`.
const SHADOW_MARGIN: f32 = 20.0;

struct NoteHandle {
    id: String,
    window: NoteWindow,
    blocks: Rc<VecModel<BlockItem>>,
}

pub struct App {
    windows: HashMap<String, NoteHandle>,
    tray: Option<tray::Tray>,
    hotkey: Option<(global_hotkey::GlobalHotKeyManager, u32)>,
    dark_mode: bool,
    poll_timer: Timer,
    save_timer: Timer,
    /// Timestamp of a pending tray left-click, deferred to disambiguate it
    /// from a double-click.
    tray_click_pending: Option<Instant>,
    /// Primary monitor size in logical pixels, used to center new notes.
    screen_size: Option<(f32, f32)>,
}

impl App {
    pub fn new() -> Result<Self> {
        let mut app = Self {
            windows: HashMap::new(),
            tray: None,
            hotkey: None,
            dark_mode: false,
            poll_timer: Timer::default(),
            save_timer: Timer::default(),
            tray_click_pending: None,
            screen_size: screen_size(),
        };
        app.tray = tray::Tray::create().ok();
        app.hotkey = hotkey::register_new_note().ok().flatten();
        Ok(app)
    }

    /// Starts the recurring timers and restores previously saved notes.
    pub fn start(app: Rc<RefCell<App>>) -> Result<()> {
        {
            let a = app.clone();
            app.borrow()
                .save_timer
                .start(TimerMode::SingleShot, Duration::from_millis(DEBOUNCE_MS), move || {
                    a.borrow_mut().flush_save();
                });
        }
        {
            let a = app.clone();
            app.borrow()
                .poll_timer
                .start(TimerMode::Repeated, Duration::from_millis(120), move || {
                    a.borrow_mut().poll(a.clone());
                });
        }

        app.borrow_mut().restore(app.clone())?;
        Ok(())
    }

    fn restore(&mut self, app: Rc<RefCell<App>>) -> Result<()> {
        let mut state = storage::load().unwrap_or_default();
        if state.notes.is_empty() {
            state.notes.push(Note::new(
                format!("note-{}", timestamp_millis()),
                (200, 140),
            ));
        }
        let notes = std::mem::take(&mut state.notes);
        for note in notes {
            self.open_note(note, app.clone())?;
        }
        Ok(())
    }

    fn open_note(&mut self, note: Note, app: Rc<RefCell<App>>) -> Result<()> {
        let window = NoteWindow::new()?;
        window.set_note_id(note.id.clone().into());
        window.set_note_title("便签".into());
        window.set_pinned(note.always_on_top);
        window.global::<Theme>().set_dark_mode(self.dark_mode);

        let blocks: Rc<VecModel<BlockItem>> = Rc::new(VecModel::default());
        for b in &note.blocks {
            blocks.push(BlockItem {
                id: b.id.clone().into(),
                is_todo: b.block_type == BlockType::Todo,
                content: b.content.clone().into(),
                checked: b.checked.unwrap_or(false),
                rendered: render_markdown(&b.content),
            });
        }
        window.set_blocks(ModelRc::from(blocks.clone()));

        // Restore geometry. The persisted size/position describe the note body;
        // the window is larger by the shadow margin on each side.
        window.window().set_position(slint::WindowPosition::Logical(
            slint::LogicalPosition::new(
                note.position.0 as f32 - SHADOW_MARGIN,
                note.position.1 as f32 - SHADOW_MARGIN,
            ),
        ));
        window.window().set_size(slint::WindowSize::Logical(slint::LogicalSize::new(
            note.size.0 as f32 + 2.0 * SHADOW_MARGIN,
            note.size.1 as f32 + 2.0 * SHADOW_MARGIN,
        )));

        wire_callbacks(window.clone_strong(), blocks.clone(), note.id.clone(), app);
        window.show()?;
        self.windows.insert(
            note.id.clone(),
            NoteHandle {
                id: note.id,
                window,
                blocks,
            },
        );
        Ok(())
    }

    fn create_note(&mut self, app: Rc<RefCell<App>>) {
        let n = self.windows.len() as i32;
        // New notes appear at the screen center, slightly to the right, with a
        // small cascade so consecutive notes don't overlap exactly.
        let (base_x, base_y) = match self.screen_size {
            Some((w, h)) => (w * 0.58, h * 0.45),
            None => (240.0, 140.0),
        };
        let note = Note::new(
            format!("note-{}", timestamp_millis()),
            (base_x as i32 + n * 28, base_y as i32 + n * 28),
        );
        if let Err(err) = self.open_note(note, app) {
            eprintln!("failed to open new note: {err}");
            return;
        }
        self.schedule_save();
    }

    /// Permanently removes a note: fades its window out, then drops it and
    /// persists the change.
    fn delete_note(&mut self, id: &str) {
        if let Some(handle) = self.windows.remove(id) {
            handle.window.set_fade_opacity(0.0);
            Timer::single_shot(Duration::from_millis(160), move || {
                // Dropping the last strong handle closes the window.
                drop(handle);
            });
        }
        self.schedule_save();
    }

    fn show_all(&mut self) {
        for handle in self.windows.values() {
            show_note(&handle.window);
        }
    }

    fn hide_all(&mut self) {
        for handle in self.windows.values() {
            hide_note(&handle.window);
        }
    }

    fn toggle_all(&mut self) {
        let any_visible = self
            .windows
            .values()
            .any(|h| h.window.window().is_visible());
        if any_visible {
            self.hide_all();
        } else {
            self.show_all();
        }
    }

    /// Handles a left-click on the tray icon. A second click within the
    /// double-click window creates a note; otherwise the click toggles
    /// show/hide after a short deferral.
    fn on_tray_click(&mut self, app: Rc<RefCell<App>>) {
        let now = Instant::now();
        if let Some(prev) = self.tray_click_pending.take() {
            if now.duration_since(prev) < DOUBLE_CLICK_WINDOW {
                self.create_note(app);
                return;
            }
        }
        self.tray_click_pending = Some(now);
    }

    fn quit(&mut self) {
        self.flush_save();
        let _ = slint::quit_event_loop();
    }

    fn schedule_save(&mut self) {
        self.save_timer.restart();
    }

    pub fn flush_save(&mut self) {
        let notes = self.snapshot();
        if let Err(err) = storage::save(&AppState { notes }) {
            eprintln!("failed to save notes: {err}");
        }
    }

    fn snapshot(&self) -> Vec<Note> {
        self.windows
            .values()
            .map(|handle| {
                let w = &handle.window;
                let sf = w.window().scale_factor();
                let pos = w.window().position().to_logical(sf);
                let size = w.window().size().to_logical(sf);
                let blocks = (0..handle.blocks.row_count())
                    .filter_map(|i| handle.blocks.row_data(i))
                    .map(|item| Block {
                        id: item.id.to_string(),
                        block_type: if item.is_todo {
                            BlockType::Todo
                        } else {
                            BlockType::Text
                        },
                        content: item.content.to_string(),
                        checked: if item.is_todo { Some(item.checked) } else { None },
                    })
                    .collect();
                Note {
                    id: handle.id.clone(),
                    position: (
                        (pos.x + SHADOW_MARGIN) as i32,
                        (pos.y + SHADOW_MARGIN) as i32,
                    ),
                    size: (
                        (size.width - 2.0 * SHADOW_MARGIN).max(200.0) as u32,
                        (size.height - 2.0 * SHADOW_MARGIN).max(200.0) as u32,
                    ),
                    always_on_top: w.get_pinned(),
                    blocks,
                }
            })
            .collect()
    }

    fn apply_dark_mode(&mut self, dark: bool) {
        if self.dark_mode == dark {
            return;
        }
        self.dark_mode = dark;
        for handle in self.windows.values() {
            handle.window.global::<Theme>().set_dark_mode(dark);
        }
    }

    /// Manually flip light/dark.
    fn toggle_theme(&mut self) {
        self.apply_dark_mode(!self.dark_mode);
    }

    fn poll(&mut self, app: Rc<RefCell<App>>) {
        // Tray actions.
        if let Some(action) = self.tray.as_ref().and_then(|t| t.next_action()) {
            match action {
                tray::Action::New => self.create_note(app.clone()),
                tray::Action::ShowAll => self.show_all(),
                tray::Action::HideAll => self.hide_all(),
                tray::Action::Toggle => self.on_tray_click(app.clone()),
                tray::Action::Quit => self.quit(),
            }
        }

        // Resolve a deferred tray single-click once the double-click window has
        // elapsed without a second click.
        if let Some(prev) = self.tray_click_pending {
            if prev.elapsed() >= DOUBLE_CLICK_WINDOW {
                self.tray_click_pending = None;
                self.toggle_all();
            }
        }

        // Global hotkey.
        if let Some((_, id)) = self.hotkey.as_ref() {
            let pressed = global_hotkey::GlobalHotKeyEvent::receiver()
                .try_iter()
                .any(|event| event.id == *id);
            if pressed {
                self.create_note(app.clone());
            }
        }
    }
}

/// Registers all UI callbacks on a note window.
fn wire_callbacks(
    window: NoteWindow,
    blocks: Rc<VecModel<BlockItem>>,
    note_id: String,
    app: Rc<RefCell<App>>,
) {
    // Content edited: detect `- [ ]` -> todo, then debounce save.
    {
        let app = app.clone();
        let blocks = blocks.clone();
        let weak = window.as_weak();
        window.on_block_edited(move |index, text| {
            let idx = index as usize;
            if let Some(mut item) = blocks.row_data(idx) {
                if !item.is_todo && text.trim() == "- [ ]" {
                    item.is_todo = true;
                    item.content = "".into();
                    item.checked = false;
                    item.rendered = render_markdown("");
                    blocks.set_row_data(idx, item);
                    focus_block(&weak, index);
                } else {
                    // Refresh the rendered Markdown for the new source text.
                    item.content = text.clone();
                    item.rendered = render_markdown(&text);
                    blocks.set_row_data(idx, item);
                }
            }
            app.borrow_mut().schedule_save();
        });
    }

    // Enter: insert a new text line below.
    {
        let app = app.clone();
        let blocks = blocks.clone();
        let weak = window.as_weak();
        window.on_block_accepted(move |index| {
            blocks.insert((index + 1) as usize, empty_text_block());
            clear_selection(&weak);
            focus_block(&weak, index + 1);
            app.borrow_mut().schedule_save();
        });
    }

    // Checkbox toggled.
    {
        let app = app.clone();
        let blocks = blocks.clone();
        window.on_block_checked(move |index, value| {
            let idx = index as usize;
            if let Some(mut item) = blocks.row_data(idx) {
                item.checked = value;
                blocks.set_row_data(idx, item);
            }
            app.borrow_mut().schedule_save();
        });
    }

    // Backspace on an empty text line, or "delete" from the context menu:
    // remove the whole selection when multiple blocks are selected, otherwise
    // the single block.
    {
        let app = app.clone();
        let blocks = blocks.clone();
        let weak = window.as_weak();
        window.on_block_delete(move |index| {
            let (lo, hi) = {
                let sel = weak
                    .upgrade()
                    .map(|w| (w.get_sel_start(), w.get_sel_end()))
                    .unwrap_or((-1, -1));
                if sel.0 >= 0 && sel.1 >= 0 && sel.0 != sel.1 {
                    (sel.0.min(sel.1), sel.1.max(sel.0))
                } else {
                    (index, index)
                }
            };
            let count = (hi - lo + 1) as usize;
            let len = blocks.row_count();
            if count >= len {
                blocks.set_vec(vec![empty_text_block()]);
                focus_block(&weak, 0);
            } else {
                for _ in 0..count {
                    blocks.remove(lo as usize);
                }
                focus_block(&weak, lo.clamp(0, (len - count) as i32 - 1));
            }
            clear_selection(&weak);
            app.borrow_mut().schedule_save();
        });
    }

    // Backspace on an empty todo: downgrade it to a plain text line.
    {
        let app = app.clone();
        let blocks = blocks.clone();
        window.on_block_downgrade(move |index| {
            let idx = index as usize;
            if let Some(mut item) = blocks.row_data(idx) {
                item.is_todo = false;
                item.checked = false;
                blocks.set_row_data(idx, item);
            }
            app.borrow_mut().schedule_save();
        });
    }

    // Drag-reorder a todo line.
    {
        let app = app.clone();
        let blocks = blocks.clone();
        let weak = window.as_weak();
        window.on_block_move(move |index, delta| {
            let len = blocks.row_count() as i32;
            if len < 2 {
                return;
            }
            let target = (index + (delta / ROW_HEIGHT).round() as i32).clamp(0, len - 1);
            if target == index {
                return;
            }
            let item = blocks.remove(index as usize);
            blocks.insert(target as usize, item);
            clear_selection(&weak);
            focus_block(&weak, target);
            app.borrow_mut().schedule_save();
        });
    }

    // Context menu: insert a todo after the current line.
    {
        let app = app.clone();
        let blocks = blocks.clone();
        let weak = window.as_weak();
        window.on_insert_todo(move |index| {
            blocks.insert((index + 1) as usize, empty_todo_block());
            clear_selection(&weak);
            focus_block(&weak, index + 1);
            app.borrow_mut().schedule_save();
        });
    }

    // In-app Ctrl+Alt+N (fallback when the Wayland global hotkey is unavailable).
    {
        let app = app.clone();
        window.on_new_note_requested(move || {
            app.borrow_mut().create_note(app.clone());
        });
    }

    // Delete this note permanently.
    {
        let app = app.clone();
        let id = note_id.clone();
        window.on_delete_note_requested(move || {
            app.borrow_mut().delete_note(&id);
        });
    }

    // Manual light/dark toggle (global).
    {
        let app = app.clone();
        window.on_theme_toggle_requested(move || {
            app.borrow_mut().toggle_theme();
        });
    }

    // Pin toggle.
    {
        let app = app.clone();
        let weak = window.as_weak();
        window.on_pin_requested(move || {
            if let Some(w) = weak.upgrade() {
                w.set_pinned(!w.get_pinned());
            }
            app.borrow_mut().schedule_save();
        });
    }

    // Close -> fade out then hide.
    {
        let weak = window.as_weak();
        window.on_close_requested(move || {
            if let Some(w) = weak.upgrade() {
                hide_note(&w);
            }
        });
    }
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Parses Markdown for display, falling back to plain text when the source
/// contains unsupported syntax (headings, tables, code blocks, ...).
fn render_markdown(text: &str) -> slint::StyledText {
    slint::StyledText::from_markdown(text)
        .unwrap_or_else(|_| slint::StyledText::from_plain_text(text))
}

fn empty_text_block() -> BlockItem {
    BlockItem {
        id: format!("block-{}", timestamp_millis()).into(),
        is_todo: false,
        content: "".into(),
        checked: false,
        rendered: slint::StyledText::from_plain_text(""),
    }
}

fn empty_todo_block() -> BlockItem {
    BlockItem {
        id: format!("block-{}", timestamp_millis()).into(),
        is_todo: true,
        content: "".into(),
        checked: false,
        rendered: slint::StyledText::from_plain_text(""),
    }
}

fn focus_block(weak: &Weak<NoteWindow>, index: i32) {
    if let Some(w) = weak.upgrade() {
        w.set_focus_index(index);
        w.set_focus_seq(w.get_focus_seq() + 1);
    }
}

fn clear_selection(weak: &Weak<NoteWindow>) {
    if let Some(w) = weak.upgrade() {
        w.set_sel_start(-1);
        w.set_sel_end(-1);
    }
}

/// Queries the desktop's logical screen size so new notes can be centered.
///
/// Uses a subprocess (rather than a winit event loop, which cannot be created
/// a second time once Slint's backend owns one) and falls back to `None` when
/// no tool is available, e.g. a pure-Wayland session without XWayland.
fn screen_size() -> Option<(f32, f32)> {
    // `xrandr --query` first line: "Screen 0: minimum 8 x 8, current 1920 x 1080, maximum ..."
    let output = std::process::Command::new("xrandr").arg("--query").output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?;
    let dims = line.split("current ").nth(1)?.split(',').next()?;
    let parts: Vec<&str> = dims.split_whitespace().collect();
    if parts.len() != 3 || parts[1] != "x" {
        return None;
    }
    let width: f32 = parts[0].parse().ok()?;
    let height: f32 = parts[2].parse().ok()?;
    Some((width, height))
}

fn hide_note(w: &NoteWindow) {
    w.set_fade_opacity(0.0);
    let weak = w.as_weak();
    Timer::single_shot(Duration::from_millis(150), move || {
        if let Some(w) = weak.upgrade() {
            let _ = w.hide();
        }
    });
}

fn show_note(w: &NoteWindow) {
    w.set_fade_opacity(0.0);
    let _ = w.show();
    let weak = w.as_weak();
    Timer::single_shot(Duration::from_millis(16), move || {
        if let Some(w) = weak.upgrade() {
            w.set_fade_opacity(1.0);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::render_markdown;

    #[test]
    fn markdown_supported_syntax_parses() {
        // Bold, italic, strikethrough, inline code, links and lists are all
        // supported by Slint's CommonMark subset.
        for src in [
            "**bold**",
            "*italic*",
            "~~strike~~",
            "`code`",
            "[link](https://example.com)",
            "- item",
            "1. item",
            "**bold** and *italic*",
        ] {
            assert!(
                slint::StyledText::from_markdown(src).is_ok(),
                "expected supported markdown to parse: {src}"
            );
        }
    }

    #[test]
    fn markdown_unsupported_falls_back_to_plain_text() {
        // Headings/tables are not supported and must not panic; render_markdown
        // falls back to the raw text.
        for src in ["# heading", "| a | b |"] {
            let _ = render_markdown(src);
        }
        assert!(slint::StyledText::from_markdown("# heading").is_err());
    }
}
