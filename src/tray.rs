//! System tray icon and menu (StatusNotifierItem / KSNI backend).

use anyhow::Result;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

/// An action the user triggered through the tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    New,
    ShowAll,
    HideAll,
    Quit,
    /// Left-click on the tray icon: toggle "show all / hide all".
    Toggle,
}

pub struct Tray {
    _icon: TrayIcon,
    new_id: tray_icon::menu::MenuId,
    show_id: tray_icon::menu::MenuId,
    hide_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
}

impl Tray {
    pub fn create() -> Result<Self> {
        let menu = Menu::new();
        let new_item = MenuItem::with_id("new", "新建便签 (Ctrl+Alt+N)", true, None);
        let show_item = MenuItem::with_id("show", "显示全部", true, None);
        let hide_item = MenuItem::with_id("hide", "隐藏全部", true, None);
        let quit_item = MenuItem::with_id("quit", "退出", true, None);
        menu.append(&new_item)?;
        menu.append(&show_item)?;
        menu.append(&hide_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&quit_item)?;

        let icon = Icon::from_rgba(sticky_note_icon(), 32, 32)?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Deepin 极简便签")
            .with_icon(icon)
            .build()?;

        Ok(Self {
            _icon: tray,
            new_id: new_item.id().clone(),
            show_id: show_item.id().clone(),
            hide_id: hide_item.id().clone(),
            quit_id: quit_item.id().clone(),
        })
    }

    /// Pops the next tray action off the event queue, if any.
    pub fn next_action(&self) -> Option<Action> {
        // Menu selections first.
        for event in MenuEvent::receiver().try_iter() {
            if event.id == self.new_id {
                return Some(Action::New);
            } else if event.id == self.show_id {
                return Some(Action::ShowAll);
            } else if event.id == self.hide_id {
                return Some(Action::HideAll);
            } else if event.id == self.quit_id {
                return Some(Action::Quit);
            }
        }

        // Left-click toggles visibility. (KSNI reports left activation.)
        for event in TrayIconEvent::receiver().try_iter() {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                return Some(Action::Toggle);
            }
        }

        None
    }
}

/// A small programmatically drawn sticky-note icon (32x32 RGBA): a yellow note
/// with a folded corner and two grey text lines.
fn sticky_note_icon() -> Vec<u8> {
    const S: usize = 32;
    let mut rgba = vec![0u8; S * S * 4];

    for y in 0..S {
        for x in 0..S {
            let i = (y * S + x) * 4;

            // Fold triangle in the bottom-right corner (cut off the note).
            let in_fold = (x as i32) + (y as i32) >= 2 * S as i32 - 4;

            let in_body = x >= 3 && x < S - 3 && y >= 3 && y < S - 3;

            if in_body && !in_fold {
                // Note body: soft yellow.
                rgba[i] = 0xFF;
                rgba[i + 1] = 0xF9;
                rgba[i + 2] = 0xD0;
                rgba[i + 3] = 0xFF;
            } else if in_body && in_fold {
                // Folded corner: slightly darker yellow.
                rgba[i] = 0xE8;
                rgba[i + 1] = 0xDF;
                rgba[i + 2] = 0x9A;
                rgba[i + 3] = 0xFF;
            }
        }
    }

    // Two grey "text" lines.
    for x in 8..24 {
        let y = 12;
        rgba[(y * S + x) * 4] = 0x88;
        rgba[(y * S + x) * 4 + 1] = 0x88;
        rgba[(y * S + x) * 4 + 2] = 0x88;
        rgba[(y * S + x) * 4 + 3] = 0xFF;

        let y2 = 18;
        rgba[(y2 * S + x) * 4] = 0x88;
        rgba[(y2 * S + x) * 4 + 1] = 0x88;
        rgba[(y2 * S + x) * 4 + 2] = 0x88;
        rgba[(y2 * S + x) * 4 + 3] = 0xFF;
    }

    rgba
}
