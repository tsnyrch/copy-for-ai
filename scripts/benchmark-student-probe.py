#!/usr/bin/env python3
"""Measure synthetic whole-text student cost; an untrained head proves no PII quality."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import runpy
import statistics
import time

os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
os.environ["TOKENIZERS_PARALLELISM"] = "false"
os.environ["PYTORCH_ENABLE_MPS_FALLBACK"] = "0"

import coremltools as ct
import numpy as np
import torch
from transformers import XLMRobertaTokenizerFast

from student_probe_model import load_student, load_trained_student


def encode(text, tokenizer, bucket, overlap, trim_whitespace=False):
    encoded = tokenizer(text, add_special_tokens=False, return_offsets_mapping=True)
    ids, offsets = encoded["input_ids"], encoded["offset_mapping"]
    if any(a < 0 or b < a or b > len(text) for a, b in offsets):
        raise ValueError("Tokenizer offset contract is not original character coordinates")
    if trim_whitespace:
        trimmed = []
        for start, end in offsets:
            while start < end and text[start].isspace():
                start += 1
            while end > start and text[end - 1].isspace():
                end -= 1
            trimmed.append((start, end))
        offsets = trimmed
    prefix = [0]
    for character in text:
        prefix.append(prefix[-1] + len(character.encode("utf-8")))
    byte_offsets = [(prefix[a], prefix[b]) for a, b in offsets]
    capacity, windows = bucket - 2, []
    stride = capacity - overlap
    for start in range(0, len(ids), stride):
        end = min(start + capacity, len(ids))
        row = np.full(bucket, tokenizer.pad_token_id, dtype=np.int32)
        row[:end - start + 2] = [tokenizer.cls_token_id, *ids[start:end], tokenizer.sep_token_id]
        mask = np.zeros(bucket, dtype=np.int32)
        mask[:end - start + 2] = 1
        windows.append((start, end, row, mask))
        if end == len(ids):
            break
    return byte_offsets, windows


def decode(labels, offsets, text):
    ranges, current = [], None
    for label, (start, end) in zip(labels, offsets):
        if start == end:
            continue
        kind = (int(label) - 1) // 2 if label else None
        begins = bool(label and int(label) % 2 == 1)
        if current is not None and (kind != current[2] or begins):
            ranges.append(current)
            current = None
        if kind is not None:
            current = (current[0] if current else start, end, kind)
    if current:
        ranges.append(current)
    raw = text.encode("utf-8")
    for start, end, _ in ranges:
        raw[start:end].decode("utf-8")
    return len(ranges)


def process(text, tokenizer, runtime, batch, bucket, overlap, reference=None, trim_whitespace=False):
    started = time.perf_counter()
    offsets, windows = encode(text, tokenizer, bucket, overlap, trim_whitespace=trim_whitespace)
    encoded = time.perf_counter()
    labels = np.zeros(len(offsets), dtype=np.int64)
    assigned = np.zeros(len(offsets), dtype=np.uint8)
    prediction_ms, max_abs, argmax_mismatch = 0.0, None, None
    for position in range(0, len(windows), batch):
        group = windows[position:position + batch]
        padded = group + [group[-1]] * (batch - len(group))
        feed = {
            "input_ids": np.stack([row[2] for row in padded]),
            "attention_mask": np.stack([row[3] for row in padded]),
        }
        before = time.perf_counter()
        output = runtime.predict(feed)["token_logits"]
        prediction_ms += (time.perf_counter() - before) * 1000
        if output.shape != (batch, bucket, 11) or not np.isfinite(output).all():
            raise ValueError("Student output shape/finite-value contract failed")
        if reference is not None and position == 0:
            with torch.inference_mode():
                expected = reference(torch.from_numpy(feed["input_ids"]).long(),
                                     torch.from_numpy(feed["attention_mask"]).long()).numpy()
            max_abs = float(np.max(np.abs(expected - output)))
            argmax_mismatch = int(np.count_nonzero(expected.argmax(-1) != output.argmax(-1)))
            if max_abs > 0.0001:
                raise ValueError("CoreML student logits differ from CPU reference beyond 1e-4")
        choices = output.argmax(axis=-1)
        for index, (start, end, _, _) in enumerate(group):
            left = start if start == 0 else start + overlap // 2
            right = end if end == len(offsets) else end - (overlap - overlap // 2)
            labels[left:right] = choices[index, 1 + left - start:1 + right - start]
            assigned[left:right] += 1
    inferred = time.perf_counter()
    if not np.all(assigned == 1):
        raise ValueError("Every original text token must be owned by exactly one window")
    spans = decode(labels, offsets, text)
    finished = time.perf_counter()
    return {
        "input_bytes": len(text.encode("utf-8")), "tokens": len(offsets),
        "valid_windows": len(windows), "encoded_windows": ((len(windows) + batch - 1) // batch) * batch,
        "window_reuse": 0, "all_tokens_owned_once": True,
        "final_token_end_byte": offsets[-1][1] if offsets else 0,
        "tokenization_ms": (encoded - started) * 1000, "coreml_predict_ms": prediction_ms,
        "inference_and_dispatch_ms": (inferred - encoded) * 1000,
        "decode_ms": (finished - inferred) * 1000, "wall_ms": (finished - started) * 1000,
        ("decoded_span_count" if trim_whitespace else "untrained_span_count"): spans, "reference_max_abs": max_abs,
        "reference_argmax_mismatch": argmax_mismatch,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model-dir", type=Path, required=True)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--checkpoint", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--layers", type=int, choices=(6, 12), required=True)
    parser.add_argument("--batch", type=int, choices=(1, 8), default=8)
    parser.add_argument("--bucket", type=int, default=256)
    parser.add_argument("--overlap", type=int, default=32)
    parser.add_argument("--sizes", type=int, nargs="+", default=[102400])
    parser.add_argument("--runs", type=int, default=3)
    args = parser.parse_args()
    if args.overlap < 0 or args.overlap >= args.bucket - 2 or args.runs < 1:
        parser.error("Invalid overlap or run count")
    torch.set_num_threads(8)
    torch.set_num_interop_threads(1)
    report = {"quality_verified": False, "core_verified": False, "synthetic_only": True,
              "model_head_trained": args.checkpoint is not None, "layers": args.layers, "batch": args.batch,
              "bucket": args.bucket, "overlap": args.overlap, "cases": []}
    report["decoding_policy"] = "per_token_unicode_whitespace_trim_empty_skip" if args.checkpoint else "original_raw_offsets_empty_skip"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    def save():
        report["peak_rss_bytes"] = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    try:
        started = time.perf_counter()
        if args.checkpoint is not None:
            reference, metadata = load_trained_student(args.model_dir, args.checkpoint, layers=args.layers, head_seed=17)
            report["checkpoint_sha256"] = metadata["checkpoint_sha256"]
            report["training_report_sha256"] = metadata["training_report_sha256"]
        else:
            reference, metadata = load_student(args.model_dir, layers=args.layers, head_seed=17)
        report["weights"] = metadata
        report["reference_load_ms"] = (time.perf_counter() - started) * 1000
        tokenizer = XLMRobertaTokenizerFast(vocab_file=str(args.model_dir / "sentencepiece.bpe.model"))
        started = time.perf_counter()
        runtime = ct.models.MLModel(str(args.package), compute_units=ct.ComputeUnit.CPU_AND_GPU)
        report["coreml_load_compile_ms"] = (time.perf_counter() - started) * 1000
        report["package_metadata"] = dict(runtime.user_defined_metadata)
        package = report["package_metadata"]
        expected_untrained = "true" if args.checkpoint is None else "false"
        if (package.get("copy-ai.student.head-untrained") != expected_untrained
                or package.get("copy-ai.student.head-seed") != "17"
                or package.get("copy-ai.precision") != "fp32"
                or json.loads(package.get("copy-ai.student.layers", "[]")) != metadata["selected_layer_indices"]
                or package.get("copy-ai.student.source-sha256") != metadata["model_source_sha256"]):
            raise ValueError("CoreML package is not the explicitly selected FP32 student probe")
        if args.checkpoint is not None and (
                package.get("copy-ai.student.checkpoint-sha256") != metadata["checkpoint_sha256"]
                or package.get("copy-ai.student.training-report-sha256") != metadata["training_report_sha256"]
                or package.get("copy-ai.student.teacher-applied") != "false"
                or package.get("copy-ai.student.model-id") != metadata["model_id"]
                or package.get("copy-ai.student.revision") != metadata["revision"]):
            raise ValueError("CoreML trained checkpoint provenance does not match the verified local checkpoint")
        shapes = [list(value.type.multiArrayType.shape) for value in runtime.get_spec().description.input]
        if shapes != [[args.batch, args.bucket], [args.batch, args.bucket]]:
            raise ValueError("CoreML input shapes differ from benchmark contract")
        generator = runpy.run_path(str(Path(__file__).with_name("benchmark-speed.py")))["varied"]
        probe = generator(4096)
        report["numerical_probe"] = process(probe, tokenizer, runtime, args.batch, args.bucket,
                                            args.overlap, reference=reference, trim_whitespace=args.checkpoint is not None)
        for size in args.sizes:
            text = generator(size)
            case = {"size": size, "sha256": hashlib.sha256(text.encode()).hexdigest()}
            case["first"] = process(text, tokenizer, runtime, args.batch, args.bucket, args.overlap,
                                    trim_whitespace=args.checkpoint is not None)
            case["warm"] = [process(text, tokenizer, runtime, args.batch, args.bucket, args.overlap,
                                    trim_whitespace=args.checkpoint is not None)
                            for _ in range(args.runs)]
            case["warm_median_ms"] = statistics.median(row["wall_ms"] for row in case["warm"])
            report["cases"].append(case)
            save()
            print(json.dumps({"bytes": size, "warm_median_ms": case["warm_median_ms"],
                              "windows": case["first"]["valid_windows"]}), flush=True)
        report["completed"] = True
    except Exception as error:
        report.update(completed=False, error_type=type(error).__name__, error=str(error))
        save()
        raise
    save()


if __name__ == "__main__":
    main()
