use crate::{
    EntityKind, Error, MAX_ENTITIES, MAX_OUTPUT_BYTES, Restored, check_input, marker::MARKER,
};
use std::collections::HashMap;
use uuid::Uuid;
use zeroize::Zeroize;

#[derive(Clone)]
struct Mapping {
    kind: EntityKind,
    original: String,
    token: String,
}

impl Drop for Mapping {
    fn drop(&mut self) {
        self.original.zeroize();
    }
}

/// One explicit conversation. All originals remain in this process's memory.
#[derive(Clone)]
pub struct Session {
    id: Uuid,
    scope: String,
    mappings: Vec<Mapping>,
    counts: HashMap<EntityKind, usize>,
    by_token: HashMap<String, usize>,
    by_value: HashMap<(EntityKind, u64), Vec<usize>>,
    hash_state: std::collections::hash_map::RandomState,
    bytes: usize,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            scope: id.simple().to_string()[..8].to_uppercase(),
            mappings: Vec::new(),
            counts: HashMap::new(),
            by_token: HashMap::new(),
            by_value: HashMap::new(),
            hash_state: std::collections::hash_map::RandomState::new(),
            bytes: 0,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub(crate) fn original(&self, token: &str) -> Option<&str> {
        self.by_token
            .get(token)
            .map(|&i| self.mappings[i].original.as_str())
    }

    fn value_index(&self, kind: EntityKind, original: &str) -> Option<usize> {
        use std::hash::BuildHasher;
        let hash = self.hash_state.hash_one(original);
        self.by_value
            .get(&(kind, hash))?
            .iter()
            .copied()
            .find(|&i| self.mappings[i].original == original)
    }

    pub(crate) fn contains(&self, kind: EntityKind, original: &str) -> bool {
        self.value_index(kind, original).is_some()
    }

    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn scope(&self) -> &str {
        &self.scope
    }

    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    pub fn tokens(&self) -> impl Iterator<Item = (EntityKind, &str)> {
        self.mappings.iter().map(|m| (m.kind, m.token.as_str()))
    }

    /// Replaces exact known markers once; restored originals are never rescanned.
    pub fn restore(&self, input: &str) -> Result<Restored, Error> {
        check_input(input)?;
        let mut text = String::with_capacity(input.len());
        let mut cursor = 0;
        let mut replacements = 0;
        let mut unknown_tokens = Vec::new();
        let mut unknown = std::collections::HashSet::new();
        let mut output_length = input.len();
        for matched in MARKER.find_iter(input) {
            if let Some(mapping) = self
                .by_token
                .get(matched.as_str())
                .map(|&i| &self.mappings[i])
            {
                output_length = output_length - matched.len() + mapping.original.len();
                if output_length > MAX_OUTPUT_BYTES {
                    return Err(Error::OutputTooLarge);
                }
            }
        }
        for matched in MARKER.find_iter(input) {
            text.push_str(&input[cursor..matched.start()]);
            if let Some(mapping) = self
                .by_token
                .get(matched.as_str())
                .map(|&i| &self.mappings[i])
            {
                text.push_str(&mapping.original);
                replacements += 1;
            } else {
                text.push_str(matched.as_str());
                if unknown.insert(matched.as_str()) {
                    if unknown_tokens.len() == MAX_ENTITIES {
                        return Err(Error::TooManyUnknownTokens);
                    }
                    unknown_tokens.push(matched.as_str().to_owned());
                }
            }
            cursor = matched.end();
        }
        text.push_str(&input[cursor..]);
        Ok(Restored {
            text,
            replacements,
            unknown_tokens,
        })
    }

    pub(crate) fn token_for(&mut self, kind: EntityKind, original: &str) -> String {
        if let Some(index) = self.value_index(kind, original) {
            return self.mappings[index].token.clone();
        }
        let count = self.counts.entry(kind).or_default();
        *count += 1;
        let token = format!("[[{}_{}_{}]]", kind.tag(), self.scope, count);
        use std::hash::BuildHasher;
        self.by_value
            .entry((kind, self.hash_state.hash_one(original)))
            .or_default()
            .push(self.mappings.len());
        self.by_token.insert(token.clone(), self.mappings.len());
        self.bytes += original.len();
        self.mappings.push(Mapping {
            kind,
            original: original.to_owned(),
            token: token.clone(),
        });
        token
    }
}
