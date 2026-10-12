use std::{
    fs,
    io::{ErrorKind, Read},
    path::PathBuf,
};

pub(super) fn data_directory() -> Result<PathBuf, &'static str> {
    let home = std::env::var_os("HOME").ok_or("Could not locate the settings directory.")?;
    Ok(if cfg!(target_os = "macos") {
        PathBuf::from(home).join("Library/Application Support/Copy for AI")
    } else {
        PathBuf::from(home).join(".config/copy-for-ai")
    })
}

pub const LICENSE_TEXT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../LICENSE"));

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Preferences {
    appearance: Appearance,
    show_history: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            appearance: Appearance::System,
            show_history: false,
        }
    }
}

impl Preferences {
    pub fn appearance(self) -> Appearance {
        self.appearance
    }
    pub fn show_history(self) -> bool {
        self.show_history
    }
    pub fn with_appearance(self, appearance: Appearance) -> Self {
        Self { appearance, ..self }
    }
    pub fn with_history(self, show_history: bool) -> Self {
        Self {
            show_history,
            ..self
        }
    }

    fn encode(self) -> String {
        let appearance = match self.appearance {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        };
        format!(
            "version=1\nappearance={appearance}\nshow_history={}\n",
            self.show_history
        )
    }

    fn decode(text: &str) -> Result<Self, &'static str> {
        let mut lines = text.lines();
        if lines.next() != Some("version=1") {
            return Err("Unsupported settings version.");
        }
        let appearance = match lines.next() {
            Some("appearance=system") => Appearance::System,
            Some("appearance=light") => Appearance::Light,
            Some("appearance=dark") => Appearance::Dark,
            _ => return Err("Invalid appearance setting."),
        };
        let show_history = match lines.next() {
            Some("show_history=true") => true,
            Some("show_history=false") => false,
            _ => return Err("Invalid history setting."),
        };
        if lines.next().is_some() {
            return Err("Invalid settings format.");
        }
        Ok(Self {
            appearance,
            show_history,
        })
    }
}

pub struct SettingsService {
    preferences: Preferences,
    store: Option<PathBuf>,
}

impl SettingsService {
    pub fn load() -> Result<Self, &'static str> {
        let path = data_directory()?.join("preferences.conf");
        let preferences = match fs::File::open(&path) {
            Ok(file) => {
                let mut text = String::new();
                file.take(257)
                    .read_to_string(&mut text)
                    .map_err(|_| "Could not read settings.")?;
                if text.len() > 256 {
                    return Err("Settings file is too large.");
                }
                Preferences::decode(&text)?
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Preferences::default(),
            Err(_) => return Err("Could not read settings."),
        };
        Ok(Self {
            preferences,
            store: Some(path),
        })
    }

    pub fn temporary() -> Self {
        Self {
            preferences: Preferences::default(),
            store: None,
        }
    }
    pub fn is_persistent(&self) -> bool {
        self.store.is_some()
    }
    pub fn preferences(&self) -> Preferences {
        self.preferences
    }
    pub fn accept(&mut self, preferences: Preferences) {
        self.preferences = preferences;
    }

    /// Persists only the fixed preference schema, never document or history data.
    pub fn save(
        &self,
        preferences: Preferences,
    ) -> impl Future<Output = Result<Preferences, &'static str>> + Send + 'static {
        let store = self.store.clone();
        async move {
            smol::unblock(move || {
                if let Some(path) = store {
                    let parent = path.parent().ok_or("Could not save settings.")?;
                    fs::create_dir_all(parent)
                        .map_err(|_| "Could not create the settings directory.")?;
                    let temporary = path.with_extension("conf.tmp");
                    fs::write(&temporary, preferences.encode())
                        .map_err(|_| "Could not save settings.")?;
                    fs::rename(&temporary, &path)
                        .map_err(|_| "Could not finish saving settings.")?;
                }
                Ok(preferences)
            })
            .await
        }
    }
}

#[cfg(test)]
#[path = "../../tests/application/settings.rs"]
mod tests;
