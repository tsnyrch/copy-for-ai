use copy_for_ai_core::{
    CustomRule, DetectedEntity, Detector, Direction, EntityKind, Error, MAX_CUSTOM_RULES,
    Processed, Vault,
};

pub struct Workflow {
    vault: Vault,
    rules: Vec<CustomRule>,
    revision: u64,
}

impl Workflow {
    pub fn new() -> Self {
        Self {
            vault: Vault::new(),
            rules: Vec::new(),
            revision: 0,
        }
    }

    pub fn snapshot(&self) -> WorkflowSnapshot {
        WorkflowSnapshot {
            vault: self.vault.clone(),
            rules: self.rules.clone(),
            revision: self.revision,
        }
    }

    pub fn can_commit(&self, pending: &PendingProcessing) -> bool {
        self.revision == pending.revision
    }

    pub fn commit(&mut self, mut pending: PendingProcessing) -> Result<Processed, Error> {
        if !self.can_commit(&pending) {
            return Err(Error::StaleOperation);
        }
        self.vault = pending
            .vault
            .take()
            .expect("pending vault is consumed once");
        self.revision += 1;
        Ok(pending
            .output
            .take()
            .expect("pending output is consumed once"))
    }

    pub fn scope(&self) -> &str {
        self.vault.scope()
    }
    pub fn entity_count(&self) -> usize {
        self.vault.entity_count()
    }
    pub fn rules(&self) -> &[CustomRule] {
        &self.rules
    }

    pub fn direction(&self, text: &str) -> Result<Direction, Error> {
        self.vault.direction(text)
    }

    #[cfg(test)]
    pub fn process(&mut self, text: &str, entities: &[DetectedEntity]) -> Result<Processed, Error> {
        let result = self
            .vault
            .process(&Detector::new(), text, &self.rules, entities)?;
        self.revision += 1;
        Ok(result)
    }

    pub fn new_session(&mut self) -> Result<(), Error> {
        self.vault.new_session()?;
        self.revision += 1;
        Ok(())
    }
    pub fn session_count(&self) -> usize {
        self.vault.session_count()
    }

    #[cfg(test)]
    pub fn restore(&self, text: &str) -> Result<copy_for_ai_core::Restored, Error> {
        self.vault.restore(text)
    }

    pub fn add_rule(&mut self, kind: EntityKind, value: String) -> Result<(), Error> {
        if self.rules.len() >= MAX_CUSTOM_RULES {
            return Err(Error::TooManyRules);
        }
        if self
            .rules
            .iter()
            .any(|r| r.kind() == kind && r.value() == value)
        {
            return Ok(());
        }
        self.rules.push(CustomRule::new(kind, value)?);
        self.revision += 1;
        Ok(())
    }

    pub fn remove_rule(&mut self, index: usize) {
        if index < self.rules.len() {
            self.rules.remove(index);
            self.revision += 1;
        }
    }

    pub fn forget(&mut self) {
        self.vault.forget();
        self.rules.clear();
        self.revision += 1;
    }
}

// A worker owns a bounded draft; only the UI's validated commit can publish mappings.
pub struct WorkflowSnapshot {
    vault: Vault,
    rules: Vec<CustomRule>,
    revision: u64,
}

pub struct PendingProcessing {
    input: zeroize::Zeroizing<String>,
    vault: Option<Vault>,
    output: Option<Processed>,
    revision: u64,
}

impl PendingProcessing {
    pub fn input(&self) -> &str {
        &self.input
    }
    pub fn output(&self) -> &Processed {
        self.output
            .as_ref()
            .expect("pending output is consumed once")
    }
}

impl Drop for PendingProcessing {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        if let Some(output) = self.output.as_mut() {
            output.text.zeroize();
            output.unknown_tokens.zeroize();
        }
    }
}

impl WorkflowSnapshot {
    pub fn direction(&self, text: &str) -> Result<Direction, Error> {
        self.vault.direction(text)
    }

    #[cfg(test)]
    pub fn finish(
        self,
        detector: &Detector,
        input: zeroize::Zeroizing<String>,
        entities: &[DetectedEntity],
    ) -> Result<PendingProcessing, Error> {
        self.finish_cancellable(detector, input, entities, &|| false)
    }

    pub fn finish_cancellable(
        mut self,
        detector: &Detector,
        input: zeroize::Zeroizing<String>,
        entities: &[DetectedEntity],
        cancelled: &impl Fn() -> bool,
    ) -> Result<PendingProcessing, Error> {
        let output =
            self.vault
                .process_cancellable(detector, &input, &self.rules, entities, cancelled)?;
        Ok(PendingProcessing {
            input,
            vault: Some(self.vault),
            output: Some(output),
            revision: self.revision,
        })
    }
}
