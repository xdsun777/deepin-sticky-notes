use anyhow::Result;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager,
};

pub fn register_new_note() -> Result<Option<(GlobalHotKeyManager, u32)>> {
    let manager = GlobalHotKeyManager::new()?;
    let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyN);
    let id = hotkey.id();
    manager.register(hotkey)?;
    Ok(Some((manager, id)))
}
