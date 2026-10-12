//! Local, reversible text pseudonymization without UI, clipboard, network or disk access.
mod detector;
mod marker;
mod session;
mod types;
mod validators;
mod vault;

pub use detector::Detector;
pub use session::Session;
pub use types::{
    Anonymized, CustomRule, DetectedEntity, Direction, EntityKind, Error, Finding, Processed,
    Restored,
};
pub use vault::Vault;

pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_VAULT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_CANDIDATES: usize = 2 * 1024 * 1024;
pub const MAX_ENTITIES: usize = 4096;
pub const MAX_CUSTOM_RULES: usize = 256;

fn check_input(input: &str) -> Result<(), Error> {
    if input.len() > MAX_OUTPUT_BYTES {
        Err(Error::InputTooLarge)
    } else {
        Ok(())
    }
}

pub fn check_anonymization_input(input: &str) -> Result<(), Error> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(Error::InputTooLarge);
    }
    Ok(())
}
