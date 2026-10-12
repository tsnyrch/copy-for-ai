"""Persistent, serially owned CoreML workers and sampled host memory accounting."""

import json
import os
import resource
import subprocess
import sys
import threading
import time

import numpy as np

_LOCAL = threading.local()


def peak_rss_bytes():
    value = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    return int(value if sys.platform == "darwin" else value * 1024)


def _validate_output(output, batch, bucket):
    if (output.shape != (batch, bucket, 11) or output.dtype != np.float32
            or not np.isfinite(output).all()):
        raise ValueError("Parallel student output shape or finite-value contract failed")


def initialize_worker(package_path, expected_metadata, batch, bucket, special_ids, pid_queue=None):
    if pid_queue is not None:
        pid_queue.put(os.getpid())
    import coremltools as ct

    started = time.perf_counter()
    runtime = ct.models.MLModel(package_path, compute_units=ct.ComputeUnit.CPU_AND_GPU)
    metadata = dict(runtime.user_defined_metadata)
    for key, expected in expected_metadata.items():
        actual = metadata.get(key)
        if key == "copy-ai.student.layers":
            actual = json.loads(actual or "[]")
        if actual != expected:
            raise ValueError("Parallel CoreML package metadata verification failed")
    spec = runtime.get_spec().description
    inputs = {entry.name: list(entry.type.multiArrayType.shape) for entry in spec.input}
    outputs = {entry.name: list(entry.type.multiArrayType.shape) for entry in spec.output}
    if inputs != {"input_ids": [batch, bucket], "attention_mask": [batch, bucket]}:
        raise ValueError("Parallel CoreML input names or shapes differ")
    if outputs != {"token_logits": [batch, bucket, 11]}:
        raise ValueError("Parallel CoreML output name or shape differs")
    types = ct.proto.FeatureTypes_pb2.ArrayFeatureType
    if (any(entry.type.multiArrayType.dataType != types.INT32 for entry in spec.input)
            or any(entry.type.multiArrayType.dataType != types.FLOAT32 for entry in spec.output)):
        raise ValueError("Parallel CoreML input or output dtype differs")
    loaded = time.perf_counter()
    pad, cls, sep = special_ids
    ids = np.full((batch, bucket), pad, dtype=np.int32)
    ids[:, :2] = [cls, sep]
    mask = np.zeros((batch, bucket), dtype=np.int32)
    mask[:, :2] = 1
    output = runtime.predict({"input_ids": ids, "attention_mask": mask})["token_logits"]
    _validate_output(output, batch, bucket)
    warmed = time.perf_counter()
    _LOCAL.runtime = runtime
    _LOCAL.batch = batch
    _LOCAL.bucket = bucket
    _LOCAL.info = {
        "pid": os.getpid(),
        "coreml_load_compile_ms": (loaded - started) * 1000,
        "warmup_ms": (warmed - loaded) * 1000,
        "warmup_predict_calls": 1,
        "rss_peak_bytes": peak_rss_bytes(),
        "independent_serial_model": True,
        "output_shape": [batch, bucket, 11],
    }


def worker_info():
    return dict(_LOCAL.info)


def predict_batch(index, ids, mask):
    expected_shape = (_LOCAL.batch, _LOCAL.bucket)
    if (ids.shape != expected_shape or mask.shape != expected_shape
            or ids.dtype != np.int32 or mask.dtype != np.int32
            or not ids.flags.c_contiguous or not mask.flags.c_contiguous):
        raise ValueError("Parallel worker batch contract failed")
    copy_bytes, copy_ms = 0, 0.0
    if not ids.flags.writeable or not mask.flags.writeable:
        before_copy = time.perf_counter_ns()
        # The CoreML Python bridge requires writable buffers; parent inputs stay immutable.
        if not ids.flags.writeable:
            ids = ids.copy(order="C")
            copy_bytes += ids.nbytes
        if not mask.flags.writeable:
            mask = mask.copy(order="C")
            copy_bytes += mask.nbytes
        copy_ms = (time.perf_counter_ns() - before_copy) / 1_000_000
    started = time.perf_counter_ns()
    output = _LOCAL.runtime.predict({"input_ids": ids, "attention_mask": mask})["token_logits"]
    predicted = time.perf_counter_ns()
    _validate_output(output, *expected_shape)
    choices = output.argmax(axis=-1).astype(np.uint8)
    finished = time.perf_counter_ns()
    return {
        "index": index,
        "choices": choices,
        "predict_start_ns": started,
        "predict_end_ns": predicted,
        "predict_ms": (predicted - started) / 1_000_000,
        "validate_argmax_ms": (finished - predicted) / 1_000_000,
        "input_copy_bytes": copy_bytes,
        "input_copy_ms": copy_ms,
        "pid": os.getpid(),
        "rss_peak_bytes": peak_rss_bytes(),
    }


