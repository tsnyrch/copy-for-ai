#!/usr/bin/env python3
"""Emit trained student byte ranges for the existing Rust core validation bridge."""

import argparse
import json
import os
from pathlib import Path
import resource
import time

os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
os.environ["TOKENIZERS_PARALLELISM"] = "false"
os.environ["PYTORCH_ENABLE_MPS_FALLBACK"] = "0"

import coremltools as ct
import numpy as np
import torch
from transformers import XLMRobertaTokenizerFast

from student_probe_model import CATEGORIES, file_sha256, load_trained_student


SAFE_ERROR_CODES = frozenset({
    "offset_count_mismatch", "unsupported_zero_width_or_out_of_bounds_raw_offset",
    "unsupported_overlapping_raw_offsets", "decoded_range_contract_failed",
    "student_output_contract_failed", "reference_output_contract_failed",
    "trained_coreml_cpu_delta_exceeds_1e_minus_4", "trained_coreml_cpu_owned_argmax_mismatch",
    "token_ownership_contract_failed",
    "student_range_capacity_exceeded", "trained_package_metadata_mismatch",
    "trained_package_layer_mismatch", "trained_package_input_shape_mismatch",
    "trained_package_output_mismatch", "frozen_corpus_file_set_mismatch",
    "invalid_case_id", "frozen_corpus_byte_count_mismatch",
})


class OffsetContractError(ValueError):
    pass


class NumericalContractError(ValueError):
    def __init__(self, code, metrics):
        super().__init__(code)
        self.metrics = metrics


def encode(text, tokenizer, bucket, overlap):
    encoded = tokenizer(text, add_special_tokens=False, return_offsets_mapping=True)
    ids, raw = encoded["input_ids"], encoded["offset_mapping"]
    if len(ids) != len(raw):
        raise OffsetContractError("offset_count_mismatch")
    prefix = [0]
    for character in text:
        prefix.append(prefix[-1] + len(character.encode("utf-8")))
    offsets, previous, whitespace_empty = [], 0, 0
    for start, end in raw:
        if not 0 <= start < end <= len(text):
            raise OffsetContractError("unsupported_zero_width_or_out_of_bounds_raw_offset")
        if start < previous:
            raise OffsetContractError("unsupported_overlapping_raw_offsets")
        previous = end
        while start < end and text[start].isspace():
            start += 1
        while end > start and text[end - 1].isspace():
            end -= 1
        whitespace_empty += start == end
        offsets.append((prefix[start], prefix[end]))
    capacity, windows = bucket - 2, []
    for start in range(0, len(ids), capacity - overlap):
        end = min(start + capacity, len(ids))
        row = np.full(bucket, tokenizer.pad_token_id, dtype=np.int32)
        row[:end - start + 2] = [tokenizer.cls_token_id, *ids[start:end], tokenizer.sep_token_id]
        mask = np.zeros(bucket, dtype=np.int32)
        mask[:end - start + 2] = 1
        windows.append((start, end, row, mask))
        if end == len(ids):
            break
    return offsets, windows, whitespace_empty


def decode(labels, offsets, text):
    ranges, current = [], None
    for label, (start, end) in zip(labels, offsets, strict=True):
        if start == end:
            continue
        kind = (int(label) - 1) // 2 if label else None
        begins = bool(label and int(label) % 2 == 1)
        if current is not None and (kind != current[2] or begins):
            ranges.append(current)
            current = None
        if kind is not None:
            current = (current[0] if current else start, end, kind)
    if current is not None:
        ranges.append(current)
    raw, previous = text.encode("utf-8"), 0
    for start, end, kind in ranges:
        if not previous <= start < end <= len(raw) or not 0 <= kind < len(CATEGORIES):
            raise OffsetContractError("decoded_range_contract_failed")
        raw[start:end].decode("utf-8")
        previous = end
    return ranges


