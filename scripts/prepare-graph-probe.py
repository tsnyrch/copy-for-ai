#!/usr/bin/env python3
"""Prepare isolated FP32 GLiNER graph experiments without running the model."""

import argparse
from collections import Counter
import gc
import hashlib
import json
from pathlib import Path
import re

import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper
from onnx.reference import ReferenceEvaluator


PREFIX = "/token_rep_layer/bert_layer/model/encoder/"
POSITION_OUTPUT = PREFIX + "Unsqueeze_6_output_0"
POSITION_BOUNDARY = "/token_rep_layer/bert_layer/model/embeddings/Mul_output_0"
PROJECTION = re.compile(r"(.*/encoder/layer\.\d+/attention/self/)(query|key|value)_proj/MatMul$")


def digest(path):
    checksum = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(8 * 1024 * 1024), b""):
            checksum.update(block)
    return checksum.hexdigest()


def summary(model):
    return {
        "nodes": len(model.graph.node),
        "initializers": len(model.graph.initializer),
        "operators": dict(sorted(Counter(node.op_type for node in model.graph.node).items())),
        "inputs": [value_info(value) for value in model.graph.input],
        "outputs": [value_info(value) for value in model.graph.output],
    }


def value_info(value):
    tensor = value.type.tensor_type
    return {
        "name": value.name,
        "type": TensorProto.DataType.Name(tensor.elem_type),
        "shape": [dimension.dim_value if dimension.HasField("dim_value") else dimension.dim_param
                  for dimension in tensor.shape.dim],
    }


def specialize_contract(model):
    fixed = {"batch_size": 1, "value": 1, "idx": 2}
    changed = 0
    for value in list(model.graph.input) + list(model.graph.output) + list(model.graph.value_info):
        for dimension in value.type.tensor_type.shape.dim:
            if dimension.dim_param in fixed:
                size = fixed[dimension.dim_param]
                dimension.ClearField("dim_param")
                dimension.dim_value = size
                changed += 1
    return {"fixed_symbols": fixed, "changed_dimensions": changed}


def ancestors(model, output, boundaries=()):
    by_output = {name: node for node in model.graph.node for name in node.output}
    required = set()
    pending = [output]
    while pending:
        name = pending.pop()
        if name in required or name in boundaries:
            continue
        required.add(name)
        node = by_output.get(name)
        if node is not None:
            pending.extend(node.input)
    return [node for node in model.graph.node if any(name in required for name in node.output)]


def position_branch(model):
    nodes = ancestors(model, POSITION_OUTPUT, (POSITION_BOUNDARY,))
    if not nodes or not any(node.op_type == "Log" for node in nodes):
        raise ValueError("Expected the original DeBERTa relative-position bucket branch")
    input_value = helper.make_tensor_value_info(POSITION_BOUNDARY, TensorProto.FLOAT, [1, "length", 768])
    output_value = helper.make_tensor_value_info(POSITION_OUTPUT, TensorProto.INT64, [1, "length", "length"])
    graph = helper.make_graph(nodes, "original-position-buckets", [input_value], [output_value])
    branch = helper.make_model(graph, opset_imports=list(model.opset_import))
    branch.ir_version = model.ir_version
    return branch


def replace_position_buckets(model, maximum, destination):
    branch = position_branch(model)
    onnx.save_model(branch, destination / "original-position-branch.onnx")
    # Only positional integer indices are evaluated; no token embedding or model inference occurs.
    positions = ReferenceEvaluator(branch).run(
        None, {POSITION_BOUNDARY: np.zeros((1, maximum, 768), dtype=np.float32)}
    )[0]
    if positions.dtype != np.int64 or positions.shape != (1, maximum, maximum):
        raise ValueError("Unexpected position-bucket table shape or dtype")
    names = {node.name for node in model.graph.node}
    if "probe/position/Slice" in names:
        raise ValueError("Model already contains experiment nodes")
    initializers = [
        numpy_helper.from_array(positions, "probe/position/table"),
        numpy_helper.from_array(np.array(1, dtype=np.int64), "probe/position/sequence_axis"),
        numpy_helper.from_array(np.array([0], dtype=np.int64), "probe/position/unsqueeze_axis"),
        numpy_helper.from_array(np.array([0, 0], dtype=np.int64), "probe/position/starts"),
        numpy_helper.from_array(np.array([1, 2], dtype=np.int64), "probe/position/axes"),
    ]
    nodes = [
        helper.make_node("Shape", ["input_ids"], ["probe/position/input_shape"], name="probe/position/Shape"),
        helper.make_node("Gather", ["probe/position/input_shape", "probe/position/sequence_axis"],
                         ["probe/position/length"], axis=0, name="probe/position/Gather"),
        helper.make_node("Unsqueeze", ["probe/position/length", "probe/position/unsqueeze_axis"],
                         ["probe/position/end"], name="probe/position/Unsqueeze"),
        helper.make_node("Concat", ["probe/position/end", "probe/position/end"],
                         ["probe/position/ends"], axis=0, name="probe/position/Concat"),
        helper.make_node("Slice", ["probe/position/table", "probe/position/starts", "probe/position/ends",
                                   "probe/position/axes"], [POSITION_OUTPUT], name="probe/position/Slice"),
    ]
    rewritten = []
    for node in model.graph.node:
        if POSITION_OUTPUT in node.output:
            rewritten.extend(nodes)
        else:
            rewritten.append(node)
    del model.graph.node[:]
    model.graph.node.extend(rewritten)
    model.graph.initializer.extend(initializers)
    return {"maximum_sequence_length": maximum, "table_shape": list(positions.shape),
            "table_sha256": hashlib.sha256(positions.tobytes()).hexdigest(),
            "evaluation": "Original ONNX position-only branch, reference evaluator; model not run"}