class MemoryGuard:
    def __init__(self, limit_bytes):
        self.limit_bytes = limit_bytes
        self.pids = {os.getpid()}
        self.sampled_aggregate_peak_bytes = 0
        self.last_sample_bytes = 0
        self.samples = 0
        self.last_sample_at = 0.0
        self.worker_peaks = {}

    def register(self, information):
        pid = information["pid"]
        self.pids.add(pid)
        self.worker_peaks[pid] = max(
            self.worker_peaks.get(pid, 0), information["rss_peak_bytes"])

    def sample(self, force=False):
        now = time.perf_counter()
        if not force and now - self.last_sample_at < 1:
            return self.last_sample_bytes
        result = subprocess.run(
            ["ps", "-o", "pid=,rss=", "-p", ",".join(str(pid) for pid in sorted(self.pids))],
            check=True, capture_output=True, text=True)
        rows = [line.split() for line in result.stdout.splitlines() if line.strip()]
        observed = {int(pid): int(rss) * 1024 for pid, rss in rows}
        if set(observed) != self.pids:
            raise RuntimeError("Sampled RSS guard could not observe every live worker")
        total = sum(observed.values())
        self.last_sample_bytes = total
        self.sampled_aggregate_peak_bytes = max(self.sampled_aggregate_peak_bytes, total)
        self.last_sample_at = now
        self.samples += 1
        if total > self.limit_bytes:
            raise MemoryError("Sampled parent and worker RSS exceeds configured guard")
        return total

    def report(self):
        peaks = dict(self.worker_peaks)
        peaks[os.getpid()] = max(peaks.get(os.getpid(), 0), peak_rss_bytes())
        return {
            "limit_bytes": self.limit_bytes,
            "guard_kind": "sampled_parent_plus_worker_host_rss",
            "hard_allocation_cap": False,
            "full_gpu_memory_measured": False,
            "sample_count": self.samples,
            "sampled_aggregate_peak_bytes": self.sampled_aggregate_peak_bytes,
            "sum_individual_process_peak_rss_bytes": sum(peaks.values()),
        }


def max_call_overlap(intervals):
    events = [(start, 1) for start, _ in intervals]
    events.extend((end, -1) for _, end in intervals)
    active, maximum = 0, 0
    for _, change in sorted(events):
        active += change
        maximum = max(maximum, active)
    if active != 0:
        raise ValueError("Unbalanced predict-call intervals")
    return maximum


def abort_process_pools(pools, terminate_seconds=3, kill_seconds=2):
    # Python 3.13 lacks terminate_workers; only handles owned by these executors qualify.
    children = [child for pool in pools for child in (pool._processes or {}).values()]
    terminated, killed = 0, 0
    for child in children:
        if child.is_alive():
            child.terminate()
            terminated += 1
    for pool in pools:
        pool.shutdown(wait=False, cancel_futures=True)
    deadline = time.perf_counter() + terminate_seconds
    for child in children:
        child.join(timeout=max(0, deadline - time.perf_counter()))
    for child in children:
        if child.is_alive():
            child.kill()
            killed += 1
    deadline = time.perf_counter() + kill_seconds
    for child in children:
        child.join(timeout=max(0, deadline - time.perf_counter()))
    return {
        "mode": "owned_process_terminate_then_kill",
        "owned_process_count": len(children),
        "terminate_signals": terminated,
        "kill_signals": killed,
        "remaining_live_processes": sum(child.is_alive() for child in children),
        "join_budget_seconds": terminate_seconds + kill_seconds,
    }
