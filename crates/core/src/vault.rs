use crate::{
    CustomRule, DetectedEntity, Detector, Direction, EntityKind, Error, MAX_ENTITIES,
    MAX_OUTPUT_BYTES, MAX_VAULT_BYTES, Processed, Restored, Session, check_anonymization_input,
    check_input, marker::MARKER,
};

const MAX_SESSIONS: usize = 16;

/// Bounded, memory-only conversation registry. Rotation retains older mappings.
#[derive(Clone)]
pub struct Vault {
    sessions: Vec<Session>,
}

impl Default for Vault {
    fn default() -> Self {
        Self::new()
    }
}

impl Vault {
    pub fn new() -> Self {
        Self {
            sessions: vec![Session::new()],
        }
    }

    pub fn scope(&self) -> &str {
        self.active().scope()
    }
    pub fn entity_count(&self) -> usize {
        self.sessions.iter().map(Session::len).sum()
    }
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
    pub fn tokens(&self) -> impl Iterator<Item = (EntityKind, &str)> {
        self.sessions.iter().flat_map(Session::tokens)
    }

    pub fn new_session(&mut self) -> Result<(), Error> {
        if self.sessions.len() == MAX_SESSIONS {
            return Err(Error::TooManySessions);
        }
        let session = loop {
            let candidate = Session::new();
            if self.sessions.iter().all(|s| s.scope() != candidate.scope()) {
                break candidate;
            }
        };
        self.sessions.push(session);
        Ok(())
    }

    pub fn forget(&mut self) {
        *self = Self::new();
    }

    pub fn direction(&self, input: &str) -> Result<Direction, Error> {
        check_input(input)?;
        let mut unknown = false;
        for marker in MARKER.find_iter(input) {
            if self.original(marker.as_str()).is_some() {
                return Ok(Direction::Restore);
            }
            unknown = true;
        }
        if !unknown {
            check_anonymization_input(input)?;
        }
        Ok(if unknown {
            Direction::Unresolved
        } else {
            Direction::Anonymize
        })
    }

    pub fn process(
        &mut self,
        detector: &Detector,
        input: &str,
        rules: &[CustomRule],
        entities: &[DetectedEntity],
    ) -> Result<Processed, Error> {
        self.process_cancellable(detector, input, rules, entities, &|| false)
    }

    pub fn process_cancellable(
        &mut self,
        detector: &Detector,
        input: &str,
        rules: &[CustomRule],
        entities: &[DetectedEntity],
        cancelled: &impl Fn() -> bool,
    ) -> Result<Processed, Error> {
        if cancelled() {
            return Err(Error::Cancelled);
        }
        let direction = self.direction(input)?;
        if direction != Direction::Anonymize {
            let result = self.restore(input)?;
            return Ok(Processed {
                text: result.text,
                direction,
                replacements: result.replacements,
                unknown_tokens: result.unknown_tokens,
            });
        }
        let available = MAX_ENTITIES - self.entity_count();
        let available_bytes =
            MAX_VAULT_BYTES - self.sessions.iter().map(Session::bytes).sum::<usize>();
        let result = detector.anonymize_bounded(
            input,
            self.sessions
                .last_mut()
                .expect("vault always has active session"),
            rules,
            entities,
            crate::detector::Capacity {
                entities: available,
                bytes: available_bytes,
            },
            cancelled,
        )?;
        Ok(Processed {
            text: result.text,
            direction,
            replacements: result.findings.len(),
            unknown_tokens: Vec::new(),
        })
    }

    /// Exact, single-pass restoration across all retained sessions.
    pub fn restore(&self, input: &str) -> Result<Restored, Error> {
        check_input(input)?;
        let mut length = input.len();
        for marker in MARKER.find_iter(input) {
            if let Some(original) = self.original(marker.as_str()) {
                length = length - marker.len() + original.len();
            }
        }
        if length > MAX_OUTPUT_BYTES {
            return Err(Error::OutputTooLarge);
        }
        let mut text = String::with_capacity(length);
        let mut cursor = 0;
        let mut replacements = 0;
        let mut unknown_tokens = Vec::new();
        let mut unknown = std::collections::HashSet::new();
        for marker in MARKER.find_iter(input) {
            text.push_str(&input[cursor..marker.start()]);
            if let Some(original) = self.original(marker.as_str()) {
                text.push_str(original);
                replacements += 1;
            } else {
                text.push_str(marker.as_str());
                if unknown.insert(marker.as_str()) {
                    if unknown_tokens.len() == MAX_ENTITIES {
                        return Err(Error::TooManyUnknownTokens);
                    }
                    unknown_tokens.push(marker.as_str().to_owned());
                }
            }
            cursor = marker.end();
        }
        text.push_str(&input[cursor..]);
        Ok(Restored {
            text,
            replacements,
            unknown_tokens,
        })
    }

    fn active(&self) -> &Session {
        self.sessions
            .last()
            .expect("vault always has an active session")
    }
    fn original(&self, token: &str) -> Option<&str> {
        self.sessions
            .iter()
            .find_map(|session| session.original(token))
    }
}
