"""Isolated Nym ONNX experiment; returns UTF-8 byte ranges, never input logs.

This adapter is deliberately outside the production detector. Its fixed taxonomy
has no event class, and its predictions require an independent quality gate.
"""

from __future__ import annotations

from bisect import bisect_right
import json
from pathlib import Path
import time
from typing import Any

import numpy as np
import onnxruntime as ort
from tokenizers import Tokenizer


class NymProbe:
    """Probe a pinned local model without network access or fallback detection."""

    CATEGORY_BY_LABEL = {
        "GIVEN_NAME": "PERSON",
        "SURNAME": "PERSON",
        "COMPANY_NAME": "ORG",
        "CITY": "LOCATION",
        "COUNTRY": "LOCATION",
        "STATE": "LOCATION",
        "STREET_ADDRESS": "ADDRESS",
        "STREET_NAME": "ADDRESS",
        "BUILDING_NUMBER": "ADDRESS",
        "SECONDARY_ADDRESS": "ADDRESS",
        "ZIP_CODE": "ADDRESS",
    }
    supported_categories = frozenset(CATEGORY_BY_LABEL.values())

    def __init__(
        self,
        model_dir: str | Path,
        threads: int = 8,
        window_tokens: int = 480,
        overlap: int = 64,
        threshold: float = 0.5,
        model_file: str = "model.onnx",
    ) -> None:
        if not 0 <= threshold <= 1 or not 0 <= overlap < window_tokens:
            raise ValueError("invalid Nym probe configuration")
        model_dir = Path(model_dir)
        config = json.loads((model_dir / "config.json").read_text())
        labels = config.get("id2label", {})
        self.labels = [labels[str(i)] for i in range(len(labels))]
        if not self.labels or self.labels[0] != "O":
            raise ValueError("invalid Nym BIO label configuration")
        if window_tokens + 2 > config["max_position_embeddings"]:
            raise ValueError("Nym window exceeds the model context")
        started = time.perf_counter()
        self.tokenizer = Tokenizer.from_file(str(model_dir / "tokenizer.json"))
        self.tokenizer.no_truncation()
        self.tokenizer.no_padding()
        self.bos_id = self.tokenizer.token_to_id("<bos>")
        self.eos_id = self.tokenizer.token_to_id("<eos>")
        if self.bos_id is None or self.eos_id is None:
            raise ValueError("Nym tokenizer is missing sequence boundary tokens")
        options = ort.SessionOptions()
        options.intra_op_num_threads = threads
        options.inter_op_num_threads = 1
        options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
        options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
        self.session = ort.InferenceSession(
            str(model_dir / model_file), options, providers=["CPUExecutionProvider"]
        )
        inputs = {item.name: item for item in self.session.get_inputs()}
        if set(inputs) != {"input_ids", "attention_mask"} or any(
            item.type != "tensor(int64)" for item in inputs.values()
        ):
            raise ValueError("unexpected Nym ONNX input contract")
        outputs = self.session.get_outputs()
        if len(outputs) != 1 or outputs[0].name != "logits":
            raise ValueError("unexpected Nym ONNX output contract")
        self.load_ms = (time.perf_counter() - started) * 1000
        self.window_tokens = window_tokens
        self.overlap = overlap
        self.threshold = threshold
        self.last_metrics: dict[str, int | float] = {}

    def detect(self, text: str) -> list[dict[str, Any]]:
        """Return supported model spans over the complete, untruncated input."""
        started = time.perf_counter()
        encoding = self.tokenizer.encode(text, add_special_tokens=False)
        offsets = encoding.offsets
        token_ids = encoding.ids
        if len(offsets) != len(token_ids) or any(
            not 0 <= start <= end <= len(text) for start, end in offsets
        ):
            raise ValueError("invalid Nym tokenizer offsets")
        byte_offsets = [0]
        for character in text:
            byte_offsets.append(byte_offsets[-1] + len(character.encode("utf-8")))
        windows = self._windows(offsets)
        tokenize_ms = (time.perf_counter() - started) * 1000
        inference_ms = 0.0
        decode_ms = 0.0
        found: dict[tuple[int, int, str], dict[str, Any]] = {}
        for start, end in windows:
            ids = np.asarray(
                [[self.bos_id, *token_ids[start:end], self.eos_id]], dtype=np.int64
            )
            run_started = time.perf_counter()
            logits = self.session.run(
                ["logits"], {"input_ids": ids, "attention_mask": np.ones_like(ids)}
            )[0]
            inference_ms += (time.perf_counter() - run_started) * 1000
            decode_started = time.perf_counter()
            if logits.shape != (1, end - start + 2, len(self.labels)):
                raise ValueError("unexpected Nym logits shape")
            if not np.isfinite(logits).all():
                raise ValueError("non-finite Nym logits")
            for span in self._decode(text, logits[0, 1:-1], offsets[start:end]):
                category = self.CATEGORY_BY_LABEL.get(span["label"])
                if category is None:
                    continue
                result = {
                    "start": byte_offsets[span["start"]],
                    "end": byte_offsets[span["end"]],
                    "category": category,
                    "label": span["label"],
                    "score": span["score"],
                }
                key = (result["start"], result["end"], category)
                previous = found.get(key)
                if previous is None or result["score"] > previous["score"]:
                    found[key] = result
            decode_ms += (time.perf_counter() - decode_started) * 1000
        self.last_metrics = {
            "tokenize_ms": tokenize_ms,
            "inference_ms": inference_ms,
            "decode_ms": decode_ms,
            "windows": len(windows),
            "raw_tokens": len(token_ids),
            "inferred_tokens": sum(end - start for start, end in windows),
        }
        return sorted(found.values(), key=lambda item: (item["start"], item["end"]))

    def _windows(self, offsets: list[tuple[int, int]]) -> list[tuple[int, int]]:
        if not offsets:
            return []
        legal = [0]
        covered_end = 0
        for index, (start, end) in enumerate(offsets):
            if index and start >= covered_end:
                legal.append(index)
            covered_end = max(covered_end, end)
        legal.append(len(offsets))
        windows = []
        start = 0
        while start < len(offsets):
            end = legal[bisect_right(legal, start + self.window_tokens) - 1]
            if end <= start:
                raise ValueError("Nym Unicode offset group exceeds the window budget")
            windows.append((start, end))
            if end == len(offsets):
                break
            target = end - self.overlap
            next_start = legal[bisect_right(legal, target) - 1]
            start = next_start if next_start > start else end
        return windows

    def _decode(
        self, text: str, logits: np.ndarray, offsets: list[tuple[int, int]]
    ) -> list[dict[str, Any]]:
        shifted = logits - logits.max(axis=-1, keepdims=True)
        exponentials = np.exp(shifted)
        probabilities = exponentials / exponentials.sum(axis=-1, keepdims=True)
        predictions = logits.argmax(axis=-1)
        spans: list[dict[str, Any]] = []
        current: dict[str, Any] | None = None
        for row, label_id in enumerate(predictions):
            start, end = offsets[row]
            if start == end:
                continue
            label = self.labels[int(label_id)]
            score = float(probabilities[row, label_id])
            if label == "O" or score < self.threshold:
                if current is not None:
                    spans.append(current)
                    current = None
                continue
            prefix, base = label.split("-", 1)
            if current is not None and prefix == "I" and current["label"] == base:
                current["end"] = max(current["end"], end)
                current["score_sum"] += score
                current["count"] += 1
            else:
                if current is not None:
                    spans.append(current)
                current = {
                    "start": start,
                    "end": end,
                    "label": base,
                    "score_sum": score,
                    "count": 1,
                }
        if current is not None:
            spans.append(current)
        merged: list[dict[str, Any]] = []
        for span in spans:
            if merged and merged[-1]["label"] == span["label"]:
                previous = merged[-1]
                gap = text[previous["end"] : span["start"]]
                if span["start"] <= previous["end"] or not gap or gap.isspace():
                    previous["end"] = max(previous["end"], span["end"])
                    previous["score_sum"] += span["score_sum"]
                    previous["count"] += span["count"]
                    continue
            merged.append(span)
        output = []
        for span in merged:
            start, end = span["start"], span["end"]
            while start < end and text[start] in " \t\r\n\v\f":
                start += 1
            while end > start and text[end - 1] in " \t\r\n\v\f":
                end -= 1
            if start < end:
                output.append(
                    {
                        "start": start,
                        "end": end,
                        "label": span["label"],
                        "score": span["score_sum"] / span["count"],
                    }
                )
        return output
