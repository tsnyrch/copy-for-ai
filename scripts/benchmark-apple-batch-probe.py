#!/usr/bin/env python3
"""Screen static FP32 Core ML encoder batches with the original ONNX head."""

import argparse
import importlib.metadata
import json
from pathlib import Path
import runpy
import statistics
import time
import traceback

import coremltools as ct
import numpy as np
from tokenizers import Tokenizer

from apple_probe_weights import ONNX_SHA256, file_sha256

BOUNDARY = "/token_rep_layer/projection/Add_output_0"


def session(path, threads, ort):
    options = ort.SessionOptions()
    options.intra_op_num_threads = threads
    options.inter_op_num_threads = 1
    options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    options.log_severity_level = 3
    for key in ("session.intra_op.allow_spinning", "session.inter_op.allow_spinning"):
        options.add_session_config_entry(key, "0")
    started = time.perf_counter()
    runtime = ort.InferenceSession(str(path), options, providers=["CPUExecutionProvider"])
    return runtime, (time.perf_counter() - started) * 1000


def pack(rows, inputs, bucket):
    result = {}
    for name in inputs:
        arrays = [feed[name][0] for feed in rows]
        shape = list(arrays[0].shape)
        if name in ("input_ids", "attention_mask", "words_mask"):
            shape[0] = bucket
        elif name in ("span_idx", "span_mask"):
            shape[0] = max(array.shape[0] for array in arrays)
        value = np.zeros((len(rows), *shape), dtype=arrays[0].dtype)
        for index, array in enumerate(arrays):
            slices = (index, *(slice(0, length) for length in array.shape))
            value[slices] = array
        result[name] = value
    return result


def pipeline(encoder, tail, tail_inputs, rows, inputs, bucket, mode):
    started = time.perf_counter()
    stacked = pack(rows, inputs, bucket)
    gpu_feed = {name: stacked[name].astype(np.int32) for name in ("input_ids", "attention_mask")}
    prepared = time.perf_counter()
    embeddings = encoder.predict(gpu_feed)["token_embeddings"]
    encoded = time.perf_counter()
    if embeddings.shape != (len(rows), bucket, 512) or not np.isfinite(embeddings).all():
        raise ValueError("Encoder output shape or finite-value check failed")
    head_ms = 0.0
    if mode == "batch":
        feed = {name: value for name, value in stacked.items() if name in tail_inputs}
        feed[BOUNDARY] = np.asarray(embeddings, dtype=np.float32)
        head_started = time.perf_counter()
        combined = tail.run(None, feed)[0]
        head_ms = (time.perf_counter() - head_started) * 1000
        results = [combined[index:index + 1, :int(row["text_lengths"][0, 0])].copy()
                   for index, row in enumerate(rows)]
    else:
        results = []
        for index, row in enumerate(rows):
            feed = {name: value for name, value in row.items() if name in tail_inputs}
            feed[BOUNDARY] = np.asarray(embeddings[index:index + 1, :row["input_ids"].shape[1]], dtype=np.float32)
            head_started = time.perf_counter()
            results.append(tail.run(None, feed)[0])
            head_ms += (time.perf_counter() - head_started) * 1000
    finished = time.perf_counter()
    return results, {
        "prepare_ms": (prepared - started) * 1000,
        "encoder_batch_ms": (encoded - prepared) * 1000,
        "head_ms": head_ms,
        "postprocess_ms": (finished - encoded) * 1000 - head_ms,
        "wall_ms": (finished - started) * 1000,
    }


