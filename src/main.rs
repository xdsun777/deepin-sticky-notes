mod app;
mod hotkey;
mod model;
mod storage;
mod theme;
mod tray;

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;

fn main() -> Result<()> {
    let app = Rc::new(RefCell::new(app::App::new()?));
    app::App::start(app.clone())?;

    slint::run_event_loop()?;

    // Flush any pending state on exit.
    app.borrow_mut().flush_save();
    Ok(())
}
