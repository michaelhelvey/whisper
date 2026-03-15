#![allow(dead_code)]

use std::thread;
use std::time::Duration;

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSPasteboardWriting};
use objc2_foundation::{NSArray, NSData, NSString};

use crate::config;

/// Represents a single pasteboard item's data across all its types.
struct SavedItem {
    /// Each entry is (type_string, raw_data) for one representation.
    entries: Vec<(String, Vec<u8>)>,
}

/// Saves all items and their type/data pairs from the general pasteboard.
fn save_pasteboard(pb: &NSPasteboard) -> Vec<SavedItem> {
    let Some(items) = pb.pasteboardItems() else {
        return Vec::new();
    };

    items
        .iter()
        .map(|item| {
            let types = item.types();
            let entries = types
                .iter()
                .filter_map(|t| {
                    let data = item.dataForType(&t)?;
                    Some((t.to_string(), data.to_vec()))
                })
                .collect();
            SavedItem { entries }
        })
        .collect()
}

/// Restores previously saved items back onto the pasteboard.
fn restore_pasteboard(pb: &NSPasteboard, saved: &[SavedItem]) {
    pb.clearContents();

    if saved.is_empty() {
        return;
    }

    let items: Vec<_> = saved
        .iter()
        .map(|saved_item| {
            let item = NSPasteboardItem::new();
            for (type_str, bytes) in &saved_item.entries {
                let ns_type = NSString::from_str(type_str);
                let ns_data = NSData::with_bytes(bytes);
                item.setData_forType(&ns_data, &ns_type);
            }
            item
        })
        .collect();

    // Convert Vec<Retained<NSPasteboardItem>> to an NSArray of NSPasteboardWriting protocol
    // objects so `writeObjects` accepts it.
    let protocol_objects: Vec<&ProtocolObject<dyn NSPasteboardWriting>> = items
        .iter()
        .map(|item| {
            let r: &NSPasteboardItem = item;
            ProtocolObject::from_ref(r)
        })
        .collect();
    let array = NSArray::from_slice(&protocol_objects);
    pb.writeObjects(&array);
}

/// Synthesises a ⌘V paste by posting key-down and key-up `CGEvent`s for
/// virtual keycode 9 (V) with the Command modifier flag.
fn post_paste_keystroke() {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .expect("failed to create CGEventSource");

    // Virtual keycode 9 = 'V'
    let key_down = CGEvent::new_keyboard_event(source.clone(), 9, true)
        .expect("failed to create key-down CGEvent");
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(CGEventTapLocation::HID);

    let key_up =
        CGEvent::new_keyboard_event(source, 9, false).expect("failed to create key-up CGEvent");
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.post(CGEventTapLocation::HID);
}

/// Injects `text` into the currently focused application by:
///
/// 1. Placing `text` on the pasteboard.
/// 2. Synthesising a ⌘V keystroke.
/// 3. Waiting [`config::PASTE_DELAY_MS`] ms for the target app to process the paste.
/// 4. Restoring the original pasteboard contents on a background thread.
///
/// If the synthetic ⌘V fails (e.g. missing Accessibility permission), the text
/// remains on the clipboard so the user can paste manually.
pub fn inject(text: &str) {
    let pb = NSPasteboard::generalPasteboard();

    // 7.1 — Stash current pasteboard contents.
    let saved = save_pasteboard(&pb);

    // 7.2 — Clear pasteboard and write transcript string.
    pb.clearContents();
    let ns_string = NSString::from_str(text);
    pb.setString_forType(&ns_string, unsafe { NSPasteboardTypeString });

    // Small delay to ensure pasteboard update propagates before the paste event.
    thread::sleep(Duration::from_millis(10));

    // 7.3 — Post synthetic ⌘V.
    post_paste_keystroke();

    // 7.4 — Restore the original pasteboard after a delay, on a background thread
    // so we don't block the main run loop and risk restoring before the target
    // app has finished processing the paste.
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(config::PASTE_DELAY_MS));
        // Re-acquire the pasteboard on this thread.
        let pb = NSPasteboard::generalPasteboard();
        restore_pasteboard(&pb, &saved);
    });
}
