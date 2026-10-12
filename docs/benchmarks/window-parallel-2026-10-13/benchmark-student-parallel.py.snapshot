#!/usr/bin/env python3
"""Measure trained student windows through bounded, independently owned CoreML models."""

import argparse
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor, TimeoutError
import gc
import hashlib
import json
import math
import multiprocessing
import os
from pathlib import Path
import queue
import runpy
import statistics
import sys
import time

os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
os.environ["TOKENIZERS_PARALLELISM"] = "false"
os.environ["PYTORCH_ENABLE_MPS_FALLBACK"] = "0"

import numpy as np

from student_parallel_runtime import (
    MemoryGuard, abort_process_pools, initialize_worker, max_call_overlap, peak_rss_bytes,
    predict_batch, worker_info,
)
from student_parallel_text import prepare

BUCKET, OVERLAP = 256, 32
ONE_MIB = 1024 * 1024
REFERENCE_DIGEST = "6fcc5287370a931588fa28f6d9add36a12c6908a3a34b50355d788ce444587b1"


class Progress:
    def __init__(self, guard, pid_queue, workers, future_timeout_seconds):
        self.guard, self.pid_queue, self.workers = guard, pid_queue, workers
        self.future_timeout_seconds = future_timeout_seconds
        self.stage, self.last_at, self.last_batches = "worker_initialization", 0.0, 0
        self.input_bytes, self.run, self.completed_batches = 0, 0, 0

    def pulse(self, force=False):
        if self.pid_queue is not None:
            while True:
                try:
                    self.guard.pids.add(self.pid_queue.get_nowait())
                except queue.Empty:
                    break
        rss = self.guard.sample(force=force)
        now = time.perf_counter()
        if force or now - self.last_at >= 5 or self.completed_batches - self.last_batches >= 128:
            print(json.dumps({
                "stage": self.stage, "workers": self.workers, "input_bytes": self.input_bytes,
                "run": self.run, "completed_batches": self.completed_batches,
                "sampled_aggregate_rss_bytes": rss,
            }), flush=True)
            self.last_at, self.last_batches = now, self.completed_batches

    def wait(self, future):
        deadline = time.perf_counter() + self.future_timeout_seconds
        while True:
            self.pulse()
            remaining = deadline - time.perf_counter()
            if remaining <= 0:
                raise TimeoutError("Parallel worker result deadline exceeded")
            try:
                return future.result(timeout=min(1, remaining))
            except TimeoutError:
                if future.done():
                    return future.result()


def expected_package(metadata, batch):
    from student_probe_model import MODEL_ID, REVISION

    return {
        "copy-ai.student.model-id": MODEL_ID,
        "copy-ai.student.revision": REVISION,
        "copy-ai.student.source-sha256": metadata["model_source_sha256"],
        "copy-ai.student.layers": metadata["selected_layer_indices"],
        "copy-ai.student.head-seed": "17",
        "copy-ai.student.head-untrained": "false",
        "copy-ai.student.checkpoint-sha256": metadata["checkpoint_sha256"],
        "copy-ai.student.training-report-sha256": metadata["training_report_sha256"],
        "copy-ai.student.teacher-applied": "false",
        "copy-ai.precision": "fp32",
        "copy-ai.compute-units": "cpu-and-gpu",
        "copy-ai.batch-size": str(batch),
        "copy-ai.sequence-length": str(BUCKET),
    }