def anticipated_plan(encoder):
    try:
        plan = ct.models.MLComputePlan.load_from_path(encoder.get_compiled_model_path(), ct.ComputeUnit.CPU_AND_GPU)
        counts, weights = {}, {}
        def visit(block):
            for operation in block.operations:
                usage = plan.get_compute_device_usage_for_mlprogram_operation(operation)
                device = type(usage.preferred_compute_device).__name__ if usage else "unknown"
                counts[device] = counts.get(device, 0) + 1
                cost = plan.get_estimated_cost_for_mlprogram_operation(operation)
                if cost:
                    weights[device] = weights.get(device, 0) + cost.weight
                for child in operation.blocks:
                    visit(child)
        for function in plan.model_structure.program.functions.values():
            visit(function.block)
        return {"preferred_operation_counts": counts, "estimated_cost_weights": weights, "actual_hardware_trace": False}
    except Exception as error:
        return {"unavailable_error_type": type(error).__name__, "actual_hardware_trace": False}


def run(args):
    helpers = runpy.run_path(str(Path(__file__).with_name("evaluate-graph-probe.py")))
    inputs = helpers["INPUTS"]
    report = {
        "status": "running", "synthetic_only": True, "documents_or_core_verified": False,
        "batch_size": args.batch_size, "sequence_bucket": args.bucket_size, "head_mode": args.head_mode,
        "precision": args.precision, "compute_units": "CPU_AND_GPU", "boundary": BOUNDARY,
        "threads": args.threads, "threshold": args.threshold,
        "ort_session_policy": {"allow_spinning": False, "intra_op_threads": args.threads, "inter_op_threads": 1,
                               "graph_optimization": "ALL", "provider": "CPUExecutionProvider"},
        "measurement": "window throughput; not document latency",
        "versions": {name: importlib.metadata.version(name) for name in ("coremltools", "numpy", "onnxruntime")},
        "cases": [],
    }
    try:
        report["stage"] = "verify_artifacts"
        manifest = json.loads((args.head.parent / "manifest.json").read_text())
        hashes = {
            "source": file_sha256(args.model), "tail": file_sha256(args.head),
            "tail_weights": file_sha256(args.head.parent / "weights.bin"),
            "tokenizer": file_sha256(args.model.parent / "tokenizer.json"),
        }
        if hashes["source"] != ONNX_SHA256 or manifest["source_sha256"] != ONNX_SHA256:
            raise ValueError("Original model or tail source fingerprint differs from pinned ONNX")
        if hashes["tail"] != manifest["model_sha256"] or hashes["tail_weights"] != manifest["weights_sha256"]:
            raise ValueError("Tail artifact fingerprint mismatch")
        if manifest["boundary"]["name"] != BOUNDARY or manifest["boundary"]["shape"][-1] != 512:
            raise ValueError("Tail boundary differs from the exact original encoder projection")
        report["artifact_sha256"] = hashes
        report["tail_manifest_sha256"] = file_sha256(args.head.parent / "manifest.json")
        report["stage"] = "load_runtimes"
        baseline, report["original_ort_load_ms"] = session(args.model, args.threads, helpers["ort"])
        tail, report["tail_ort_load_ms"] = session(args.head, args.threads, helpers["ort"])
        tail_inputs = {value.name for value in tail.get_inputs()}
        if BOUNDARY not in tail_inputs:
            raise ValueError("Tail session has no verified encoder boundary input")
        started = time.perf_counter()
        encoder = ct.models.MLModel(str(args.coreml_package), compute_units=ct.ComputeUnit.CPU_AND_GPU)
        report["coreml_load_compile_ms"] = (time.perf_counter() - started) * 1000
        metadata = encoder.user_defined_metadata
        if metadata.get("copy-ai.precision") != args.precision:
            raise ValueError("CoreML precision metadata differs from the explicitly requested candidate")
        shape = [args.batch_size, args.bucket_size]
        spec = encoder.get_spec()
        if any(list(value.type.multiArrayType.shape) != shape for value in spec.description.input):
            raise ValueError("CoreML input shape differs from the requested static batch/bucket")
        report["coreml_package"] = str(args.coreml_package.resolve())
        report["anticipated_compute_plan"] = anticipated_plan(encoder)
        tokenizer = Tokenizer.from_file(str(args.model.parent / "tokenizer.json"))
        cases = []
        for budget in args.budgets:
            for language, words in helpers["SYNTHETIC_WORDS"].items():
                cases.append((language, budget, helpers["synthetic_feed"](tokenizer, words, budget)))
            if args.batch_size == 8:
                words = helpers["SYNTHETIC_WORDS"]["cs"]
                cases.append(("cs-extra", budget, helpers["synthetic_feed"](tokenizer, words, budget)))
        reference = [baseline.run(None, case[2])[0] for case in cases]
        groups = []
        for offset in range(0, len(cases), args.batch_size):
            real = cases[offset:offset + args.batch_size]
            padded = real + [real[-1]] * (args.batch_size - len(real))
            groups.append((offset, len(real), [case[2] for case in padded]))
        report["valid_windows"] = len(cases)
        report["encoded_windows_per_round"] = len(groups) * args.batch_size
        report["unscored_batch_padding_rows"] = len(groups) * args.batch_size - len(cases)
        report["stage"] = "first_predictions_and_parity"
        first = []
        for offset, count, rows in groups:
            predictions, timing = pipeline(encoder, tail, tail_inputs, rows, inputs, args.bucket_size, args.head_mode)
            first.append({"offset": offset, "valid_windows": count, **timing})
            for index in range(count):
                language, budget, feed = cases[offset + index]
                report["cases"].append({
                    "language": language, "budget": budget, "tokens": feed["input_ids"].shape[1],
                    "words": int(feed["text_lengths"][0, 0]),
                    **helpers["compare"](reference[offset + index], predictions[index], feed, args.threshold),
                })
        report["first_batches"] = first
        report["stage"] = "warm_throughput"
        rounds = []
        for _ in range(args.runs):
            started = time.perf_counter()
            timings = []
            for _, _, rows in groups:
                _, timing = pipeline(encoder, tail, tail_inputs, rows, inputs, args.bucket_size, args.head_mode)
                timings.append(timing)
            rounds.append({
                **{name: sum(timing[name] for timing in timings) for name in timings[0]},
                "round_wall_ms": (time.perf_counter() - started) * 1000,
            })
        report["warm_rounds"] = rounds
        report["warm_median_ms"] = {name: statistics.median(row[name] for row in rounds) for name in rounds[0]}
        report["warm_wall_ms_per_valid_window"] = report["warm_median_ms"]["round_wall_ms"] / len(cases)
        report["warm_head_ms_per_valid_window"] = report["warm_median_ms"]["head_ms"] / len(cases)
        report["warm_encoder_ms_per_valid_window"] = report["warm_median_ms"]["encoder_batch_ms"] / len(cases)
        report["warm_valid_windows_per_second"] = 1000 / report["warm_wall_ms_per_valid_window"]
        report["parity_passed"] = all(case["threshold_spans_equal"] for case in report["cases"])
        report["status"] = "window_screening_passed" if report["parity_passed"] else "parity_failed"
        report["stage"] = "complete"
    except Exception as error:
        report["status"] = "failed"
        report["error"] = {"type": type(error).__name__}
        report["error_frames"] = [{"file": f.filename, "line": f.lineno, "function": f.name}
                                  for f in traceback.extract_tb(error.__traceback__)[-6:]]
    finally:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in ("status", "stage", "batch_size", "head_mode")}))
    return 0 if report["status"] == "window_screening_passed" else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--head", type=Path, required=True)
    parser.add_argument("--coreml-package", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--batch-size", type=int, choices=(1, 4, 8), required=True)
    parser.add_argument("--bucket-size", type=int, default=256)
    parser.add_argument("--head-mode", choices=("serial", "batch"), default="batch")
    parser.add_argument("--precision", choices=("fp32", "mixed-linear"), default="fp32")
    parser.add_argument("--budgets", type=int, nargs="+", default=[256])
    parser.add_argument("--threads", type=int, default=8)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--threshold", type=float, default=0.4)
    args = parser.parse_args()
    if args.runs < 1 or args.threads < 1 or any(budget < 32 or budget > args.bucket_size for budget in args.budgets):
        parser.error("Positive runs/threads and budgets within 32..bucket-size are required")
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
