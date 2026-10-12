#!/usr/bin/env python3
"""Reconstruct the pinned GLiNER ONNX weights for isolated Apple backend probes."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np
import onnx
import torch
from gliner import GLiNERConfig
from gliner.modeling.base import SpanModel
from tokenizers import Tokenizer


ONNX_SHA256 = "57af7f3d8d72a3ca4e5619c247344b4b5ee362a5ebcf307c0634d67b65e721c0"
EMBEDDING = "token_rep_layer.bert_layer.model.embeddings.word_embeddings.weight"


def file_sha256(path: Path) -> str:
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def module_path(node_name: str) -> str:
    parts: list[str] = []
    for part in node_name.strip("/").split("/")[:-1]:
        if parts and part.startswith(parts[-1] + "."):
            parts[-1] = part
        else:
            parts.append(part)
    return ".".join(parts)


@dataclass
class ProbeWeights:
    model: SpanModel
    config: GLiNERConfig
    manifest: dict[str, Any]


class SpanWrapper(torch.nn.Module):
    def __init__(self, model: SpanModel):
        super().__init__()
        self.model = model

    def forward(self, input_ids, attention_mask, words_mask, text_lengths, span_idx, span_mask):
        return self.model(
            input_ids=input_ids, attention_mask=attention_mask, words_mask=words_mask,
            text_lengths=text_lengths, span_idx=span_idx, span_mask=span_mask,
        ).logits


def load_span_model(
    model_path: str | Path, config_directory: str | Path,
) -> tuple[SpanModel, dict[str, Any]]:
    model_path = Path(model_path)
    config_directory = Path(config_directory)
    result = load_probe_model(
        model_path, config_directory / "gliner-config.json",
        config_directory / "encoder-config.json", model_path.parent / "tokenizer.json",
    )
    return result.model, result.manifest


def load_probe_model(
    onnx_path: str | Path,
    gliner_config_path: str | Path,
    encoder_config_path: str | Path,
    tokenizer_path: str | Path,
    *,
    load_model: bool = True,
) -> ProbeWeights:
    """Return a strict CPU model or a meta-device shape audit, without inference/network."""
    paths = [Path(p) for p in (onnx_path, gliner_config_path, encoder_config_path, tokenizer_path)]
    fingerprints = {str(path): file_sha256(path) for path in paths}
    if fingerprints[str(paths[0])] != ONNX_SHA256:
        raise ValueError("The probe accepts only the pinned original ONNX model")
    graph = onnx.load(paths[0], load_external_data=False).graph
    initializers = {tensor.name: tensor for tensor in graph.initializer}
    if any(tensor.data_location == onnx.TensorProto.EXTERNAL for tensor in initializers.values()):
        raise ValueError("External ONNX weights are outside this probe's verified artifact")
    config_values = json.loads(paths[1].read_text())
    config_values["encoder_config"] = json.loads(paths[2].read_text())
    tokenizer = Tokenizer.from_file(str(paths[3]))
    vocabulary_size = initializers[EMBEDDING].dims[0]
    entity_token = config_values.get("ent_token", "<<ENT>>")
    entity_id = tokenizer.token_to_id(entity_token)
    if entity_id is None or tokenizer.get_vocab_size() != vocabulary_size:
        raise ValueError("Tokenizer marker/vocabulary does not match the ONNX embedding")
    config_values.update(vocab_size=vocabulary_size, class_token_index=entity_id)
    config_values["encoder_config"]["vocab_size"] = vocabulary_size
    config = GLiNERConfig(**config_values)
    with torch.device("meta"):
        model = SpanModel(config, encoder_from_pretrained=False)
    expected = model.state_dict()
    aliases = {node.output[0]: node.input[0] for node in graph.node if node.op_type == "Identity"}
    used: set[str] = set()
    mapping: dict[str, dict[str, Any]] = {}
    state: dict[str, torch.Tensor] = {}

    def source(name: str) -> str:
        seen: set[str] = set()
        while name in aliases:
            if name in seen:
                raise ValueError("Cyclic ONNX initializer aliases")
            seen.add(name)
            name = aliases[name]
        return name

    def record(name: str, initializer: str, operation: str, shape: tuple[int, ...], array=None):
        if name not in expected:
            raise ValueError(f"Unexpected reconstructed parameter: {name}")
        if tuple(expected[name].shape) != shape:
            raise ValueError(f"Shape mismatch for parameter: {name}")
        entry = {"initializer": initializer, "operation": operation, "shape": list(shape)}
        if name in mapping:
            if mapping[name] != entry:
                raise ValueError(f"Conflicting reconstructed parameter: {name}")
            return
        used.add(initializer)
        mapping[name] = entry
        if load_model:
            state[name] = torch.from_numpy(np.array(array, copy=True))

    def direct(name: str, initializer: str, transpose: bool = False):
        tensor = initializers[initializer]
        if tensor.data_type != onnx.TensorProto.FLOAT:
            raise ValueError("The original checkpoint must contain only FP32 parameters")
        shape = tuple(reversed(tensor.dims)) if transpose else tuple(tensor.dims)
        array = onnx.numpy_helper.to_array(tensor) if load_model else None
        if transpose and load_model:
            array = array.T
        record(name, initializer, "transpose" if transpose else "identity", shape, array)

    for name in expected:
        initializer = source(name)
        if initializer in initializers:
            direct(name, initializer)
    for node in graph.node:
        if node.op_type != "MatMul" or len(node.input) != 2:
            continue
        initializer = source(node.input[1])
        if initializer not in initializers:
            continue
        target = module_path(node.name) + ".weight"
        if target not in expected:
            target = re.sub(r"_(\d+)(?=\.|$)", "", target)
        if target not in expected:
            raise ValueError(f"Unmapped initializer-consuming MatMul: {node.name}")
        direct(target, initializer, transpose=True)
    for node in graph.node:
        if node.op_type != "LSTM":
            continue
        attributes = {a.name: onnx.helper.get_attribute_value(a) for a in node.attribute}
        if attributes.get("direction") != b"bidirectional":
            raise ValueError("Only the original bidirectional LSTM is supported")
        hidden = int(attributes["hidden_size"])
        prefix = module_path(node.name)
        for index, parameter in [(1, "weight_ih"), (2, "weight_hh"), (3, "bias")]:
            initializer = source(node.input[index])
            tensor = initializers[initializer]
            array = onnx.numpy_helper.to_array(tensor) if load_model else None
            for direction, suffix in [(0, ""), (1, "_reverse")]:
                # ONNX i/o/f/c gates become PyTorch i/f/g/o; B stores Wb then Rb.
                reorder = [0, 2, 3, 1]
                if parameter == "bias":
                    for bias_index, bias_name in [(0, "bias_ih"), (1, "bias_hh")]:
                        result = None
                        if load_model:
                            half = array[direction, bias_index * 4 * hidden : (bias_index + 1) * 4 * hidden]
                            result = np.concatenate([np.split(half, 4)[gate] for gate in reorder])
                        record(
                            f"{prefix}.{bias_name}_l0{suffix}", initializer,
                            f"lstm-gates-direction-{direction}-bias-{bias_index}", (4 * hidden,), result,
                        )
                else:
                    result = None
                    if load_model:
                        result = np.concatenate([np.split(array[direction], 4)[gate] for gate in reorder])
                    shape = tuple(tensor.dims[1:])
                    record(
                        f"{prefix}.{parameter}_l0{suffix}", initializer,
                        f"lstm-gates-direction-{direction}", shape, result,
                    )
    missing = sorted(set(expected) - set(mapping))
    unused = sorted(set(initializers) - used)
    if missing or unused:
        raise ValueError(json.dumps({"missing_parameters": missing, "unused_initializers": unused}))
    if load_model:
        model.load_state_dict(state, strict=True, assign=True)
        if any(parameter.is_meta for parameter in model.parameters()):
            raise ValueError("The strict checkpoint still contains meta parameters")
    regenerated_buffers = []
    for name, buffer in list(model.named_buffers()):
        if not buffer.is_meta:
            continue
        if name != "token_rep_layer.bert_layer.model.embeddings.position_ids":
            raise ValueError(f"No verified deterministic constructor for meta buffer: {name}")
        embeddings = model.token_rep_layer.bert_layer.model.embeddings
        if "position_ids" not in embeddings._non_persistent_buffers_set:
            raise ValueError("Only nonpersistent position IDs may be regenerated")
        positions = config.encoder_config.max_position_embeddings
        if tuple(buffer.shape) != (1, positions) or buffer.dtype != torch.int64:
            raise ValueError("The deterministic position ID buffer differs from pinned config")
        if load_model:
            # This exactly reproduces DebertaV2Embeddings.__init__'s nonpersistent buffer.
            embeddings.position_ids = torch.arange(positions, device="cpu").expand((1, -1))
        regenerated_buffers.append({
            "name": name,
            "shape": list(buffer.shape),
            "dtype": str(buffer.dtype),
            "constructor": "torch.arange(config.max_position_embeddings).expand((1, -1))",
            "regenerated": load_model,
        })
    if load_model and any(buffer.is_meta for buffer in model.buffers()):
        raise ValueError("The reconstructed model still contains meta buffers")
    model.eval()
    manifest = {
        "artifacts": fingerprints,
        "onnx_initializers": len(initializers),
        "pytorch_parameters": len(mapping),
        "missing_parameters": missing,
        "unused_initializers": unused,
        "identity_aliases": len(aliases),
        "loaded": load_model,
        "vocabulary_size": vocabulary_size,
        "entity_marker_id": entity_id,
        "mapping": mapping,
        "nonpersistent_buffers": regenerated_buffers,
    }
    return ProbeWeights(model, config, manifest)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--onnx", required=True, type=Path)
    parser.add_argument("--gliner-config", required=True, type=Path)
    parser.add_argument("--encoder-config", required=True, type=Path)
    parser.add_argument("--tokenizer", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--shape-only", action="store_true")
    args = parser.parse_args()
    result = load_probe_model(
        args.onnx, args.gliner_config, args.encoder_config, args.tokenizer,
        load_model=not args.shape_only,
    )
    args.manifest.write_text(json.dumps(result.manifest, indent=2) + "\n")
    print(json.dumps({k: v for k, v in result.manifest.items() if k != "mapping"}))


if __name__ == "__main__":
    main()
