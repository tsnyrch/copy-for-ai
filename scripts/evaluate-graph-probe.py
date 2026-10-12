#!/usr/bin/env python3
"""Compare isolated GLiNER graphs on synthetic or captured synthetic tensor inputs."""

import argparse
import gc
import json
from pathlib import Path
import statistics
import time

import numpy as np
import onnx
import onnxruntime as ort
from onnx import numpy_helper
from tokenizers import Tokenizer


LABELS = ("person", "first name", "organization", "company", "location", "city", "address", "event")
BOUNDARY = "/token_rep_layer/bert_layer/model/embeddings/Mul_output_0"
INPUTS = ("input_ids", "attention_mask", "words_mask", "text_lengths", "span_idx", "span_mask")
SYNTHETIC_WORDS = {
    "cs": "Jan Novák pracuje pro Acme v Praze a plánuje konferenci .".split(),
    "en": "Alice Johnson works at Contoso in London and plans a conference .".split(),
    "de": "Anna Müller arbeitet für Acme in Berlin und plant eine Konferenz .".split(),
    "fr": "Marie Dupont travaille chez Contoso à Paris et prépare une conférence .".split(),
    "ru": "Анна Иванова работает в компании Acme в Москве и готовит конференцию .".split(),
    "ja": list("田中太郎は東京の架空会社で会議を準備する。"),
    "zh": list("王小明在北京的虚构公司组织会议。"),
}


def session(path, threads):
    options = ort.SessionOptions()
    options.intra_op_num_threads = threads
    options.inter_op_num_threads = 1
    options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    options.log_severity_level = 3
    started = time.perf_counter()
    runtime = ort.InferenceSession(str(path), options, providers=["CPUExecutionProvider"])
    return runtime, (time.perf_counter() - started) * 1000


def synthetic_feed(tokenizer, words, budget, max_width=12):
    labels = []
    for label in LABELS:
        labels.extend(tokenizer.encode("<<ENT>>", add_special_tokens=False).ids)
        labels.extend(tokenizer.encode(label, add_special_tokens=False).ids)
    labels.extend(tokenizer.encode("<<SEP>>", add_special_tokens=False).ids)
    ids = [1] + labels
    word_mask = [0] * len(ids)
    word_count = 0
    for index in range(1024):
        encoded = tokenizer.encode(words[index % len(words)], add_special_tokens=False).ids
        if len(ids) + len(encoded) + 1 > budget:
            break
        word_count += 1
        ids.extend(encoded)
        word_mask.extend([word_count] + [0] * (len(encoded) - 1))
    ids.append(2)
    word_mask.append(0)
    spans = np.zeros((1, word_count * max_width, 2), dtype=np.int64)
    masks = np.zeros((1, word_count * max_width), dtype=np.bool_)
    for start in range(word_count):
        for width in range(min(max_width, word_count - start)):
            spans[0, start * max_width + width] = (start, start + width)
            masks[0, start * max_width + width] = True
    return {
        "input_ids": np.array([ids], dtype=np.int64),
        "attention_mask": np.ones((1, len(ids)), dtype=np.int64),
        "words_mask": np.array([word_mask], dtype=np.int64),
        "text_lengths": np.array([[word_count]], dtype=np.int64),
        "span_idx": spans, "span_mask": masks,
    }


def load_feed(path):
    if path.suffix == ".npz":
        with np.load(path, allow_pickle=False) as archive:
            return {name: archive[name] for name in INPUTS}
    data = json.loads(path.read_text())
    data = data.get("inputs", data)
    return {name: np.asarray(data[name], dtype=np.bool_ if name == "span_mask" else np.int64)
            for name in INPUTS}


def threshold_spans(logits, word_count, threshold):
    probabilities = np.float32(1) / (np.float32(1) + np.exp(-logits, dtype=np.float32))
    indices = np.argwhere(probabilities >= np.float32(threshold))
    return {(int(start), int(start + width), int(label)) for _, start, width, label in indices
            if start < word_count and start + width < word_count}


def compare(baseline, candidate, feed, threshold):
    if baseline.shape != candidate.shape or not np.isfinite(candidate).all():
        raise ValueError("Candidate output shape differs or contains non-finite logits")
    difference = np.abs(baseline.astype(np.float64) - candidate.astype(np.float64))
    word_count = int(feed["text_lengths"][0, 0])
    old_spans = threshold_spans(baseline, word_count, threshold)
    new_spans = threshold_spans(candidate, word_count, threshold)
    return {
        "logits_shape": list(baseline.shape), "bitwise_equal": bool(np.array_equal(baseline, candidate)),
        "max_abs": float(difference.max()), "mean_abs": float(difference.mean()),
        "threshold": threshold, "baseline_threshold_spans": len(old_spans),
        "candidate_threshold_spans": len(new_spans), "lost_threshold_spans": len(old_spans - new_spans),
        "gained_threshold_spans": len(new_spans - old_spans),
        "threshold_spans_equal": old_spans == new_spans,
    }


