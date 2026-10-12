#!/usr/bin/env python3
"""Export a MiniLM BIO probe with explicit training provenance and no quality claim."""

import argparse
import contextlib
import importlib.metadata
import io
import json
from pathlib import Path
import time
import traceback
import warnings

import coremltools as ct
import numpy as np
import torch

from student_probe_model import load_student, load_trained_student


def operation_summary(spec):
    counts, dependent_counts, dtypes = {}, {}, {}
    def visit(block, dependencies):
        for operation in block.operations:
            counts[operation.type] = counts.get(operation.type, 0) + 1
            dependent = any(binding.WhichOneof("binding") == "name" and binding.name in dependencies
                            for argument in operation.inputs.values() for binding in argument.arguments)
            if dependent:
                dependent_counts[operation.type] = dependent_counts.get(operation.type, 0) + 1
                dependencies.update(output.name for output in operation.outputs)
            for output in operation.outputs:
                if output.type.WhichOneof("type") == "tensorType":
                    dtype = ct.proto.MIL_pb2.DataType.Name(output.type.tensorType.dataType)
                    summary = dtypes.setdefault(operation.type, {})
                    summary[dtype] = summary.get(dtype, 0) + 1
            for child in operation.blocks:
                visit(child, set(dependencies))
    for function in spec.mlProgram.functions.values():
        for block in function.block_specializations.values():
            visit(block, {"input_ids", "attention_mask"})
    if dependent_counts.get("linear", 0) == 0 or dependent_counts.get("gather", 0) == 0:
        raise ValueError("Final MIL must retain input-dependent linear and embedding gather operations")
    return {"operator_counts": counts, "input_dependent_operator_counts": dependent_counts,
            "output_dtype_counts_by_operator": dtypes, "encoder_preserved_in_mil": True}


def safe_message(error):
    message = str(error)
    return "Tensor data omitted from error" if "tensor(" in message.lower() or "array(" in message.lower() else message[:2000]


def run(args):
    report = {
        "status": "running", "stage": "verify_and_load_local_model", "untrained": args.checkpoint is None,
        "anonymization_quality_verified": False, "production_candidate_accepted": False,
        "batch_size": args.batch_size, "sequence_length": args.sequence_length, "layer_count": args.layers,
        "output_shape": [args.batch_size, args.sequence_length, 11], "head_seed": args.head_seed,
        "precision": "float32", "compute_units": "CPU_AND_GPU", "minimum_deployment_target": "macOS15",
        "trace_input_dtype": "int64", "coreml_input_dtype": "int32", "tracer_check_enabled": False,
        "versions": {name: importlib.metadata.version(name) for name in ("torch", "transformers", "coremltools")},
        "warnings": [],
    }
    captured, recorded = io.StringIO(), []
    started = time.perf_counter()
    try:
        torch.set_num_threads(args.threads)
        torch.set_num_interop_threads(1)
        with warnings.catch_warnings(record=True) as recorded, contextlib.redirect_stdout(captured), contextlib.redirect_stderr(captured):
            warnings.simplefilter("always")
            if args.checkpoint is not None:
                model, manifest = load_trained_student(args.model_directory, args.checkpoint, args.layers, args.head_seed, args.manifest)
            else:
                model, manifest = load_student(args.model_directory, args.layers, args.head_seed, args.manifest)
            report["model"] = manifest
            generator = torch.Generator().manual_seed(17)
            shape = (args.batch_size, args.sequence_length)
            ids = torch.randint(0, model.encoder.config.vocab_size, shape, generator=generator, dtype=torch.int64)
            mask = torch.ones_like(ids)
            mask[:, -3:] = 0
            report["stage"] = "trace_static_student"
            with torch.inference_mode():
                traced = torch.jit.trace(model, (ids, mask), strict=True, check_trace=False)
            report["stage"] = "convert_mlprogram"
            converted = ct.convert(
                traced, convert_to="mlprogram",
                inputs=[ct.TensorType(name="input_ids", shape=shape, dtype=np.int32),
                        ct.TensorType(name="attention_mask", shape=shape, dtype=np.int32)],
                outputs=[ct.TensorType(name="token_logits", dtype=np.float32)],
                compute_precision=ct.precision.FLOAT32, minimum_deployment_target=ct.target.macOS15,
                compute_units=ct.ComputeUnit.CPU_AND_GPU, skip_model_load=True,
            )
            report["stage"] = "verify_encoder_retained_in_mil"
            report["mil"] = operation_summary(converted.get_spec())
            converted.user_defined_metadata.update({
                "copy-ai.student.model-id": manifest["model_id"], "copy-ai.student.revision": manifest["revision"],
                "copy-ai.student.source-sha256": manifest["model_source_sha256"],
                "copy-ai.student.layers": json.dumps(manifest["selected_layer_indices"]),
                "copy-ai.student.head-seed": str(args.head_seed),
                "copy-ai.student.head-untrained": "true" if manifest["head_untrained"] else "false",
                "copy-ai.student.quality-verified": "false", "copy-ai.precision": "fp32",
                "copy-ai.batch-size": str(args.batch_size), "copy-ai.sequence-length": str(args.sequence_length),
                "copy-ai.compute-units": "cpu-and-gpu",
            })
            if args.checkpoint is not None:
                converted.user_defined_metadata.update({"copy-ai.student.checkpoint-sha256": manifest["checkpoint_sha256"],
                                                        "copy-ai.student.training-report-sha256": manifest["training_report_sha256"],
                                                        "copy-ai.student.teacher-applied": "false"})
            report["stage"] = "save_student_probe"
            args.output.parent.mkdir(parents=True, exist_ok=True)
            converted.save(str(args.output))
            report["output_path"] = str(args.output.resolve())
            report["status"] = "trained_probe_exported_requires_quality_runtime_and_parity" if args.checkpoint else "untrained_latency_probe_exported_requires_runtime_and_parity"
            report["stage"] = "complete"
    except Exception as error:
        report["status"] = "failed"
        report["error"] = {"type": type(error).__name__, "message": safe_message(error)}
        report["error_frames"] = [{"file": f.filename, "line": f.lineno, "function": f.name}
                                  for f in traceback.extract_tb(error.__traceback__)[-8:]]
    finally:
        report["warnings"] = [{"category": warning.category.__name__, "message": safe_message(warning.message)} for warning in recorded]
        report["diagnostic_output_characters"] = len(captured.getvalue())
        report["elapsed_seconds"] = time.perf_counter() - started
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in ("status", "stage", "batch_size", "layer_count")}))
    return 1 if report["status"] == "failed" else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model-directory", type=Path, required=True)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--checkpoint", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--layers", type=int, choices=(6, 12), default=12)
    parser.add_argument("--batch-size", type=int, choices=(1, 8, 16, 32), default=1)
    parser.add_argument("--sequence-length", type=int, default=256)
    parser.add_argument("--head-seed", type=int, default=17)
    parser.add_argument("--threads", type=int, default=8)
    args = parser.parse_args()
    if not 4 <= args.sequence_length <= 512 or args.threads < 1:
        parser.error("A static 4..512 sequence length and positive thread count are required")
    if args.output.suffix != ".mlpackage" or args.output.exists():
        parser.error("--output must be a new isolated .mlpackage path")
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
