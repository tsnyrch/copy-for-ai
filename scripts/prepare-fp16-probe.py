#!/usr/bin/env python3
"""Convert an isolated multilingual GLiNER experiment to FP16, keeping IO types."""

import argparse
from collections import Counter
import gc
import hashlib
import json
from pathlib import Path

import onnx
from onnx import TensorProto
from onnxruntime.transformers.float16 import DEFAULT_OP_BLOCK_LIST, convert_float_to_float16


def digest(path):
    checksum = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(8 * 1024 * 1024), b""):
            checksum.update(block)
    return checksum.hexdigest()


def graph_info(model):
    return {"nodes": len(model.graph.node), "initializers": len(model.graph.initializer),
            "operators": dict(sorted(Counter(node.op_type for node in model.graph.node).items())),
            "initializer_types": dict(sorted(Counter(TensorProto.DataType.Name(value.data_type)
                                                      for value in model.graph.initializer).items())),
            "inputs": {value.name: TensorProto.DataType.Name(value.type.tensor_type.elem_type)
                       for value in model.graph.input},
            "outputs": {value.name: TensorProto.DataType.Name(value.type.tensor_type.elem_type)
                        for value in model.graph.output}}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    source = args.model.resolve()
    destination = args.output.resolve()
    if source.parent == destination or source.parent in destination.parents:
        raise ValueError("FP16 experiment must be outside the verified model directory")
    destination.mkdir(parents=True, exist_ok=False)
    source_hash = digest(source)
    model = onnx.load_model(source)
    original = graph_info(model)
    converted = convert_float_to_float16(model, keep_io_types=True)
    final = graph_info(converted)
    if original["inputs"] != final["inputs"] or original["outputs"] != final["outputs"]:
        raise ValueError("FP16 conversion unexpectedly changed model IO types")
    path = destination / "model.onnx"
    onnx.save_model(converted, path, save_as_external_data=True, all_tensors_to_one_file=True,
                    location="weights.bin", size_threshold=1024, convert_attribute=False)
    del model, converted
    gc.collect()
    onnx.checker.check_model(str(path))
    manifest = {
        "schema": 1, "candidate": "C-FP16", "source": str(source), "source_sha256": source_hash,
        "model_sha256": digest(path), "weights_sha256": digest(destination / "weights.bin"),
        "source_graph": original, "candidate_graph": final,
        "converter": "onnxruntime.transformers.float16.convert_float_to_float16",
        "keep_io_types": True, "blocked_operators": DEFAULT_OP_BLOCK_LIST,
        "min_positive_value": 5.96e-8, "max_finite_value": 65504.0,
        "validation": {"onnx_checker": "passed", "cpu_session": "not tested", "inference": "not run"},
        "risk": "Lossy FP16 arithmetic and weights; require runtime kernel smoke, threshold and gold gates",
    }
    (destination / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps({"model": str(path), "nodes": final["nodes"],
                      "initializer_types": final["initializer_types"]}))


if __name__ == "__main__":
    main()
