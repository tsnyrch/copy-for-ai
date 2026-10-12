use regex::Regex;
use std::sync::LazyLock;

pub(crate) static MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\[[A-Za-z][A-Za-z0-9_]{0,160}\]\]").expect("built-in marker regex")
});
