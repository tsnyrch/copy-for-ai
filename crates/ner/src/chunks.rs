use crate::Error;
use gliner::text::tokenizer::{HFTokenizer, Tokenizer};
use regex::Regex;
use std::{
    collections::HashMap,
    ops::Range,
    sync::Arc,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

pub(crate) struct Word {
    pub(crate) range: Range<usize>,
    pub(crate) ids: Arc<Zeroizing<Vec<u32>>>,
}

pub(crate) struct Windows {
    pub(crate) words: Vec<Word>,
    cursor: usize,
    budget: usize,
}

impl Windows {
    pub(crate) fn prepare(
        text: &str,
        segmenter: &Regex,
        tokenizer: &HFTokenizer,
        budget: usize,
        cancelled: &impl Fn() -> bool,
        deadline: Instant,
        tokenization: &mut Duration,
    ) -> Result<Self, Error> {
        let mut words = Vec::new();
        let mut cache: HashMap<&str, Arc<Zeroizing<Vec<u32>>>> = HashMap::new();
        let mut total_ids = 0;
        for matched in segmenter.find_iter(text) {
            let mut start = matched.start();
            while start < matched.end() {
                if cancelled() {
                    return Err(Error::Cancelled);
                }
                if Instant::now() >= deadline {
                    return Err(Error::Timeout);
                }
                // Bound tokenizer work even for a single document-sized lexical unit.
                let mut end = text[start..matched.end()]
                    .char_indices()
                    .nth(128)
                    .map_or(matched.end(), |(offset, _)| start + offset);
                let ids = loop {
                    let value = &text[start..end];
                    let ids = if let Some(ids) = cache.get(value) {
                        ids.clone()
                    } else {
                        let timer = Instant::now();
                        let mut ids =
                            Zeroizing::new(tokenizer.encode(value).map_err(|_| Error::Inference)?);
                        if ids.is_empty() {
                            ids.extend(tokenizer.encode("[UNK]").map_err(|_| Error::Inference)?);
                        }
                        *tokenization += timer.elapsed();
                        if ids.len() <= budget {
                            total_ids += ids.len();
                            if total_ids > 4 * 1024 * 1024 {
                                return Err(Error::Capacity);
                            }
                            let ids = Arc::new(ids);
                            cache.insert(value, ids.clone());
                            ids
                        } else {
                            let count = value.chars().count();
                            if count <= 1 {
                                return Err(Error::Inference);
                            }
                            end = start
                                + value
                                    .char_indices()
                                    .nth(count / 2)
                                    .ok_or(Error::Inference)?
                                    .0;
                            continue;
                        }
                    };
                    if ids.is_empty() {
                        return Err(Error::Inference);
                    }
                    break ids;
                };
                words.push(Word {
                    range: start..end,
                    ids,
                });
                start = end;
            }
        }
        Ok(Self {
            words,
            cursor: 0,
            budget,
        })
    }

    fn bounds(&self, start: usize) -> Option<(Range<usize>, usize)> {
        if start == self.words.len() {
            return None;
        }
        let mut end = start;
        let mut count = 0;
        while end < self.words.len() && count + self.words[end].ids.len() <= self.budget {
            count += self.words[end].ids.len();
            end += 1;
        }
        let next = if end == self.words.len() {
            end
        } else {
            // GLiNER spans have width <= 12 words. Preserve that context at every seam.
            let mut overlap = end;
            let mut overlap_ids = 0;
            while overlap > start + 1 && end - overlap < 24 {
                let length = self.words[overlap - 1].ids.len();
                if overlap_ids + length > self.budget / 3 {
                    break;
                }
                overlap -= 1;
                overlap_ids += length;
            }
            overlap
        };
        Some((start..end, next))
    }

    pub(crate) fn total(&self) -> usize {
        let mut cursor = 0;
        let mut count = 0;
        while let Some((_, next)) = self.bounds(cursor) {
            count += 1;
            cursor = next;
        }
        count
    }

    pub(crate) fn next(&mut self) -> Option<Range<usize>> {
        let (range, next) = self.bounds(self.cursor)?;
        self.cursor = next;
        Some(range)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_windows_cover_every_word_and_keep_entity_width_at_seams() {
        let words = (0..10_000)
            .map(|index| Word {
                range: index * 4..index * 4 + 3,
                ids: Arc::new(Zeroizing::new(vec![1])),
            })
            .collect();
        let mut windows = Windows {
            words,
            cursor: 0,
            budget: 64,
        };
        let mut covered = vec![false; windows.words.len()];
        let mut previous: Option<Range<usize>> = None;
        while let Some(range) = windows.next() {
            assert!(range.len() <= 64);
            for value in &mut covered[range.clone()] {
                *value = true;
            }
            if let Some(previous) = previous {
                assert!(previous.end - range.start >= 12);
            }
            previous = Some(range);
        }
        assert!(covered.into_iter().all(|value| value));
    }
}
