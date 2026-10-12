use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == "branding/logo.png" {
            return Ok(Some(Cow::Borrowed(include_bytes!("../../assets/logo.png"))));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = gpui_kit::assets::Assets.list(path)?;
        if "branding/logo.png".starts_with(path) {
            assets.push("branding/logo.png".into());
        }
        Ok(assets)
    }
}