def fuse_qkv(model):
    weights = {initializer.name: initializer for initializer in model.graph.initializer}
    groups = {}
    for node in model.graph.node:
        match = PROJECTION.fullmatch(node.name)
        if match:
            groups.setdefault(match[1], {})[match[2]] = node
    if len(groups) != 12 or any(set(group) != {"query", "key", "value"} for group in groups.values()):
        raise ValueError("Expected exactly twelve complete Q/K/V projection groups")
    replacements = {}
    removed = set()
    for index, group in enumerate(groups.values()):
        projections = [group[name] for name in ("query", "key", "value")]
        if len({node.input[0] for node in projections}) != 1:
            raise ValueError("Q/K/V projections do not have the same activation input")
        matrices = [numpy_helper.to_array(weights[node.input[1]]) for node in projections]
        if any(matrix.dtype != np.float32 or matrix.shape != (768, 768) for matrix in matrices):
            raise ValueError("Unexpected Q/K/V weight shape or precision")
        weight_name = f"probe/qkv/{index}/weight"
        split_name = f"probe/qkv/{index}/split_sizes"
        projection_name = f"probe/qkv/{index}/projected"
        model.graph.initializer.extend([
            numpy_helper.from_array(np.concatenate(matrices, axis=1), weight_name),
            numpy_helper.from_array(np.array([768, 768, 768], dtype=np.int64), split_name),
        ])
        replacements[projections[0].name] = [
            helper.make_node("MatMul", [projections[0].input[0], weight_name], [projection_name],
                             name=f"probe/qkv/{index}/MatMul"),
            helper.make_node("Split", [projection_name, split_name],
                             [node.output[0] for node in projections], axis=-1,
                             name=f"probe/qkv/{index}/Split"),
        ]
        removed.update(node.name for node in projections)
    rewritten = []
    for node in model.graph.node:
        if node.name in replacements:
            rewritten.extend(replacements[node.name])
        elif node.name not in removed:
            rewritten.append(node)
    del model.graph.node[:]
    model.graph.node.extend(rewritten)
    return {"layers": len(groups), "precision": "FP32", "weight_operation": "Exact concatenate only",
            "removed_matmul_count": 36, "added_matmul_count": 12, "added_split_count": 12,
            "risk": "MatMul blocking may change floating-point last bits; evaluate logits and decoded spans"}


def prune(model):
    live = set()
    for output in model.graph.output:
        for node in ancestors(model, output.name):
            live.add(node.name)
    kept = [node for node in model.graph.node if node.name in live]
    removed = len(model.graph.node) - len(kept)
    del model.graph.node[:]
    model.graph.node.extend(kept)
    required = {name for node in kept for name in node.input} | {value.name for value in model.graph.output}
    initializers = [value for value in model.graph.initializer if value.name in required]
    del model.graph.initializer[:]
    model.graph.initializer.extend(initializers)
    return removed


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--maximum-sequence-length", type=int, default=512)
    parser.add_argument("--candidate", choices=("A", "B", "both"), default="both")
    args = parser.parse_args()
    source = args.model.resolve()
    output = args.output.resolve()
    if source.parent == output or source.parent in output.parents:
        raise ValueError("Experiments must be written outside the verified model directory")
    if not 1 <= args.maximum_sequence_length <= 512:
        raise ValueError("Experiment maximum must be within 1..512")
    output.mkdir(parents=True, exist_ok=True)
    source_sha256 = digest(source)
    candidates = ("A", "B") if args.candidate == "both" else (args.candidate,)
    for candidate in candidates:
        destination = output / candidate
        destination.mkdir(exist_ok=False)
        model = onnx.load_model(source)
        original = summary(model)
        contract = specialize_contract(model)
        positions = replace_position_buckets(model, args.maximum_sequence_length, destination)
        qkv = fuse_qkv(model) if candidate == "B" else None
        pruned_nodes = prune(model)
        final = summary(model)
        model_path = destination / "model.onnx"
        onnx.save_model(model, model_path, save_as_external_data=True, all_tensors_to_one_file=True,
                        location="weights.bin", size_threshold=1024, convert_attribute=False)
        del model
        gc.collect()
        onnx.checker.check_model(str(model_path))
        manifest = {
            "schema": 1, "candidate": candidate, "source": str(source), "source_sha256": source_sha256,
            "model_sha256": digest(model_path), "weights_sha256": digest(destination / "weights.bin"),
            "source_graph": original, "candidate_graph": final, "contract": contract,
            "positions": positions, "qkv": qkv, "pruned_nodes": pruned_nodes,
            "weights": "Original FP32 weights, no quantization or retraining",
            "validation": {"onnx_checker": "passed", "full_model_inference": "not run"},
            "limit": "Experiment only: batch=1; reject any input sequence exceeding table maximum",
        }
        (destination / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        print(json.dumps({"candidate": candidate, "model": str(model_path),
                          "source_nodes": original["nodes"], "candidate_nodes": final["nodes"],
                          "source_matmul": original["operators"].get("MatMul", 0),
                          "candidate_matmul": final["operators"].get("MatMul", 0)}))


if __name__ == "__main__":
    main()
