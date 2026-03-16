use objc2::rc::Retained;
use objc2::sel;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem,
    NSVariableStatusItemLength,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSString};

// Icon strings for each app state.
const ICON_IDLE: &str = "🎤";
const ICON_RECORDING: &str = "🔴";
const ICON_TRANSCRIBING: &str = "⏳";

/// The possible visual states of the menu bar icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconState {
    Idle,
    Recording,
    Transcribing,
}

// Define a minimal Objective-C class to act as a target for menu item actions.
define_class!(
    // SAFETY: NSObject has no subclassing requirements. We don't implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "MenuBarDelegate"]
    struct MenuBarDelegate;

    impl MenuBarDelegate {
        #[unsafe(method(quit:))]
        fn quit(&self, _sender: *mut NSObject) {
            let mtm = MainThreadMarker::from(self);
            let app = NSApplication::sharedApplication(mtm);
            app.terminate(None);
        }
    }

    unsafe impl NSObjectProtocol for MenuBarDelegate {}
);

impl MenuBarDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let _ = mtm;
        unsafe { msg_send![Self::alloc(mtm), init] }
    }
}

/// Holds references to the menu bar UI elements.
pub struct MenuBar {
    status_item: Retained<NSStatusItem>,
    status_label: Retained<NSMenuItem>,
    // Keep the delegate alive so it isn't deallocated.
    _delegate: Retained<MenuBarDelegate>,
}

impl MenuBar {
    /// Initialise the NSApplication and create the menu bar status item.
    ///
    /// Must be called on the main thread.
    pub fn new(mtm: MainThreadMarker) -> Self {
        // 3.1 — Set up NSApplication with accessory activation policy (no dock icon).
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

        // 3.2 — Create an NSStatusItem with a text title.
        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = status_item.button(mtm) {
            button.setTitle(&NSString::from_str(ICON_IDLE));
        }

        // 3.4 — Create the delegate for menu item actions.
        let delegate = MenuBarDelegate::new(mtm);

        // 3.3 — Create an NSMenu with a disabled status label and a Quit item.
        let menu = NSMenu::new(mtm);

        // Disabled status label.
        let status_label = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Idle"),
                None,
                &NSString::from_str(""),
            )
        };
        status_label.setEnabled(false);
        menu.addItem(&status_label);

        // 3.5 — Wire Quit to NSApplication::terminate via the delegate.
        let quit_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Quit"),
                Some(sel!(quit:)),
                &NSString::from_str("q"),
            )
        };
        unsafe {
            quit_item.setTarget(Some(&delegate));
        }
        menu.addItem(&quit_item);

        status_item.setMenu(Some(&menu));

        Self {
            status_item,
            status_label,
            _delegate: delegate,
        }
    }

    /// 3.6 — Update the menu bar icon and status label to reflect the current state.
    pub fn set_state(&self, state: IconState, mtm: MainThreadMarker) {
        let (icon, label) = match state {
            IconState::Idle => (ICON_IDLE, "Idle"),
            IconState::Recording => (ICON_RECORDING, "Recording…"),
            IconState::Transcribing => (ICON_TRANSCRIBING, "Transcribing…"),
        };

        if let Some(button) = self.status_item.button(mtm) {
            button.setTitle(&NSString::from_str(icon));
        }
        self.status_label.setTitle(&NSString::from_str(label));
    }
}
