#!/usr/bin/env python3
"""Screen identical GLiNER weights on CPU/MPS using synthetic tensor feeds."""

import argparse
import json
import os
from pathlib import Path
import resource
import runpy
import statistics
import time

import numpy as np
import torch
from tokenizers import Tokenizer

from apple_probe_weights import SpanWrapper, file_sha256, load_span_model


class MpsEncoder(torch.nn.Module):
    """An explicit GPU encoder/CPU head boundary; no backend fallback."""

    def __init__(self, encoder):
        super().__init__()
        self.encoder = encoder.to("mps")
        self.samples = []

    def forward(self, input_ids, attention_mask, *args, **kwargs):
        torch.mps.synchronize()
        started = time.perf_counter()
        ids = input_ids.to("mps")
        mask = attention_mask.to("mps")
        torch.mps.synchronize()
        uploaded = time.perf_counter()
        result = self.encoder(ids, mask, *args, **kwargs)
        torch.mps.synchronize()
        encoded = time.perf_counter()
        result = result.to("cpu")
        torch.mps.synchronize()
        downloaded = time.perf_counter()
        self.samples.append({
            "upload_ms": (uploaded - started) * 1000,
            "encoder_ms": (encoded - uploaded) * 1000,
            "download_ms": (downloaded - encoded) * 1000,
        })
        return result


class CoremlEncoder(torch.nn.Module):
    def __init__(self, package, bucket, compute_units):
        super().__init__()
        import coremltools as ct

        units = ct.ComputeUnit.ALL if compute_units == "all" else ct.ComputeUnit.CPU_AND_GPU
        self.runtime = ct.models.MLModel(str(package), compute_units=units)
        self.bucket = bucket
        self.samples = []
        self.plan_summary = {"actual_hardware_trace": False}
        try:
            from coremltools.models.compute_plan import MLComputePlan

            plan = MLComputePlan.load_from_path(self.runtime.get_compiled_model_path(), units)
            counts, costs = {}, {}

            def visit(block):
                for operation in block.operations:
                    usage = plan.get_compute_device_usage_for_mlprogram_operation(operation)
                    device = type(usage.preferred_compute_device).__name__ if usage else "unknown"
                    counts[device] = counts.get(device, 0) + 1
                    cost = plan.get_estimated_cost_for_mlprogram_operation(operation)
                    if cost:
                        costs[device] = costs.get(device, 0) + cost.weight
                    for child in operation.blocks:
                        visit(child)

            for function in plan.model_structure.program.functions.values():
                visit(function.block)
            self.plan_summary.update(preferred_operation_counts=counts, estimated_cost_weights=costs)
        except Exception as error:
            self.plan_summary["unavailable_reason"] = type(error).__name__ + ": " + str(error)

    def forward(self, input_ids, attention_mask, *args, **kwargs):
        if args or kwargs or input_ids.shape[0] != 1 or input_ids.shape[1] > self.bucket:
            raise ValueError("CoreML probe accepts only the explicit batch-1 static bucket")
        started = time.perf_counter()
        length = input_ids.shape[1]
        padding = ((0, 0), (0, self.bucket - length))
        feed = {
            "input_ids": np.pad(input_ids.numpy().astype(np.int32), padding),
            "attention_mask": np.pad(attention_mask.numpy().astype(np.int32), padding),
        }
        uploaded = time.perf_counter()
        output = self.runtime.predict(feed)["token_embeddings"]
        encoded = time.perf_counter()
        result = torch.from_numpy(np.array(output[:, :length], copy=True))
        downloaded = time.perf_counter()
        self.samples.append({
            "input_prepare_ms": (uploaded - started) * 1000,
            "coreml_predict_ms": (encoded - uploaded) * 1000,
            "output_copy_ms": (downloaded - encoded) * 1000,
        })
        return result


class OnnxHeadWrapper(torch.nn.Module):
    def __init__(self, encoder, path, helpers, threads):
        super().__init__()
        self.encoder = encoder
        manifest = json.loads((path.parent / "manifest.json").read_text())
        artifacts = {path.name: manifest["model_sha256"], "weights.bin": manifest["weights_sha256"]}
        for name, fingerprint in artifacts.items():
            if file_sha256(path.parent / name) != fingerprint:
                raise ValueError("Extracted head artifact fingerprint mismatch")
        self.boundary = manifest["boundary"]["name"]
        self.runtime, self.load_ms = helpers["session"](path, threads)
        self.input_names = {value.name for value in self.runtime.get_inputs()}

    def forward(self, input_ids, attention_mask, words_mask, text_lengths, span_idx, span_mask):
        values = (input_ids, attention_mask, words_mask, text_lengths, span_idx, span_mask)
        embeddings = self.encoder(input_ids, attention_mask)
        feed = {name: value.numpy() for name, value in zip(INPUTS, values) if name in self.input_names}
        feed[self.boundary] = embeddings.numpy()
        return torch.from_numpy(self.runtime.run(None, feed)[0])


def timed_forward(wrapper, feed, device, gpu):
    args = [torch.from_numpy(feed[name]).to(device) for name in INPUTS]
    if gpu:
        torch.mps.synchronize()
    started = time.perf_counter()
    with torch.inference_mode():
        logits = wrapper(*args)
    if gpu:
        torch.mps.synchronize()
    elapsed = (time.perf_counter() - started) * 1000
    return logits.detach().cpu().numpy(), elapsed


def session_without_spinning(path, threads, ort):
    options = ort.SessionOptions()
    options.intra_op_num_threads = threads
    options.inter_op_num_threads = 1
    options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    options.log_severity_level = 3
    for scope in ("intra", "inter"):
        options.add_session_config_entry(f"session.{scope}_op.allow_spinning", "0")
    started = time.perf_counter()
    runtime = ort.InferenceSession(str(path), options, providers=["CPUExecutionProvider"])
    return runtime, (time.perf_counter() - started) * 1000


