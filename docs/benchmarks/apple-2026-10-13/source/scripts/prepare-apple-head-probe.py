#!/usr/bin/env python3
"""Extract the unchanged pinned ONNX head for an isolated Apple encoder probe."""

import argparse
from collections import Counter
import gc
import json
from pathlib import Path
import tempfile

import onnx
from onnx import TensorProto, helper
from onnx.utils import Extractor

from apple_probe_weights import ONNX_SHA256, file_sha256


ORIGINAL_INPUTS = ("input_ids", "words_mask", "text_lengths", "span_idx", "span_mask")
PROJECTION_NODE = "/token_rep_layer/projection/Add"


def describe(value):
    tensor = value.type.tensor_type
    return {
        "name": value.name,
        "dtype": TensorProto.DataType.Name(tensor.elem_type),
        "shape": [dimension.dim_param or dimension.dim_value for dimension in tensor.shape.dim],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--dynamic-batch", action="store_true")
    args = parser.parse_args()
    source, destination = args.model.resolve(), args.output.resolve()
    temporary_roots = (Path("/tmp").resolve(), Path(tempfile.gettempdir()).resolve())
    if not any(destination.is_relative_to(root) for root in temporary_roots):
        raise ValueError("Head experiment output must be a temporary directory")
    if destination == source.parent or destination.is_relative_to(source.parent):
        raise ValueError("Head experiment must be outside the verified model directory")
    source_hash = file_sha256(source)
    if source_hash != ONNX_SHA256:
        raise ValueError("Head experiment requires the pinned original ONNX model")
    if destination.exists():
        raise ValueError("Head experiment requires a fresh output directory")
    model = onnx.load_model(source, load_external_data=False)
    if any(tensor.data_location == TensorProto.EXTERNAL for tensor in model.graph.initializer):
        raise ValueError("Head experiment does not accept external source weights")
    projections = [
        node for node in model.graph.node
        if node.name == PROJECTION_NODE and node.op_type == "Add"
    ]
    if len(projections) != 1 or len(projections[0].output) != 1:
        raise ValueError("Expected exactly one original token projection Add output")
    boundary = projections[0].output[0]
    inputs = {value.name: value for value in model.graph.input}
    if not set((*ORIGINAL_INPUTS, "attention_mask")) <= inputs.keys():
        raise ValueError("Unexpected original ONNX input contract")
    if not model.graph.output or model.graph.output[0].name != "logits":
        raise ValueError("Unexpected original ONNX output contract")
    logits_output = model.graph.output[0].name
    sequence = inputs["input_ids"].type.tensor_type.shape.dim[1].dim_param or "sequence_length"
    retained_info = [value for value in model.graph.value_info if value.name != boundary]
    del model.graph.value_info[:]
    model.graph.value_info.extend(retained_info)
    batch = inputs["input_ids"].type.tensor_type.shape.dim[0].dim_param or "batch_size"
    model.graph.value_info.append(helper.make_tensor_value_info(
        boundary, TensorProto.FLOAT, [batch if args.dynamic_batch else 1, sequence, 512]
    ))
    head = Extractor(model).extract_model(
        [boundary, *ORIGINAL_INPUTS, "attention_mask"], [logits_output]
    )
    used = {name for node in head.graph.node for name in node.input}
    if "attention_mask" not in used:
        retained_inputs = [value for value in head.graph.input if value.name != "attention_mask"]
        del head.graph.input[:]
        head.graph.input.extend(retained_inputs)
    if boundary not in used or any(
        node.name.startswith("/token_rep_layer/bert_layer/") and node.op_type != "Constant"
        for node in head.graph.node
    ):
        raise ValueError("Extraction did not isolate the original projection boundary")
    destination.mkdir(parents=True)
    path = destination / "model.onnx"
    onnx.save_model(
        head, path, save_as_external_data=True, all_tensors_to_one_file=True,
        location="weights.bin", size_threshold=1024, convert_attribute=False,
    )
    manifest = {
        "schema": 1, "candidate": "original-onnx-cpu-head", "source": str(source),
        "source_sha256": source_hash, "model_sha256": file_sha256(path),
        "weights_sha256": file_sha256(destination / "weights.bin"),
        "boundary": describe(next(value for value in head.graph.input if value.name == boundary)),
        "inputs": [describe(value) for value in head.graph.input],
        "outputs": [describe(value) for value in head.graph.output],
        "nodes": len(head.graph.node), "initializers": len(head.graph.initializer),
        "operators": dict(sorted(Counter(node.op_type for node in head.graph.node).items())),
        "onnx_version": onnx.__version__, "batch": "dynamic" if args.dynamic_batch else 1,
        "retained_encoder_named_constants": [node.name for node in head.graph.node
            if node.name.startswith("/token_rep_layer/bert_layer/")],
        "transformation": "Original tail extraction only; operators and arithmetic unchanged",
        "inference": "not run", "cpu_session": "not tested",
    }
    del model, head
    gc.collect()
    onnx.checker.check_model(str(path))
    manifest["onnx_checker"] = "passed"
    (destination / "manifest.json").write_text(json.dumps(manifest, indent=2))
    print(json.dumps({"nodes": manifest["nodes"], "operators": manifest["operators"]}))


if __name__ == "__main__":
    main()
