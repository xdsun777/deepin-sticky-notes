use anyhow::Result;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

pub enum Action {
    New,
    ShowAll,
    HideAll,
    Quit,
}

pub struct Tray {
    _icon: TrayIcon,
    pub new_id: tray_icon::menu::MenuId,
    pub show_id: tray_icon::menu::MenuId,
    pub hide_id: tray_icon::menu::MenuId,
    pub quit_id: tray_icon::menu::MenuId,
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
        menu.append(&tray_icon::menu::PredefinedMenuItem::separator())?;
        menu.append(&quit_item)?;
        let icon = Icon::from_rgba(vec![0x2e, 0x2e, 0x2e, 0xff], 1, 1)?;
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

    pub fn action(&self) -> Option<Action> {
        let event = MenuEvent::receiver().try_iter().next()?;
        Some(if event.id == self.new_id {
            Action::New
        } else if event.id == self.show_id {
            Action::ShowAll
        } else if event.id == self.hide_id {
            Action::HideAll
        } else if event.id == self.quit_id {
            Action::Quit
        } else {
            return None;
        })
    }
}