def measure(text, tokenizer, pools, progress, batch):
    started = time.perf_counter()
    progress.stage = "whole_text_tokenization"
    prepared = prepare(text, tokenizer, bucket=BUCKET, overlap=OVERLAP, batch=batch)
    labels = np.zeros(prepared.token_count, dtype=np.uint8)
    assigned = np.zeros(prepared.token_count, dtype=np.uint8)
    progress.pulse(force=True)
    encoded = time.perf_counter()
    intervals, prediction_ms, postprocess_ms, merge_ms = [], 0.0, 0.0, 0.0
    input_copy_bytes, input_copy_ms, copied_batches = 0, 0.0, 0
    pending = {}
    maximum_pending = 0

    def submit(index):
        nonlocal maximum_pending
        ids, mask, windows = prepared.batch(index, batch)
        future = pools[index % len(pools)].submit(predict_batch, index, ids, mask)
        # Queue pickling is asynchronous; retain each immutable input until completion.
        pending[index] = (future, windows, ids, mask)
        maximum_pending = max(maximum_pending, len(pending))

    progress.stage = "window_predictions"
    for index in range(min(len(pools), prepared.batch_count)):
        submit(index)
    for index in range(prepared.batch_count):
        future, windows, _, _ = pending[index]
        result = progress.wait(future)
        if result["index"] != index or result["choices"].dtype != np.uint8:
            raise ValueError("Parallel result index or compact label dtype differs")
        before = time.perf_counter()
        prepared.stitch(result["choices"], windows, labels, assigned)
        merge_ms += (time.perf_counter() - before) * 1000
        prediction_ms += result["predict_ms"]
        postprocess_ms += result["validate_argmax_ms"]
        input_copy_bytes += result["input_copy_bytes"]
        input_copy_ms += result["input_copy_ms"]
        copied_batches += result["input_copy_bytes"] > 0
        intervals.append((result["predict_start_ns"], result["predict_end_ns"]))
        progress.guard.register(result)
        del pending[index]
        if index + len(pools) < prepared.batch_count:
            submit(index + len(pools))
        progress.completed_batches = index + 1
        progress.pulse()
    inferred = time.perf_counter()
    progress.stage = "ordered_span_decode"
    evidence = prepared.finish(labels, assigned)
    finished = time.perf_counter()
    progress.pulse(force=True)
    return {
        **evidence,
        "encode_ms": (encoded - started) * 1000,
        "inference_wall_ms": (inferred - encoded) * 1000,
        "predict_call_sum_ms": prediction_ms,
        "validate_argmax_sum_ms": postprocess_ms,
        "worker_input_copy_bytes": input_copy_bytes,
        "worker_input_copy_sum_ms": input_copy_ms,
        "worker_input_copy_batches": copied_batches,
        "ordered_merge_ms": merge_ms,
        "decode_ms": (finished - inferred) * 1000,
        "wall_ms": (finished - started) * 1000,
        "predict_calls": len(intervals),
        "max_observed_predict_call_overlap": max_call_overlap(intervals),
        "overlap_scope": "host_predict_call_intervals_only",
        "gpu_execution_overlap_proven": False,
        "max_pending_batches": maximum_pending,
        "input_batch_bytes": batch * BUCKET * 4 * 2,
        "output_batch_bytes": batch * BUCKET,
        "window_reuse": 0,
        "all_output_shapes_finite": True,
        "output_dtype": "float32",
        "memory": progress.guard.report(),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model-dir", type=Path, required=True)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--sizes", type=int, nargs="+", default=[ONE_MIB])
    parser.add_argument("--workers", type=int, nargs="+", choices=(1, 2, 4), default=[1, 2, 4])
    parser.add_argument("--batch", type=int, choices=(8, 16, 32), default=8)
    parser.add_argument("--executor", choices=("process", "thread"), default="process")
    parser.add_argument("--runs", type=int, default=1)
    parser.add_argument("--memory-limit-gib", type=float, default=16)
    parser.add_argument("--future-timeout-seconds", type=float, default=300)
    parser.add_argument("--expected-1m-spans", type=int, default=20456)
    parser.add_argument("--expected-1m-digest", default=REFERENCE_DIGEST)
    args = parser.parse_args()
    if (sys.platform != "darwin" or args.runs < 1 or args.memory_limit_gib <= 0
            or not math.isfinite(args.memory_limit_gib)
            or args.future_timeout_seconds <= 0 or not math.isfinite(args.future_timeout_seconds)
            or not all(4096 <= size < 2**32 for size in args.sizes)
            or len(set(args.workers)) != len(args.workers) or 1 not in args.workers
            or args.expected_1m_spans < 1 or len(args.expected_1m_digest) != 64):
        parser.error("Requires macOS, positive runs/memory and unique workers including one")
    workers_order = [1, *(value for value in args.workers if value != 1)]
    report = {
        "completed": False, "probe": "trained_student_parallel_whole_text",
        "executor": args.executor, "process_start_method": "spawn" if args.executor == "process" else None,
        "workers": workers_order, "runs": args.runs, "batch": args.batch, "bucket": BUCKET,
        "overlap": OVERLAP, "precision": "fp32", "compute_units": "cpu-and-gpu",
        "quality_verified": False, "core_workflow_verified": False,
        "warmup_excluded_from_wall": True, "model_load_excluded_from_wall": True,
        "whole_text_preparation_repeated_per_run": True,
        "decoding_policy": "per_token_unicode_whitespace_trim_empty_skip",
        "span_digest_encoding": "ordered_struct_pack_little_endian_QQB_kind_0_to_4",
        "expected_1m_spans": args.expected_1m_spans,
        "expected_1m_digest": args.expected_1m_digest,
        "memory_limit_bytes": int(args.memory_limit_gib * 1024**3),
        "future_timeout_seconds": args.future_timeout_seconds,
        "process_failure_cleanup": "owned_process_terminate_then_kill_bounded_join",
        "running_thread_native_call_abortable": False,
        "preflight_kind": "estimated_host_memory_not_hard_allocation_cap",
        "preflight_formula": "input_bytes*40 + workers*(1.5GiB + batch*4MiB) + parent*2GiB",
        "tokenizer_transient_python_list_allocations_in_estimate_only": True,
        "experiments": [],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)

    def save():
        args.output.write_text(json.dumps(report, indent=2) + "\n")

    save()
    pools, pid_queue, progress, failed = [], None, None, False
    try:
        import torch
        from transformers import XLMRobertaTokenizerFast
        from student_probe_model import load_trained_student

        torch.set_num_threads(8)
        torch.set_num_interop_threads(1)
        started = time.perf_counter()
        model, metadata = load_trained_student(args.model_dir, args.checkpoint, layers=12, head_seed=17)
        report["parent_checkpoint_verification_ms"] = (time.perf_counter() - started) * 1000
        report["weights"] = metadata
        expected = expected_package(metadata, args.batch)
        del model
        gc.collect()
        report["parent_verification_peak_rss_bytes"] = peak_rss_bytes()
        tokenizer = XLMRobertaTokenizerFast(vocab_file=str(args.model_dir / "sentencepiece.bpe.model"))
        special_ids = (tokenizer.pad_token_id, tokenizer.cls_token_id, tokenizer.sep_token_id)
        generator = runpy.run_path(str(Path(__file__).with_name("benchmark-speed.py")))["varied"]
        baseline = {}
        for workers in workers_order:
            guard = MemoryGuard(report["memory_limit_bytes"])
            estimates = [size * 40 + workers * (1536 + args.batch * 4) * ONE_MIB
                         + 2048 * ONE_MIB for size in args.sizes]
            if max(estimates) > guard.limit_bytes:
                raise MemoryError("Estimated preflight host memory exceeds configured guard")
            context = multiprocessing.get_context("spawn")
            pid_queue = context.Queue() if args.executor == "process" else None
            progress = Progress(guard, pid_queue, workers, args.future_timeout_seconds)
            initializer = (str(args.package), expected, args.batch, BUCKET, special_ids, pid_queue)
            pools = [
                ProcessPoolExecutor(max_workers=1, mp_context=context, initializer=initialize_worker,
                                    initargs=initializer) if args.executor == "process" else
                ThreadPoolExecutor(max_workers=1, initializer=initialize_worker, initargs=initializer)
                for _ in range(workers)
            ]
            futures = [pool.submit(worker_info) for pool in pools]
            information = []
            for future in futures:
                info = progress.wait(future)
                guard.register(info)
                information.append(info)
            if args.executor == "process" and len({info["pid"] for info in information}) != workers:
                raise ValueError("Independent process workers require distinct PIDs")
            progress.pulse(force=True)
            group = {"workers": workers, "worker_initialization": information, "cases": []}
            report["experiments"].append(group)
            save()
            for size, estimate in zip(args.sizes, estimates):
                progress.input_bytes, progress.stage = size, "synthetic_input_generation"
                started = time.perf_counter()
                text = generator(size)
                raw = text.encode("utf-8")
                if len(raw) != size:
                    raise ValueError("Synthetic workload byte size differs")
                case = {"bytes": size, "input_sha256": hashlib.sha256(raw).hexdigest(),
                        "input_generation_ms": (time.perf_counter() - started) * 1000,
                        "estimated_preflight_bytes": estimate, "runs": []}
                del raw
                group["cases"].append(case)
                for run in range(args.runs):
                    progress.run, progress.completed_batches = run + 1, 0
                    result = measure(text, tokenizer, pools, progress, args.batch)
                    observed = (result["tokens"], result["decoded_span_count"], result["span_sha256"])
                    if size == ONE_MIB and observed[1:] != (args.expected_1m_spans, args.expected_1m_digest):
                        raise ValueError("Trained one-MiB spans differ from previous sequential reference")
                    if size not in baseline:
                        if workers != 1:
                            raise ValueError("Parallel comparison requires completed one-worker reference")
                        baseline[size] = observed
                    if observed != baseline[size]:
                        raise ValueError("Parallel ordered spans differ from completed one-worker reference")
                    result["matches_completed_one_worker_reference"] = True
                    result["matches_previous_1m_reference"] = size == ONE_MIB
                    case["runs"].append(result)
                    case["median_wall_ms"] = statistics.median(row["wall_ms"] for row in case["runs"])
                    save()
                print(json.dumps({"workers": workers, "bytes": size, "median_wall_ms": case["median_wall_ms"],
                                  "decoded_span_count": observed[1], "span_sha256": observed[2]}), flush=True)
                del text
                gc.collect()
            group["memory"] = guard.report()
            for pool in pools:
                pool.shutdown(wait=True, cancel_futures=True)
            pools = []
            if pid_queue is not None:
                pid_queue.close()
                pid_queue.join_thread()
                pid_queue = None
            save()
        report["completed"] = True
        save()
    except BaseException as error:
        failed = True
        report.update(completed=False, error_type=type(error).__name__,
                      failed_stage=progress.stage if progress is not None else "parent_checkpoint_verification")
        save()
        print(json.dumps({"completed": False, "error_type": type(error).__name__,
                          "failed_stage": report["failed_stage"]}), flush=True)
        raise SystemExit(130 if isinstance(error, KeyboardInterrupt) else 1) from None
    finally:
        if failed and pools:
            if args.executor == "process":
                report["failure_cleanup"] = abort_process_pools(pools)
            else:
                for pool in pools:
                    pool.shutdown(wait=False, cancel_futures=True)
                report["failure_cleanup"] = {"mode": "thread_running_native_call_not_abortable"}
            save()
        else:
            for pool in pools:
                pool.shutdown(wait=True, cancel_futures=True)
        if pid_queue is not None:
            pid_queue.close()
            pid_queue.join_thread()


if __name__ == "__main__":
    main()
