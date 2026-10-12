use super::{StatusBarAction, StatusBarState};
use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained, sel};
use objc2_app_kit::{NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSSize, NSString};
use std::sync::mpsc::{self, Receiver, Sender};

struct MenuTargetIvars {
    sender: Sender<StatusBarAction>,
}

define_class!(
    // SAFETY: NSObject has no additional subclass invariants; AppKit invokes this target on the main thread.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = MenuTargetIvars]
    struct MenuTarget;

    // SAFETY: NSObject implements the protocol and the subclass preserves its contracts.
    unsafe impl NSObjectProtocol for MenuTarget {}

    impl MenuTarget {
        // SAFETY: The selector is installed only on NSMenuItems and takes the sender AppKit supplies.
        #[unsafe(method(performMenuAction:))]
        fn perform_action(&self, item: &NSMenuItem) {
            let action = match item.tag() {
                1 => StatusBarAction::OpenDashboard,
            6 => StatusBarAction::OpenSettings,
                2 => StatusBarAction::ToggleShortcuts,
                3 => StatusBarAction::RestoreClipboard,
                4 => StatusBarAction::OpenPermissions,
                5 => StatusBarAction::Quit,
                _ => return,
            };
            let _ = self.ivars().sender.send(action);
        }
    }
);

pub struct StatusBar {
    item: Retained<NSStatusItem>,
    status: Retained<NSMenuItem>,
    toggle: Retained<NSMenuItem>,
    restore: Retained<NSMenuItem>,
    receiver: Receiver<StatusBarAction>,
    _target: Retained<MenuTarget>,
    state: Option<StatusBarState>,
}

impl StatusBar {
    pub fn new() -> Result<Self, &'static str> {
        let mtm = MainThreadMarker::new().ok_or("Menu bar requires the main thread.")?;
        let data =
            objc2_foundation::NSData::with_bytes(include_bytes!("../../assets/menu-bar.png"));
        let image = NSImage::initWithData(NSImage::alloc(), &data)
            .ok_or("Could not load the menu bar icon.")?;
        image.setTemplate(true);
        image.setSize(NSSize::new(18., 18.));
        let (sender, receiver) = mpsc::channel();
        let target = MenuTarget::alloc(mtm).set_ivars(MenuTargetIvars { sender });
        // SAFETY: NSObject's initializer is valid for this allocated subclass with initialized Rust ivars.
        let target: Retained<MenuTarget> = unsafe { msg_send![super(target), init] };
        let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Copy AI"));
        menu.setAutoenablesItems(false);
        let status = NSMenuItem::new(mtm);
        status.setEnabled(false);
        menu.addItem(&status);
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&Self::action_item("Open dashboard", 1, &target, mtm));
        menu.addItem(&Self::action_item("Settings…", 6, &target, mtm));
        let toggle = Self::action_item("Enable shortcuts", 2, &target, mtm);
        menu.addItem(&toggle);
        let restore = Self::action_item("Restore clipboard only", 3, &target, mtm);
        menu.addItem(&restore);
        menu.addItem(&Self::action_item(
            "Accessibility permission…",
            4,
            &target,
            mtm,
        ));
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&Self::action_item("Quit Copy AI", 5, &target, mtm));
        let bar = NSStatusBar::systemStatusBar();
        let item = bar.statusItemWithLength(30.);
        let Some(button) = item.button(mtm) else {
            bar.removeStatusItem(&item);
            return Err("Could not create the menu bar button.");
        };
        button.setImage(Some(&image));
        item.setMenu(Some(&menu));
        let mut result = Self {
            item,
            status,
            toggle,
            restore,
            receiver,
            _target: target,
            state: None,
        };
        result.update(StatusBarState::Paused);
        Ok(result)
    }

    fn action_item(
        title: &str,
        tag: isize,
        target: &MenuTarget,
        mtm: MainThreadMarker,
    ) -> Retained<NSMenuItem> {
        let item = NSMenuItem::new(mtm);
        item.setTitle(&NSString::from_str(title));
        item.setTag(tag);
        // SAFETY: The target is retained by StatusBar longer than its menu, and the selector signature matches NSMenuItem's sender.
        unsafe {
            item.setTarget(Some(target));
            item.setAction(Some(sel!(performMenuAction:)));
        }
        item
    }

    pub fn next_action(&self) -> Option<StatusBarAction> {
        self.receiver.try_recv().ok()
    }

    pub fn update(&mut self, state: StatusBarState) {
        if self.state == Some(state) {
            return;
        }
        self.state = Some(state);
        self.status.setTitle(&NSString::from_str(state.label()));
        self.toggle
            .setTitle(&NSString::from_str(if state == StatusBarState::Paused {
                "Enable shortcuts"
            } else {
                "Disable shortcuts"
            }));
        self.restore.setEnabled(state != StatusBarState::Processing);
        if let Some(mtm) = MainThreadMarker::new()
            && let Some(button) = self.item.button(mtm)
        {
            button.setTitle(&NSString::from_str(match state {
                StatusBarState::Paused => "Ⅱ",
                StatusBarState::PermissionRequired => "!",
                StatusBarState::Ready => "●",
                StatusBarState::Processing => "…",
            }));
            button.setToolTip(Some(&NSString::from_str(&format!(
                "Copy AI · {}",
                state.label()
            ))));
            self.item.setLength(42.);
        }
    }
}

impl Drop for StatusBar {
    fn drop(&mut self) {
        NSStatusBar::systemStatusBar().removeStatusItem(&self.item);
    }
}
