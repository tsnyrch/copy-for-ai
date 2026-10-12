use crate::{
    Anonymized, CustomRule, DetectedEntity, EntityKind, Error, Finding, MAX_CANDIDATES,
    MAX_CUSTOM_RULES, MAX_ENTITIES, MAX_OUTPUT_BYTES, MAX_VAULT_BYTES, Session,
    check_anonymization_input,
    marker::MARKER,
    validators::{iban_length, valid_czech_id},
};
use regex::Regex;
use std::{collections::BTreeMap, ops::Range};

pub(crate) struct Capacity {
    pub(crate) entities: usize,
    pub(crate) bytes: usize,
}

struct Candidate {
    range: Range<usize>,
    kind: EntityKind,
    priority: u8,
}

struct Pattern {
    regex: Regex,
    kind: EntityKind,
    priority: u8,
    capture: usize,
}

pub struct Detector {
    patterns: Vec<Pattern>,
}

impl Default for Detector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector {
    pub fn new() -> Self {
        let specs = [
            (
                r"(?i)\b[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}\b",
                EntityKind::Uuid,
                80,
                0,
            ),
            (
                r"-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----[\s\S]*?-----END (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----",
                EntityKind::Secret,
                90,
                0,
            ),
            (
                r"\b(?:sk-(?:proj-)?[A-Za-z0-9_-]{16,}|sk_live_[A-Za-z0-9]{16,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[A-Z0-9]{16}|xox[baprs]-[A-Za-z0-9-]{16,})\b",
                EntityKind::Secret,
                90,
                0,
            ),
            (
                r#"(?i)\b(?:password|passwd|api[_-]?key|access[_-]?token|client[_-]?secret|secret[_-]?key)\s*[:=]\s*["']?([^\s"';,}]{4,})"#,
                EntityKind::Secret,
                90,
                1,
            ),
            (
                r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b",
                EntityKind::Secret,
                90,
                0,
            ),
            (
                r"(?i)\b[a-z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+\b",
                EntityKind::Email,
                70,
                0,
            ),
            (
                r"\b[A-Z]{2}[0-9]{2}(?: ?[A-Z0-9]){11,30}\b",
                EntityKind::Iban,
                80,
                0,
            ),
            (r"\b[0-9]{6}/?[0-9]{3,4}\b", EntityKind::PersonalId, 80, 0),
            (
                r"\+[1-9][0-9](?:[ .-]?[0-9]){7,13}\b",
                EntityKind::Phone,
                50,
                0,
            ),
            (
                r"\b[67][0-9]{2}(?:[ .-]?[0-9]{3}){2}\b",
                EntityKind::Phone,
                50,
                0,
            ),
        ];
        Self {
            patterns: specs
                .into_iter()
                .map(|(regex, kind, priority, capture)| Pattern {
                    regex: Regex::new(regex).expect("built-in detection regex"),
                    kind,
                    priority,
                    capture,
                })
                .collect(),
        }
    }

    pub fn anonymize(
        &self,
        input: &str,
        session: &mut Session,
        rules: &[CustomRule],
    ) -> Result<Anonymized, Error> {
        self.anonymize_with_entities(input, session, rules, &[])
    }

    pub fn anonymize_with_entities(
        &self,
        input: &str,
        session: &mut Session,
        rules: &[CustomRule],
        entities: &[DetectedEntity],
    ) -> Result<Anonymized, Error> {
        let available = MAX_ENTITIES - session.len();
        let available_bytes = MAX_VAULT_BYTES - session.bytes();
        self.anonymize_bounded(
            input,
            session,
            rules,
            entities,
            Capacity {
                entities: available,
                bytes: available_bytes,
            },
            &|| false,
        )
    }