def run(args):
    torch.set_num_threads(args.threads)
    torch.set_num_interop_threads(1)
    helpers = runpy.run_path(str(Path(__file__).with_name("evaluate-graph-probe.py")))
    if args.no_ort_spinning:
        helpers["session"] = lambda path, threads: session_without_spinning(path, threads, helpers["ort"])
    global INPUTS
    INPUTS = helpers["INPUTS"]
    output = {
        "backend": args.backend,
        "torch_version": torch.__version__,
        "threads": args.threads,
        "ort_allow_spinning": not args.no_ort_spinning,
        "mps_available": torch.backends.mps.is_available(),
        "mps_fallback": os.environ.get("PYTORCH_ENABLE_MPS_FALLBACK"),
        "synthetic_only": True,
        "documents_or_core_verified": False,
        "cases": [],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)

    def save():
        output["peak_rss_bytes"] = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
        args.output.write_text(json.dumps(output, indent=2) + "\n")

    if args.backend in ("mps", "hybrid") and (
        not output["mps_available"] or output["mps_fallback"] != "0"
    ):
        raise RuntimeError("MPS must be available with explicit fallback=0")
    started = time.perf_counter()
    model, manifest = load_span_model(args.model, args.configs)
    output["weights"] = {key: value for key, value in manifest.items() if key != "mapping"}
    output["torch_load_ms"] = (time.perf_counter() - started) * 1000
    wrapper = SpanWrapper(model).eval()
    tokenizer = Tokenizer.from_file(str(args.model.parent / "tokenizer.json"))
    runtime, output["ort_load_ms"] = helpers["session"](args.model, args.threads)
    output["ort_version"] = helpers["ort"].__version__
    device = "mps" if args.backend == "mps" else "cpu"
    languages = helpers["SYNTHETIC_WORDS"]
    if args.language:
        languages = {args.language: languages[args.language]}
    save()
    try:
        if args.backend == "mps":
            wrapper.to("mps")
        elif args.backend == "hybrid":
            model.token_rep_layer = MpsEncoder(model.token_rep_layer)
        elif args.backend == "coreml":
            if not args.coreml_package:
                raise ValueError("CoreML requires a separately exported static encoder")
            started = time.perf_counter()
            model.token_rep_layer = CoremlEncoder(args.coreml_package, args.bucket_size, args.compute_units)
            output["coreml_load_ms"] = (time.perf_counter() - started) * 1000
            output["coreml_package"] = str(args.coreml_package)
            output["coreml_compute_units"] = args.compute_units
            output["coreml_anticipated_plan"] = model.token_rep_layer.plan_summary
            output["coreml_hardware_placement_verified"] = False
            output["coreml_bucket"] = args.bucket_size
            if args.head_onnx:
                wrapper = OnnxHeadWrapper(model.token_rep_layer, args.head_onnx, helpers, args.threads)
                output["head_backend"] = "original_onnx_tail"
                output["head_ort_load_ms"] = wrapper.load_ms
                output["head_model"] = str(args.head_onnx)
        for language, words in languages.items():
            for budget in args.budgets:
                feed = helpers["synthetic_feed"](tokenizer, words, budget)
                started = time.perf_counter()
                baseline = runtime.run(None, feed)[0]
                ort_first = (time.perf_counter() - started) * 1000
                gpu = args.backend in ("mps", "hybrid")
                candidate, first = timed_forward(wrapper, feed, device, gpu)
                case = {
                    "language": language,
                    "budget": budget,
                    "tokens": int(feed["input_ids"].shape[1]),
                    "words": int(feed["text_lengths"][0, 0]),
                    "ort_first_ms": ort_first,
                    "torch_first_ms": first,
                    **helpers["compare"](baseline, candidate, feed, args.threshold),
                }
                durations, ort_durations, parts = [], [], []
                for _ in range(args.runs):
                    _, elapsed = timed_forward(wrapper, feed, device, gpu)
                    durations.append(elapsed)
                    if args.backend in ("hybrid", "coreml"):
                        part = model.token_rep_layer.samples[-1].copy()
                        part["cpu_head_and_dispatch_ms"] = elapsed - sum(part.values())
                        parts.append(part)
                    started = time.perf_counter()
                    runtime.run(None, feed)
                    ort_durations.append((time.perf_counter() - started) * 1000)
                case.update(
                    warm_ms=durations,
                    warm_median_ms=statistics.median(durations),
                    ort_warm_ms=ort_durations,
                    ort_warm_median_ms=statistics.median(ort_durations),
                    boundary_samples=parts,
                )
                output["cases"].append(case)
                save()
                print(json.dumps(case), flush=True)
        output["completed"] = True
        output["threshold_spans_equal"] = all(c["threshold_spans_equal"] for c in output["cases"])
    except Exception as error:
        output["completed"] = False
        output["error_type"] = type(error).__name__
        output["error"] = str(error)
        save()
        raise
    save()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--configs", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--backend", choices=("cpu", "mps", "hybrid", "coreml"), required=True)
    parser.add_argument("--coreml-package", type=Path)
    parser.add_argument("--bucket-size", type=int, default=256)
    parser.add_argument("--compute-units", choices=("cpu-and-gpu", "all"), default="cpu-and-gpu")
    parser.add_argument("--head-onnx", type=Path)
    parser.add_argument("--no-ort-spinning", action="store_true")
    parser.add_argument("--language")
    parser.add_argument("--budgets", type=int, nargs="+", default=[96, 256])
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--threads", type=int, default=8)
    parser.add_argument("--threshold", type=float, default=0.4)
    run(parser.parse_args())


if __name__ == "__main__":
    main()
