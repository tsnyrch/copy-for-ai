use crate::{Error, LABELS, chunks::Word};
use composable::{Composable, composed};
use gliner::{
    model::{
        input::encoded::EncodedInput, input::tensors::span::SpanTensors, output, params::Parameters,
    },
    text::{
        token::Token,
        tokenizer::{HFTokenizer, Tokenizer},
    },
};
use ndarray::Array2;
use regex::Regex;
use std::{ops::Range, path::Path};

pub(crate) struct LocalPipeline {
    pub(crate) segmenter: Regex,
    pub(crate) tokenizer: HFTokenizer,
    label_ids: Vec<u32>,
}

impl LocalPipeline {
    pub(crate) fn new(path: &Path) -> Result<Self, Error> {
        let units = r"\p{Han}\p{Hiragana}\p{Katakana}\p{Thai}\p{Lao}\p{Khmer}\p{Myanmar}";
        let word = format!(r"[\w&&[^{units}]]");
        let segmenter = Regex::new(&format!(r"[{units}]\p{{M}}*|{word}+(?:[-_]{word}+)*|\S"))
            .map_err(|_| Error::Load)?;
        let tokenizer = HFTokenizer::from_file(path).map_err(|_| Error::Load)?;
        let mut label_ids = Vec::new();
        for label in LABELS {
            label_ids.extend(tokenizer.encode("<<ENT>>").map_err(|_| Error::Load)?);
            label_ids.extend(tokenizer.encode(label).map_err(|_| Error::Load)?);
        }
        label_ids.extend(tokenizer.encode("<<SEP>>").map_err(|_| Error::Load)?);
        Ok(Self {
            segmenter,
            tokenizer,
            label_ids,
        })
    }

    pub(crate) fn prompt_length(&self) -> usize {
        self.label_ids.len() + 2
    }

    pub(crate) fn encode(
        &self,
        text: &str,
        words: &[Word],
        ranges: &[Range<usize>],
        max_width: usize,
    ) -> Result<(SpanTensors<'static>, Vec<usize>), Error> {
        let num_words = ranges.iter().map(Range::len).max().unwrap_or(0);
        let num_tokens = ranges
            .iter()
            .map(|r| {
                self.prompt_length() + words[r.clone()].iter().map(|w| w.ids.len()).sum::<usize>()
            })
            .max()
            .unwrap_or(0);
        let shape = (ranges.len(), num_tokens);
        let mut input_ids = Array2::zeros(shape);
        let mut attention_masks = Array2::zeros(shape);
        let mut word_masks = Array2::zeros(shape);
        let mut text_lengths = Array2::zeros((ranges.len(), 1));
        let mut texts = Vec::new();
        let mut tokens = Vec::new();
        let mut offsets = Vec::new();
        for (row, range) in ranges.iter().enumerate() {
            let selected = &words[range.clone()];
            let bytes = chunk_range(text, words, range)?;
            let offset = bytes.start;
            let end = bytes.end;
            offsets.push(offset);
            texts.push(text[offset..end].to_owned());
            tokens.push(
                selected
                    .iter()
                    .map(|w| {
                        Token::new(
                            w.range.start - offset,
                            w.range.end - offset,
                            &text[w.range.clone()],
                        )
                    })
                    .collect(),
            );
            text_lengths[[row, 0]] = selected.len() as i64;
            input_ids[[row, 0]] = 1;
            attention_masks[[row, 0]] = 1;
            let mut column = 1;
            for id in &self.label_ids {
                input_ids[[row, column]] = i64::from(*id);
                attention_masks[[row, column]] = 1;
                column += 1;
            }
            for (word, encoded) in selected.iter().enumerate() {
                for (piece, id) in encoded.ids.iter().enumerate() {
                    input_ids[[row, column]] = i64::from(*id);
                    attention_masks[[row, column]] = 1;
                    if piece == 0 {
                        word_masks[[row, column]] = word as i64 + 1;
                    }
                    column += 1;
                }
            }
            input_ids[[row, column]] = 2;
            attention_masks[[row, column]] = 1;
        }
        let encoded = EncodedInput {
            texts,
            tokens,
            entities: LABELS.iter().map(|s| (*s).to_owned()).collect(),
            num_words,
            num_tokens,
            input_ids,
            attention_masks,
            word_masks,
            text_lengths,
        };
        Ok((
            SpanTensors::from(encoded, max_width).map_err(|_| Error::Inference)?,
            offsets,
        ))
    }

    pub(crate) fn decode(
        &self,
        output: ort::session::SessionOutputs<'_, '_>,
        context: gliner::model::pipeline::context::EntityContext,
        params: &Parameters,
    ) -> Result<output::decoded::SpanOutput, Error> {
        composed![
            output::tensors::SessionOutputToTensors::default(),
            output::decoded::span::TensorsToDecoded::new(params.threshold, params.max_width),
            output::decoded::sort::SpanSort::default(),
            output::decoded::greedy::GreedySearch::new(
                params.flat_ner,
                params.dup_label,
                params.multi_label
            )
        ]
        .apply((output, context))
        .map_err(|_| Error::Inference)
    }
}

pub(crate) fn chunk_range(
    text: &str,
    words: &[Word],
    range: &Range<usize>,
) -> Result<Range<usize>, Error> {
    let selected = words.get(range.clone()).ok_or(Error::Inference)?;
    let start = if range.start == 0 {
        0
    } else {
        selected.first().ok_or(Error::Inference)?.range.start
    };
    let end = if range.end == words.len() {
        text.len()
    } else {
        words.get(range.end).ok_or(Error::Inference)?.range.start
    };
    Ok(start..end)
}
