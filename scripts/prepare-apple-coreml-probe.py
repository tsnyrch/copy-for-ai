#!/usr/bin/env python3
"""Export the pinned GLiNER encoder as an isolated static Core ML probe."""

from __future__ import annotations

import argparse
import contextlib
import importlib.metadata
import io
import json
import os
from pathlib import Path
import time
import traceback
import warnings

os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
os.environ["PYTORCH_ENABLE_MPS_FALLBACK"] = "0"

import numpy as np
import torch

from apple_probe_weights import load_span_model


class EncoderWrapper(torch.nn.Module):
    def __init__(self, encoder: torch.nn.Module):
        super().__init__()
        self.encoder = encoder

    def forward(self, input_ids, attention_mask):
        return self.encoder(input_ids, attention_mask)


def safe_message(error: BaseException) -> str:
    message = str(error)
    if "tensor(" in message.lower() or "array(" in message.lower():
        return "Error text contained tensor data and was omitted."
    return message[:2000]


def install_sqrt_promotion(report) -> None:
    from coremltools.converters.mil import Builder as mb
    from coremltools.converters.mil.frontend.torch import register_torch_op
    from coremltools.converters.mil.frontend.torch.ops import _get_inputs
    from coremltools.converters.mil.mil import types

    if torch.get_default_dtype() != torch.float32:
        raise ValueError("Integer sqrt promotion requires PyTorch's default float32 dtype")
    record = {
        "operator": "aten::sqrt",
        "promotion": "integer_or_bool_to_fp32; floating_inputs_unchanged",
        "reason": "CoreML sqrt rejects integer inputs; PyTorch promotes them to its default floating dtype",
        "coremltools_version": report["versions"]["coremltools"],
        "promoted_nodes": [],
    }
    report["converter_shims"] = [record]

    @register_torch_op(torch_alias=["sqrt"], override=True)
    def sqrt_with_integer_promotion(context, node):
        value = _get_inputs(context, node, expected=1)[0]
        if types.is_int(value.dtype) or value.dtype is types.bool:
            record["promoted_nodes"].append({"name": node.name, "input_dtype": types.builtin_to_string(value.dtype)})
            value = mb.cast(x=value, dtype="fp32", name=f"{node.name}_integer_to_fp32")
        context.add(mb.sqrt(x=value, name=node.name))


@contextlib.contextmanager
def static_relative_positions(report, sequence_length):
    from transformers.models.deberta_v2 import modeling_deberta_v2 as deberta
    original = deberta.build_rpos
    record = {
        "function": "transformers.models.deberta_v2.modeling_deberta_v2.build_rpos",
        "assumption": "self_attention_query_and_key_lengths_equal_static_bucket",
        "sequence_length": sequence_length,
        "operation": "return_original_relative_pos; no_tensor_value_specialization",
        "calls": 0,
    }
    report["shape_specializations"] = [record]
    def build_rpos(query_layer, key_layer, relative_pos, position_buckets, max_relative_positions):
        if query_layer.size(-2) != sequence_length or key_layer.size(-2) != sequence_length:
            raise ValueError("Static self-attention query/key lengths differ from the export bucket")
        record["calls"] += 1
        return relative_pos
    deberta.build_rpos = build_rpos
    try:
        yield
    finally:
        deberta.build_rpos = original


def mil_dtype_summary(spec):
    from coremltools.proto import MIL_pb2
    operators, dtypes = {}, {}
    def visit(block):
        for operation in block.operations:
            operators[operation.type] = operators.get(operation.type, 0) + 1
            counts = dtypes.setdefault(operation.type, {})
            for output in operation.outputs:
                if output.type.WhichOneof("type") == "tensorType":
                    dtype = MIL_pb2.DataType.Name(output.type.tensorType.dataType)
                    counts[dtype] = counts.get(dtype, 0) + 1
            for child in operation.blocks:
                visit(child)
    for function in spec.mlProgram.functions.values():
        for block in function.block_specializations.values():
            visit(block)
    return {"operator_counts": operators, "output_dtype_counts_by_operator": dtypes}