    pub(crate) fn anonymize_bounded(
        &self,
        input: &str,
        session: &mut Session,
        rules: &[CustomRule],
        entities: &[DetectedEntity],
        capacity: Capacity,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Anonymized, Error> {
        check_anonymization_input(input)?;
        if cancelled() {
            return Err(Error::Cancelled);
        }
        if entities.len() > MAX_CANDIDATES {
            return Err(Error::TooManyCandidates);
        }
        if entities
            .iter()
            .any(|e| e.range.is_empty() || input.get(e.range.clone()).is_none())
        {
            return Err(Error::InvalidSpan);
        }
        if rules.len() > MAX_CUSTOM_RULES {
            return Err(Error::TooManyRules);
        }
        let protected: Vec<_> = MARKER.find_iter(input).map(|m| m.range()).collect();
        let mut candidates = Vec::new();
        let mut surfaces = std::collections::HashMap::new();
        for entity in entities {
            if cancelled() {
                return Err(Error::Cancelled);
            }
            candidates.push(Candidate {
                range: entity.range.clone(),
                kind: entity.kind,
                priority: 60,
            });
            surfaces
                .entry(&input[entity.range.clone()])
                .or_insert_with(Vec::new)
                .push((entity.kind, 60));
        }
        for rule in rules {
            surfaces
                .entry(rule.value())
                .or_insert_with(Vec::new)
                .push((rule.kind(), 100));
        }
        let surfaces: Vec<_> = surfaces
            .into_iter()
            .map(|(value, mut kinds)| {
                kinds.sort_unstable_by_key(|(kind, priority)| (kind.tag(), *priority));
                kinds.dedup();
                (value, kinds)
            })
            .collect();
        if !surfaces.is_empty() {
            let matcher = aho_corasick::AhoCorasick::new(surfaces.iter().map(|(value, _)| value))
                .map_err(|_| Error::TooManyCandidates)?;
            for matched in matcher.find_overlapping_iter(input) {
                if cancelled() {
                    return Err(Error::Cancelled);
                }
                let range = matched.range();
                if word_boundary(input, &range) {
                    for &(kind, priority) in &surfaces[matched.pattern().as_usize()].1 {
                        if candidates.len() == MAX_CANDIDATES {
                            return Err(Error::TooManyCandidates);
                        }
                        candidates.push(Candidate {
                            range: range.clone(),
                            kind,
                            priority,
                        });
                    }
                }
            }
        }
        for pattern in &self.patterns {
            for captures in pattern.regex.captures_iter(input) {
                if cancelled() {
                    return Err(Error::Cancelled);
                }
                let matched = captures.get(pattern.capture).expect("known capture group");
                let mut range = matched.range();
                match pattern.kind {
                    EntityKind::PersonalId if !valid_czech_id(matched.as_str()) => continue,
                    EntityKind::Iban => {
                        let Some(length) = iban_length(matched.as_str()) else {
                            continue;
                        };
                        range.end = range.start + length;
                    }
                    _ => {}
                }
                if candidates.len() == MAX_CANDIDATES {
                    return Err(Error::TooManyCandidates);
                }
                candidates.push(Candidate {
                    range,
                    kind: pattern.kind,
                    priority: pattern.priority,
                });
            }
        }
        if candidates.len() > MAX_CANDIDATES {
            return Err(Error::TooManyCandidates);
        }
        candidates.retain(|candidate| !intersects_protected(candidate, &protected));
        candidates.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| b.range.len().cmp(&a.range.len()))
                .then_with(|| a.range.start.cmp(&b.range.start))
        });
        let mut accepted: BTreeMap<usize, Candidate> = BTreeMap::new();
        for candidate in candidates {
            if cancelled() {
                return Err(Error::Cancelled);
            }
            if !accepted
                .range(..candidate.range.end)
                .next_back()
                .is_some_and(|(_, other)| other.range.end > candidate.range.start)
            {
                accepted.insert(candidate.range.start, candidate);
            }
        }
        let accepted: Vec<_> = accepted.into_values().collect();

        let output_bound = accepted.iter().fold(input.len(), |length, candidate| {
            length - candidate.range.len() + candidate.kind.tag().len() + session.scope().len() + 10
        });
        if output_bound > MAX_OUTPUT_BYTES {
            return Err(Error::OutputTooLarge);
        }

        // Check capacity before any mutation. An error cannot create a partial map.
        let mut new_values = std::collections::HashSet::new();
        for candidate in &accepted {
            if cancelled() {
                return Err(Error::Cancelled);
            }
            let value = &input[candidate.range.clone()];
            if !session.contains(candidate.kind, value) {
                new_values.insert((candidate.kind, value));
            }
        }
        if new_values.len() > capacity.entities {
            return Err(Error::VaultFull);
        }
        if new_values
            .iter()
            .map(|(_, value)| value.len())
            .sum::<usize>()
            > capacity.bytes
        {
            return Err(Error::VaultBytesFull);
        }
        if cancelled() {
            return Err(Error::Cancelled);
        }
        let mut text = String::with_capacity(output_bound);
        let mut findings = Vec::with_capacity(accepted.len());
        let mut cursor = 0;
        for candidate in accepted {
            text.push_str(&input[cursor..candidate.range.start]);
            let token = session.token_for(candidate.kind, &input[candidate.range.clone()]);
            text.push_str(&token);
            cursor = candidate.range.end;
            findings.push(Finding {
                range: candidate.range,
                kind: candidate.kind,
                token,
            });
        }
        text.push_str(&input[cursor..]);
        Ok(Anonymized { text, findings })
    }
}

fn intersects_protected(candidate: &Candidate, protected: &[Range<usize>]) -> bool {
    let first = protected.partition_point(|p| p.end <= candidate.range.start);
    let end = protected.partition_point(|p| p.start < candidate.range.end);
    if first >= end {
        return false;
    }
    // Disjoint markers are fully enclosed iff both extreme intersections are enclosed.
    !(candidate.kind == EntityKind::Secret
        && candidate.range.start <= protected[first].start
        && candidate.range.end >= protected[end - 1].end
        && candidate.range != protected[first])
}

fn word_boundary(input: &str, range: &Range<usize>) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let value = &input[range.clone()];
    let before = input[..range.start].chars().next_back();
    let after = input[range.end..].chars().next();
    !(value.chars().next().is_some_and(word) && before.is_some_and(word)
        || value.chars().next_back().is_some_and(word) && after.is_some_and(word))
}
