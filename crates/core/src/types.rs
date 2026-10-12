use std::{fmt, ops::Range};
use thiserror::Error;
use zeroize::Zeroize;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EntityKind {
    Person,
    Organization,
    Location,
    Address,
    Event,
    Uuid,
    Email,
    Phone,
    Iban,
    PersonalId,
    Secret,
    Term,
}

impl EntityKind {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Person => "PERSON",
            Self::Organization => "ORG",
            Self::Location => "LOCATION",
            Self::Address => "ADDRESS",
            Self::Event => "EVENT",
            Self::Uuid => "UUID",
            Self::Email => "EMAIL",
            Self::Phone => "PHONE",
            Self::Iban => "IBAN",
            Self::PersonalId => "ID",
            Self::Secret => "SECRET",
            Self::Term => "TERM",
        }
    }
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.tag())
    }
}

// Intentionally no Debug or serialization: this contains original sensitive text.
#[derive(Clone)]
pub struct CustomRule {
    kind: EntityKind,
    value: String,
}

impl CustomRule {
    pub fn new(kind: EntityKind, value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if value.trim().is_empty() || value.len() > 1024 || value.contains("[[") {
            return Err(Error::InvalidRule);
        }
        Ok(Self { kind, value })
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn kind(&self) -> EntityKind {
        self.kind
    }
}

impl Drop for CustomRule {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum Error {
    #[error("Text exceeds the 1 MiB anonymization / 32 MiB restoration limit. Paste stopped.")]
    InputTooLarge,
    #[error("Output exceeds the 32 MiB limit. Operation stopped.")]
    OutputTooLarge,
    #[error("Vault has too many entries. Forget stored mappings.")]
    VaultFull,
    #[error("In-memory vault exceeds the 8 MiB original-text limit.")]
    VaultBytesFull,
    #[error("Text contains too many detections. Operation stopped.")]
    TooManyCandidates,
    #[error("Text contains more than 4096 unknown markers.")]
    TooManyUnknownTokens,
    #[error("State changed during processing. Operation stopped.")]
    StaleOperation,
    #[error("Operation cancelled.")]
    Cancelled,
    #[error("A term must contain 1–1024 bytes and must not include the marker prefix [[.")]
    InvalidRule,
    #[error("You can add at most 256 custom terms.")]
    TooManyRules,
    #[error("Detector returned invalid UTF-8 spans. Operation stopped.")]
    InvalidSpan,
    #[error("Vault supports at most 16 sessions. Forget old mappings.")]
    TooManySessions,
}

#[derive(Clone, Debug)]
pub struct Finding {
    pub range: Range<usize>,
    pub kind: EntityKind,
    pub token: String,
}

// Output can contain unmatched PII. Do not derive Debug or log it.
pub struct Anonymized {
    pub text: String,
    pub findings: Vec<Finding>,
}

pub struct Restored {
    pub text: String,
    pub replacements: usize,
    pub unknown_tokens: Vec<String>,
}

/// Byte offsets in the original UTF-8 input. No original content is stored here.
#[derive(Clone, Debug)]
pub struct DetectedEntity {
    pub range: Range<usize>,
    pub kind: EntityKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Anonymize,
    Restore,
    Unresolved,
}

pub struct Processed {
    pub text: String,
    pub direction: Direction,
    pub replacements: usize,
    pub unknown_tokens: Vec<String>,
}
