#![allow(dead_code)]

use std::sync::mpsc;

use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapProxy, CGEventType, EventField,
};

use crate::config;

/// Installs a global `CGEventTap` that listens for the configured hotkey combo
/// (default: ⌥+Space). Returns an `mpsc::Receiver<()>` that receives a unit value
/// each time the hotkey is pressed. The event is swallowed so the focused application
/// never sees it.
///
/// The tap is added to the current `CFRunLoop`, which must be the main thread's run
/// loop (shared with `NSApplication`).
///
/// # Panics
///
/// Panics if the event tap cannot be created (typically because the process lacks
/// Accessibility permission).
pub fn install() -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel();

    // 4.1 — Create a CGEventTap listening for key-down events.
    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![CGEventType::KeyDown],
        move |_proxy: CGEventTapProxy, _etype: CGEventType, event: &CGEvent| -> Option<CGEvent> {
            // 4.2 — Inspect keycode and modifier flags for the configured combo.
            let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;

            if keycode == config::HOTKEY_KEYCODE {
                let flags = event.get_flags();
                // Check that the alternate (option) flag is set. We mask out
                // device-dependent bits (lower 16 bits) and non-coalesced to avoid
                // false negatives from extra flags the OS may set.
                let modifier_mask = CGEventFlags::CGEventFlagAlphaShift
                    | CGEventFlags::CGEventFlagShift
                    | CGEventFlags::CGEventFlagControl
                    | CGEventFlags::CGEventFlagAlternate
                    | CGEventFlags::CGEventFlagCommand;
                let active_modifiers = flags & modifier_mask;
                let expected = CGEventFlags::CGEventFlagControl;

                if active_modifiers == expected {
                    // 4.3 — Swallow the event and signal a toggle.
                    let _ = tx.send(());
                    return None;
                }
            }

            Some(event.clone())
        },
    )
    .expect(
        "failed to create CGEventTap — grant Accessibility permission in \
         System Settings → Privacy & Security → Accessibility",
    );

    // 4.4 — Add the event tap to the current CFRunLoop.
    let loop_source = tap
        .mach_port
        .create_runloop_source(0)
        .expect("failed to create run-loop source from event tap mach port");

    let current = CFRunLoop::get_current();
    current.add_source(&loop_source, unsafe { kCFRunLoopCommonModes });

    tap.enable();

    // Leak the tap and run-loop source so they live for the duration of the process.
    // We never need to remove or disable the tap.
    std::mem::forget(tap);
    std::mem::forget(loop_source);

    rx
}