def process(text, tokenizer, runtime, reference, args):
    started = time.perf_counter()
    offsets, windows, empty = encode(text, tokenizer, args.bucket, args.overlap)
    encoded = time.perf_counter()
    labels = np.zeros(len(offsets), dtype=np.int64)
    assigned = np.zeros(len(offsets), dtype=np.uint8)
    prediction_ms, max_abs, mismatch = 0.0, 0.0, 0
    owned_max_abs, ignored_max_abs, owned_mismatch, owned_tokens = 0.0, 0.0, 0, 0
    verified_batches = 0
    for position in range(0, len(windows), args.batch):
        group = windows[position:position + args.batch]
        feed_rows = group + [group[-1]] * (args.batch - len(group))
        feed = {"input_ids": np.stack([row[2] for row in feed_rows]),
                "attention_mask": np.stack([row[3] for row in feed_rows])}
        before = time.perf_counter()
        output = runtime.predict(feed)["token_logits"]
        prediction_ms += (time.perf_counter() - before) * 1000
        if output.shape != (args.batch, args.bucket, 11) or not np.isfinite(output).all():
            raise ValueError("student_output_contract_failed")
        if position == 0:
            with torch.inference_mode():
                expected = reference(torch.from_numpy(feed["input_ids"]).long(),
                                     torch.from_numpy(feed["attention_mask"]).long()).numpy()
            if expected.shape != output.shape or not np.isfinite(expected).all():
                raise ValueError("reference_output_contract_failed")
            difference = np.abs(expected - output)
            unequal = expected.argmax(-1) != output.argmax(-1)
            max_abs = float(np.max(difference))
            mismatch = int(np.count_nonzero(unequal))
            owned = np.zeros((args.batch, args.bucket), dtype=np.bool_)
            for row, (start, end, _, _) in enumerate(group):
                left = start if start == 0 else start + args.overlap // 2
                right = end if end == len(offsets) else end - (args.overlap - args.overlap // 2)
                owned[row, 1 + left - start:1 + right - start] = True
            owned_tokens = int(np.count_nonzero(owned))
            owned_mismatch = int(np.count_nonzero(unequal[owned]))
            owned_max_abs = float(difference[owned].max()) if owned_tokens else 0.0
            ignored_max_abs = float(difference[~owned].max()) if np.any(~owned) else 0.0
            verified_batches += 1
            if owned_max_abs > 0.0001 or owned_mismatch:
                code = "trained_coreml_cpu_delta_exceeds_1e_minus_4" if owned_max_abs > 0.0001 else "trained_coreml_cpu_owned_argmax_mismatch"
                raise NumericalContractError(code, {
                    "reference_max_abs": max_abs if np.isfinite(max_abs) else None,
                    "reference_max_abs_owned_tokens": owned_max_abs if np.isfinite(owned_max_abs) else None,
                    "reference_max_abs_unowned_or_padding_positions": ignored_max_abs if np.isfinite(ignored_max_abs) else None,
                    "reference_delta_tolerance": 0.0001,
                    "reference_argmax_mismatch_all_batch_positions": mismatch,
                    "reference_argmax_mismatch_owned_tokens": owned_mismatch,
                    "reference_argmax_mismatch_unowned_or_padding_positions": mismatch - owned_mismatch,
                    "reference_owned_original_tokens": owned_tokens,
                    "verified_batch_start_window": position,
                    "valid_windows_in_verified_batch": len(group),
                    "padding_rows_in_verified_batch": args.batch - len(group),
                })
        choices = output.argmax(axis=-1)
        for index, (start, end, _, _) in enumerate(group):
            left = start if start == 0 else start + args.overlap // 2
            right = end if end == len(offsets) else end - (args.overlap - args.overlap // 2)
            labels[left:right] = choices[index, 1 + left - start:1 + right - start]
            assigned[left:right] += 1
    if not np.all(assigned == 1):
        raise ValueError("token_ownership_contract_failed")
    decoded_start = time.perf_counter()
    ranges = decode(labels, offsets, text)
    if len(ranges) > 2 * 1024 * 1024:
        raise ValueError("student_range_capacity_exceeded")
    metrics = {"bytes": len(text.encode("utf-8")), "tokens": len(offsets),
               "valid_windows": len(windows), "encoded_windows": ((len(windows) + args.batch - 1) // args.batch) * args.batch,
               "window_reuse": 0, "all_tokens_owned_once": True, "whitespace_empty_tokens_skipped": empty,
               "tokenization_ms": (encoded - started) * 1000, "coreml_predict_ms": prediction_ms,
               "decode_ms": (time.perf_counter() - decoded_start) * 1000,
               "wall_with_reference_ms": (time.perf_counter() - started) * 1000,
               "cpu_reference_batches": verified_batches, "reference_max_abs": max_abs,
               "reference_max_abs_owned_tokens": owned_max_abs,
               "reference_max_abs_unowned_or_padding_positions": ignored_max_abs,
               "reference_argmax_mismatch_owned_tokens": owned_mismatch,
               "reference_argmax_mismatch_all_batch_positions": mismatch,
               "reference_argmax_mismatch_unowned_or_padding_positions": mismatch - owned_mismatch,
               "reference_owned_original_tokens": owned_tokens, "entities": len(ranges)}
    return ranges, metrics


def package_check(runtime, metadata, args):
    package = dict(runtime.user_defined_metadata)
    expected = {"copy-ai.student.model-id": metadata["model_id"],
                "copy-ai.student.revision": metadata["revision"],
                "copy-ai.student.source-sha256": metadata["model_source_sha256"],
                "copy-ai.student.checkpoint-sha256": metadata["checkpoint_sha256"],
                "copy-ai.student.training-report-sha256": metadata["training_report_sha256"],
                "copy-ai.student.head-seed": str(metadata["head_seed"]),
                "copy-ai.student.head-untrained": "false", "copy-ai.precision": "fp32",
                "copy-ai.student.teacher-applied": str(metadata["teacher_applied"]).lower(),
                "copy-ai.batch-size": str(args.batch), "copy-ai.sequence-length": str(args.bucket)}
    if any(package.get(key) != value for key, value in expected.items()):
        raise ValueError("trained_package_metadata_mismatch")
    if json.loads(package.get("copy-ai.student.layers", "[]")) != metadata["selected_layer_indices"]:
        raise ValueError("trained_package_layer_mismatch")
    description = runtime.get_spec().description
    shapes = {value.name: list(value.type.multiArrayType.shape) for value in description.input}
    if shapes != {"input_ids": [args.batch, args.bucket], "attention_mask": [args.batch, args.bucket]}:
        raise ValueError("trained_package_input_shape_mismatch")
    if [value.name for value in description.output] != ["token_logits"]:
        raise ValueError("trained_package_output_mismatch")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model-directory", type=Path, required=True)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--corpus", type=Path, default=Path("/private/tmp/copy-ai-speed-2026-10-13/corpus/quality"))
    parser.add_argument("--log", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--layers", type=int, choices=(6, 12), default=12)
    parser.add_argument("--batch", type=int, choices=(1, 8), default=8)
    parser.add_argument("--bucket", type=int, default=256)
    parser.add_argument("--overlap", type=int, default=32)
    args = parser.parse_args()
    if not 4 <= args.bucket <= 512 or not 0 <= args.overlap < args.bucket - 2:
        parser.error("invalid window configuration")
    if args.log.exists() or args.report.exists():
        parser.error("evaluation requires fresh output files")
    torch.set_num_threads(8)
    torch.set_num_interop_threads(1)
    report = {"completed": False, "synthetic_only": True, "core_verified": False,
              "gold_compared": False, "production_candidate_accepted": False, "cases": [],
              "layers": args.layers, "batch": args.batch, "bucket": args.bucket, "overlap": args.overlap,
              "offset_policy": "per_token_unicode_whitespace_trim; trim_empty_whitespace_skip_without_entity_close",
              "unsupported_offset_contract": "raw_zero_width_or_overlapping_offsets_fail_entire_evaluation",
              "decoding_policy": "global_stitched_argmax_BIO11; no_gold_snapping_or_entity_trimming",
              "reference_policy": "first_padded_batch_per_case; finite_shape_all_positions; owned_original_tokens_delta_tolerance_1e-4_and_argmax_equal; unowned_padding_dummy_differences_diagnostic_only",
              "timing_policy": "quality_evaluation_including_CPU_reference; not_latency_benchmark"}
    current_case = None
    try:
        reference, metadata = load_trained_student(args.model_directory, args.checkpoint, layers=args.layers)
        report["model"] = metadata
        tokenizer = XLMRobertaTokenizerFast(vocab_file=str(args.model_directory / "sentencepiece.bpe.model"))
        runtime = ct.models.MLModel(str(args.package), compute_units=ct.ComputeUnit.CPU_AND_GPU)
        package_check(runtime, metadata, args)
        manifest_path = args.corpus / "manifest.json"
        manifest = json.loads(manifest_path.read_text())
        report["corpus_manifest_sha256"] = file_sha256(manifest_path)
        report["expected_cases"] = len(manifest)
        if not manifest or {path.stem for path in args.corpus.glob("*.txt")} != set(manifest):
            raise ValueError("frozen_corpus_file_set_mismatch")
        bridge = []
        for current_case in sorted(manifest):
            if not current_case or not all(character.isascii() and (character.isalnum() or character in "-_") for character in current_case):
                raise ValueError("invalid_case_id")
            with (args.corpus / f"{current_case}.txt").open("rb") as handle:
                raw = handle.read(1024 * 1024 + 1)
            if len(raw) > 1024 * 1024 or len(raw) != manifest[current_case]["bytes"]:
                raise ValueError("frozen_corpus_byte_count_mismatch")
            text = raw.decode("utf-8")
            spans, metrics = process(text, tokenizer, runtime, reference, args)
            report["cases"].append({"case": current_case, **metrics})
            bridge.append(f"case={current_case} entities={len(spans)} round_trip=pending\n")
            bridge.extend(f"span case={current_case} start={start} end={end} kind={CATEGORIES[kind]}\n" for start, end, kind in spans)
        report["completed"] = True
        report["aggregate"] = {key: sum(row[key] for row in report["cases"]) for key in
                               ("bytes", "tokens", "valid_windows", "entities", "coreml_predict_ms", "wall_with_reference_ms")}
        report["reference_max_abs"] = max(row["reference_max_abs"] for row in report["cases"])
        report["reference_max_abs_owned_tokens"] = max(row["reference_max_abs_owned_tokens"] for row in report["cases"])
        report["reference_argmax_mismatch_owned_tokens"] = sum(row["reference_argmax_mismatch_owned_tokens"] for row in report["cases"])
        args.log.parent.mkdir(parents=True, exist_ok=True)
        args.log.write_text("".join(bridge))
    except Exception as error:
        report.update(completed=False, error_type=type(error).__name__, failed_case=current_case)
        code = error.args[0] if len(error.args) == 1 else None
        if isinstance(code, str) and code in SAFE_ERROR_CODES:
            report["error_code"] = code
            if isinstance(error, OffsetContractError):
                report["unsupported_offset_code"] = code
        if isinstance(error, NumericalContractError):
            report["numerical_failure"] = error.metrics
    report["peak_rss_bytes"] = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2))
    print(json.dumps({"completed": report["completed"], "cases": len(report["cases"]),
                      "error_type": report.get("error_type")}))
    if not report["completed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
