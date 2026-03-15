mod config;
mod hotkey;
mod injector;
mod menu_bar;
mod recorder;
mod transcriber;

use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;

fn main() {
    let mtm = MainThreadMarker::new().expect("must run on the main thread");

    // 8.1 — Initialize menu bar (NSApplication, status item, menu).
    let _menu_bar = menu_bar::MenuBar::new(mtm);

    // Run the NSApplication event loop (blocks forever).
    let app = NSApplication::sharedApplication(mtm);
    app.run();
}