def run(args) -> int:
    report = {
        "status": "running",
        "stage": "load_weights",
        "sequence_length": args.sequence_length, "batch_size": args.batch_size,
        "input_shapes": {"input_ids": [args.batch_size, args.sequence_length], "attention_mask": [args.batch_size, args.sequence_length]},
        "trace_input_dtype": "int64", "coreml_input_dtype": "int32",
        "output_shape": [args.batch_size, args.sequence_length, 512],
        "precision": {"fp32": "float32", "fp16": "float16", "mixed-linear": "mixed-linear"}[args.precision],
        "minimum_deployment_target": "macOS15",
        "compute_units": {"cpu-and-gpu": "CPU_AND_GPU", "all": "ALL"}[args.compute_units],
        "offline": True,
        "weights_loaded_from_pinned_onnx": True,
        "tracer_check_enabled": False,
        "parity_verified": False, "runtime_compilation_verified": False,
        "versions": {name: importlib.metadata.version(name) for name in ("torch", "gliner", "transformers", "coremltools")},
        "warnings": [],
    }
    captured = io.StringIO()
    started = time.perf_counter()
    recorded_warnings = []
    try:
        torch.set_num_threads(args.threads)
        torch.set_num_interop_threads(1)
        with warnings.catch_warnings(record=True) as recorded_warnings, contextlib.redirect_stdout(captured), contextlib.redirect_stderr(captured):
            warnings.simplefilter("always")
            import coremltools as ct

            install_sqrt_promotion(report)
            model, manifest = load_span_model(args.model, args.configs)
            report["weights"] = {key: value for key, value in manifest.items() if key != "mapping"}
            encoder = EncoderWrapper(model.token_rep_layer).eval()
            vocabulary_size = encoder.encoder.bert_layer.model.config.vocab_size
            generator = torch.Generator().manual_seed(17)
            ids = torch.randint(0, vocabulary_size, (args.batch_size, args.sequence_length), generator=generator, dtype=torch.int64)
            mask = torch.ones_like(ids)
            mask[:, -3:] = 0
            report["stage"] = "trace_static_encoder"
            with torch.inference_mode(), static_relative_positions(report, args.sequence_length):
                traced = torch.jit.trace(encoder, (ids, mask), strict=True, check_trace=False)
            report["stage"] = "convert_mlprogram"
            precision = ct.precision.FLOAT32 if args.precision == "fp32" else ct.precision.FLOAT16
            pipeline = ct.PassPipeline.DEFAULT
            if args.precision == "mixed-linear":
                precision = ct.transform.FP16ComputePrecision(op_selector=lambda op: op.op_type == "linear")
                pipeline.remove_passes(["common::add_int16_cast"])
                report["precision_policy"] = {"fp16_selector": "op.op_type == 'linear'",
                                              "removed_passes": ["common::add_int16_cast"], "other_ops": "float32"}
            converted = ct.convert(
                traced,
                convert_to="mlprogram",
                inputs=[
                    ct.TensorType(name="input_ids", shape=(args.batch_size, args.sequence_length), dtype=np.int32),
                    ct.TensorType(name="attention_mask", shape=(args.batch_size, args.sequence_length), dtype=np.int32),
                ],
                outputs=[ct.TensorType(name="token_embeddings", dtype=np.float32)],
                compute_precision=precision,
                pass_pipeline=pipeline,
                minimum_deployment_target=ct.target.macOS15,
                compute_units=ct.ComputeUnit.CPU_AND_GPU if args.compute_units == "cpu-and-gpu" else ct.ComputeUnit.ALL,
                skip_model_load=True,
            )
            report["stage"] = "save_mlpackage"
            report["mil_dtype_summary"] = mil_dtype_summary(converted.get_spec())
            converted.user_defined_metadata.update({"copy-ai.precision": args.precision, "copy-ai.compute-units": args.compute_units,
                                                   "copy-ai.batch-size": str(args.batch_size), "copy-ai.sequence-length": str(args.sequence_length)})
            args.output.parent.mkdir(parents=True, exist_ok=True)
            converted.save(str(args.output))
            report["output_path"] = str(args.output.resolve())
            report["status"] = "exported_requires_parity_and_runtime_checks"
            report["stage"] = "complete"
    except Exception as error:
        report["status"] = "failed"
        report["error"] = {
            "type": type(error).__name__,
            "message": safe_message(error),
            "frames": [{"file": f.filename, "line": f.lineno, "function": f.name}
                       for f in traceback.extract_tb(error.__traceback__)[-8:]],
        }
    finally:
        report["warnings"] = [
            {"category": warning.category.__name__, "message": safe_message(warning.message)}
            for warning in recorded_warnings
        ]
        report["diagnostic_output_characters"] = len(captured.getvalue())
        report["elapsed_seconds"] = time.perf_counter() - started
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in ("status", "stage", "sequence_length", "elapsed_seconds")}))
    return 1 if report["status"] == "failed" else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--configs", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--sequence-length", type=int, default=256)
    parser.add_argument("--batch-size", type=int, choices=(1, 4, 8), default=1)
    parser.add_argument("--threads", type=int, default=8)
    parser.add_argument("--precision", choices=("fp32", "fp16", "mixed-linear"), default="fp32")
    parser.add_argument("--compute-units", choices=("cpu-and-gpu", "all"), default="cpu-and-gpu")
    args = parser.parse_args()
    if args.sequence_length < 4 or args.sequence_length > 512:
        parser.error("--sequence-length must be between 4 and 512")
    if args.threads < 1:
        parser.error("--threads must be positive")
    if args.precision == "mixed-linear" and args.compute_units != "cpu-and-gpu":
        parser.error("--precision mixed-linear requires explicit CPU_AND_GPU execution")
    if args.output.suffix != ".mlpackage":
        parser.error("--output must use the .mlpackage suffix")
    if args.output.exists():
        parser.error("--output already exists; use a new isolated output path")
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
