use global_hotkey::{
    GlobalHotKeyManager,
    hotkey::{Code, HotKey, Modifiers},
};

pub struct Shortcuts {
    manager: GlobalHotKeyManager,
    paste: HotKey,
    restore: HotKey,
}

impl Shortcuts {
    pub fn register() -> Result<Self, String> {
        let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
        let paste = HotKey::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyV);
        let restore = HotKey::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyR);
        manager
            .register(paste)
            .map_err(|e| format!("Shortcut Cmd+Shift+V is unavailable: {e}"))?;
        if let Err(error) = manager.register(restore) {
            let _ = manager.unregister(paste);
            return Err(format!("Shortcut Cmd+Shift+R is unavailable: {error}"));
        }
        Ok(Self {
            manager,
            paste,
            restore,
        })
    }

    pub fn is_paste(&self, id: u32) -> bool {
        self.paste.id() == id
    }
    pub fn is_restore(&self, id: u32) -> bool {
        self.restore.id() == id
    }
}

impl Drop for Shortcuts {
    fn drop(&mut self) {
        let _ = self.manager.unregister(self.paste);
        let _ = self.manager.unregister(self.restore);
    }
}
