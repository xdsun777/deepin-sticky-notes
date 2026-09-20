mod hotkey;
mod model;
mod storage;
mod theme;
mod tray;

slint::include_modules!();

use anyhow::Result;
use model::{AppState, Note, NoteColor};
use slint::{ComponentHandle, Timer, TimerMode};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn main() -> Result<()> {
    let mut state = storage::load().unwrap_or_default();
    if state.notes.is_empty() {
        state.notes.push(Note::new("note-1".into(), (120, 120)));
        storage::save(&state)?;
    }

    let state = Rc::new(RefCell::new(state));
    let windows: Rc<RefCell<Vec<NoteWindow>>> = Rc::new(RefCell::new(Vec::new()));
    let ids: Vec<String> = state
        .borrow()
        .notes
        .iter()
        .map(|note| note.id.clone())
        .collect();

    for id in ids {
        open_note(&id, state.clone(), windows.clone())?;
    }

    let tray = tray::Tray::create().ok();
    let hotkey = hotkey::register_new_note().ok().flatten();
    let dark_mode = theme::is_dark();
    let current_dark_mode = Rc::new(RefCell::new(dark_mode));
    for window in windows.borrow().iter() {
        window.set_dark_mode(dark_mode);
    }

    let state_for_events = state.clone();
    let windows_for_events = windows.clone();
    let event_timer = Timer::default();
    let theme_state = current_dark_mode.clone();
    event_timer.start(TimerMode::Repeated, Duration::from_millis(120), move || {
        let detected_dark_mode = theme::is_dark();
        if detected_dark_mode != *theme_state.borrow() {
            *theme_state.borrow_mut() = detected_dark_mode;
            windows_for_events
                .borrow()
                .iter()
                .for_each(|window| window.set_dark_mode(detected_dark_mode));
        }
        if let Some(tray) = tray.as_ref() {
            match tray.action() {
                Some(tray::Action::New) => {
                    create_note(state_for_events.clone(), windows_for_events.clone())
                }
                Some(tray::Action::ShowAll) => {
                    windows_for_events.borrow().iter().for_each(|window| {
                        let _ = window.show();
                    })
                }
                Some(tray::Action::HideAll) => {
                    windows_for_events.borrow().iter().for_each(|window| {
                        let _ = window.hide();
                    })
                }
                Some(tray::Action::Quit) => {
                    let _ = storage::save(&state_for_events.borrow());
                    slint::quit_event_loop().ok();
                }
                None => {}
            }
        }
        if let Some((_, hotkey_id)) = hotkey.as_ref() {
            if global_hotkey::GlobalHotKeyEvent::receiver()
                .try_iter()
                .any(|event| event.id == *hotkey_id)
            {
                create_note(state_for_events.clone(), windows_for_events.clone());
            }
        }
    });
    slint::run_event_loop()?;
    storage::save(&state.borrow())?;
    Ok(())
}

fn create_note(state: Rc<RefCell<AppState>>, windows: Rc<RefCell<Vec<NoteWindow>>>) {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let id = format!("note-{suffix}");
    let position = (
        240 + (state.borrow().notes.len() as i32 * 24),
        160 + (state.borrow().notes.len() as i32 * 24),
    );
    {
        state
            .borrow_mut()
            .notes
            .push(Note::new(id.clone(), position));
    }
    persist_state(&state);
    let _ = open_note(&id, state, windows);
}

fn persist_state(state: &Rc<RefCell<AppState>>) {
    let snapshot = state.borrow().clone();
    let _ = storage::save(&snapshot);
}

fn open_note(
    id: &str,
    state: Rc<RefCell<AppState>>,
    windows: Rc<RefCell<Vec<NoteWindow>>>,
) -> Result<()> {
    let note = state
        .borrow()
        .notes
        .iter()
        .find(|note| note.id == id)
        .cloned()
        .unwrap();
    let window = NoteWindow::new()?;
    window.set_note_id(note.id.clone().into());
    window.set_note_title(format!("便签 · {}", &note.id[..note.id.len().min(8)]).into());
    window.set_initial_body(note.plain_text().into());
    window.set_note_color(color_name(&note.color).into());
    window.set_pinned(note.always_on_top);

    let note_id = note.id.clone();
    let state_for_edit = state.clone();
    window.on_body_edited(move |body| {
        {
            let mut app_state = state_for_edit.borrow_mut();
            if let Some(note) = app_state.notes.iter_mut().find(|note| note.id == note_id) {
                note.from_text(body.as_str());
            }
        }
        persist_state(&state_for_edit);
    });

    let note_id = note.id.clone();
    let state_for_pin = state.clone();
    let window_for_pin = window.as_weak();
    window.on_pin_requested(move || {
        let pinned = {
            let mut app_state = state_for_pin.borrow_mut();
            app_state
                .notes
                .iter_mut()
                .find(|note| note.id == note_id)
                .map(|note| {
                    note.always_on_top = !note.always_on_top;
                    note.always_on_top
                })
        };
        if let Some(pinned) = pinned {
            if let Some(window) = window_for_pin.upgrade() {
                window.set_pinned(pinned);
            }
            persist_state(&state_for_pin);
        }
    });

    let note_id = note.id.clone();
    let state_for_color = state.clone();
    let window_for_color = window.as_weak();
    window.on_color_requested(move |color| {
        let selected = match color.as_str() {
            "white" => NoteColor::White,
            "blue" => NoteColor::Blue,
            _ => NoteColor::Yellow,
        };
        let found = {
            let mut app_state = state_for_color.borrow_mut();
            if let Some(note) = app_state.notes.iter_mut().find(|note| note.id == note_id) {
                note.color = selected;
                true
            } else {
                false
            }
        };
        if found {
            if let Some(window) = window_for_color.upgrade() {
                window.set_note_color(color);
            }
            persist_state(&state_for_color);
        }
    });

    let window_for_close = window.as_weak();
    window.on_close_requested(move || {
        if let Some(window) = window_for_close.upgrade() {
            window.hide().ok();
        }
    });

    window.show()?;
    windows.borrow_mut().push(window);
    Ok(())
}

fn color_name(color: &NoteColor) -> &'static str {
    match color {
        NoteColor::Yellow => "yellow",
        NoteColor::White => "white",
        NoteColor::Blue => "blue",
    }
}