def run_graph(path, feeds, threads, warmups, runs):
    runtime, load_ms = session(path, threads)
    results = {}
    for name, feed in feeds.items():
        for _ in range(warmups):
            runtime.run(["logits"], feed)
        timings = []
        logits = None
        for _ in range(runs):
            started = time.perf_counter()
            logits = runtime.run(["logits"], feed)[0]
            timings.append((time.perf_counter() - started) * 1000)
        results[name] = {"logits": logits, "warm_ms": timings,
                         "median_ms": statistics.median(timings)}
    del runtime
    gc.collect()
    return load_ms, results


def position_gate(candidate, threads):
    model = onnx.load_model(candidate, load_external_data=False)
    table_proto = next((value for value in model.graph.initializer if value.name == "probe/position/table"), None)
    if table_proto is None:
        return None
    table = numpy_helper.to_array(table_proto, base_dir=str(candidate.parent))
    branch, _ = session(candidate.parent / "original-position-branch.onnx", threads)
    results = []
    for length in (1, 2, 31, 128, 129, 255, 256, 257, 384, 511, 512):
        actual = branch.run(None, {BOUNDARY: np.zeros((1, length, 768), dtype=np.float32)})[0]
        results.append({"length": length, "exact": bool(np.array_equal(actual, table[:, :length, :length]))})
    return results


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, nargs="+", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--tokenizer", type=Path)
    parser.add_argument("--inputs", type=Path, nargs="*")
    parser.add_argument("--fixtures", nargs="*")
    parser.add_argument("--threads", type=int, default=8)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--threshold", type=float, default=0.4)
    args = parser.parse_args()
    if args.runs < 1 or args.warmups < 0:
        raise ValueError("Runs must be positive and warmups nonnegative")
    if args.inputs:
        feeds = {path.stem: load_feed(path) for path in args.inputs}
    elif args.tokenizer:
        tokenizer = Tokenizer.from_file(str(args.tokenizer))
        feeds = {f"{language}-{budget}": synthetic_feed(tokenizer, words, budget)
                 for language, words in SYNTHETIC_WORDS.items() for budget in (96, 256)}
    else:
        raise ValueError("Supply a tokenizer for synthetic fixtures or captured synthetic --inputs")
    if args.fixtures:
        feeds = {name: feeds[name] for name in args.fixtures}
    for feed in feeds.values():
        if feed["input_ids"].shape[0] != 1 or feed["input_ids"].shape[1] > 512:
            raise ValueError("Graph experiment requires batch one and sequence length <=512")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    baseline_load, baseline = run_graph(args.baseline, feeds, args.threads, args.warmups, args.runs)
    result = {"schema": 1, "runtime": ort.__version__, "provider": "CPUExecutionProvider",
              "threads": args.threads, "warmups": args.warmups, "runs": args.runs,
              "baseline": str(args.baseline), "baseline_load_ms": baseline_load, "candidates": []}
    for path in args.candidate:
        try:
            load_ms, candidate = run_graph(path, feeds, args.threads, args.warmups, args.runs)
        except Exception as error:
            result["candidates"].append({"path": str(path), "status": "runtime_failed",
                                         "error_type": type(error).__name__, "error": str(error)})
            args.output.write_text(json.dumps(result, indent=2) + "\n")
            print(json.dumps({"candidate": path.parent.name, "status": "runtime_failed",
                              "error_type": type(error).__name__}))
            continue
        samples = []
        for name, feed in feeds.items():
            sample = compare(baseline[name]["logits"], candidate[name]["logits"], feed, args.threshold)
            sample.update({"fixture": name, "sequence_length": feed["input_ids"].shape[1],
                           "word_count": int(feed["text_lengths"][0, 0]),
                           "baseline_warm_ms": baseline[name]["warm_ms"],
                           "candidate_warm_ms": candidate[name]["warm_ms"],
                           "speedup": baseline[name]["median_ms"] / candidate[name]["median_ms"]})
            samples.append(sample)
        candidate_result = {"path": str(path), "load_ms": load_ms, "samples": samples,
                            "position_gate": position_gate(path, args.threads)}
        result["candidates"].append(candidate_result)
        args.output.write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps({"candidate": path.parent.name, "max_abs": max(s["max_abs"] for s in samples),
                          "lost_threshold_spans": sum(s["lost_threshold_spans"] for s in samples),
                          "gained_threshold_spans": sum(s["gained_threshold_spans"] for s in samples),
                          "median_speedup": statistics.median(s["speedup"] for s in samples),
                          "position_gate": (all(s["exact"] for s in candidate_result["position_gate"])
                                            if candidate_result["position_gate"] is not None else None)}))


if __name__ == "__main__":
    main()
