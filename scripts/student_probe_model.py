#!/usr/bin/env python3
"""Strict local MiniLM encoder and an untrained BIO head for latency probes."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path

os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"

import torch
from transformers import BertConfig, BertModel, XLMRobertaTokenizer

MODEL_ID = "microsoft/Multilingual-MiniLM-L12-H384"
REVISION = "6e8c1ec6b4ec4e3fc6eb7d2cd834fcd582b61daf"
MODEL_SHA256 = "cce170910f4d4f3b45be025508f51ef1ad6a1de69f2d46dce7e4603ad31aaaeb"
CATEGORIES = ("person", "organization", "location", "address", "event")
BIO_LABELS = ("O", *(label for category in CATEGORIES for label in (f"B-{category}", f"I-{category}")))


def file_sha256(path: Path) -> str:
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def verify_manifest(model_directory, manifest_path=None):
    directory = Path(model_directory).resolve()
    path = Path(manifest_path) if manifest_path else directory / "manifest.json"
    manifest = json.loads(path.read_text())
    if manifest.get("verified") is not True or manifest.get("model_id") != MODEL_ID or manifest.get("revision") != REVISION:
        raise ValueError("Student probe requires the exact verified MiniLM source revision")
    files = manifest.get("files", {})
    required = {"config.json", "pytorch_model.bin", "sentencepiece.bpe.model", "tokenizer_config.json", "special_tokens_map.json"}
    if not required.issubset(files):
        raise ValueError("Pinned manifest is missing required model or tokenizer artifacts")
    if files["pytorch_model.bin"]["sha256"] != MODEL_SHA256:
        raise ValueError("Student checkpoint differs from the exact pretrained model fingerprint")
    for name, entry in files.items():
        artifact = (directory / name).resolve()
        if not artifact.is_relative_to(directory):
            raise ValueError("Artifact path escapes the isolated model directory")
        expected = entry["sha256"]
        if len(expected) != 64 or artifact.stat().st_size != entry["bytes"] or file_sha256(artifact) != expected:
            raise ValueError(f"Pinned student artifact fingerprint mismatch: {name}")
        if entry.get("lfs_sha256") is not None and entry["lfs_sha256"] != expected:
            raise ValueError(f"Student artifact differs from the recorded LFS fingerprint: {name}")
    return directory, manifest, file_sha256(path)


class StudentProbe(torch.nn.Module):
    """An untrained classification head; outputs provide no anonymization quality."""

    def __init__(self, encoder: BertModel, head_seed: int):
        super().__init__()
        self.encoder = encoder
        with torch.random.fork_rng(devices=[]):
            torch.manual_seed(head_seed)
            self.head = torch.nn.Linear(encoder.config.hidden_size, len(BIO_LABELS))
            torch.nn.init.normal_(self.head.weight, mean=0.0, std=0.02)
            torch.nn.init.normal_(self.head.bias, mean=0.0, std=0.005)
        if torch.count_nonzero(self.head.weight).item() != self.head.weight.numel() or torch.count_nonzero(self.head.bias).item() != self.head.bias.numel():
            raise ValueError("Latency probe requires a deterministic nonzero BIO head")

    def forward(self, input_ids, attention_mask):
        hidden = self.encoder(input_ids=input_ids, attention_mask=attention_mask,
                              output_attentions=False, output_hidden_states=False, return_dict=False)[0]
        return self.head(hidden)


def load_student(model_directory, layers=12, head_seed=17, manifest_path=None):
    if layers not in (6, 12):
        raise ValueError("Student probe supports only the original 12 layers or selected 6 layers")
    directory, source, manifest_sha = verify_manifest(model_directory, manifest_path)
    raw_config = json.loads((directory / "config.json").read_text())
    expected = {"model_type": "bert", "num_hidden_layers": 12, "hidden_size": 384,
                "intermediate_size": 1536, "num_attention_heads": 12, "vocab_size": 250037,
                "max_position_embeddings": 512}
    if any(raw_config.get(name) != value for name, value in expected.items()):
        raise ValueError("Pinned source config differs from the MiniLM architecture contract")
    config = BertConfig.from_dict(raw_config)
    config._attn_implementation = "eager"
    state = torch.load(directory / "pytorch_model.bin", map_location="cpu", weights_only=True)
    if not isinstance(state, dict) or not state or any(not isinstance(name, str) or not isinstance(value, torch.Tensor) for name, value in state.items()):
        raise ValueError("Pinned checkpoint must contain a plain tensor state dictionary")
    has_pooler = "pooler.dense.weight" in state
    encoder = BertModel(config, add_pooling_layer=has_pooler)
    expected_state = encoder.state_dict()
    nonpersistent = []
    for name in ("embeddings.position_ids", "embeddings.token_type_ids"):
        if name in state and name not in expected_state:
            reference = getattr(encoder.embeddings, name.rsplit(".", 1)[1])
            if not torch.equal(state[name], reference):
                raise ValueError("Checkpoint buffer differs from the exact BERT constructor buffer")
            del state[name]
            nonpersistent.append(name)
    encoder.load_state_dict(state, strict=True, assign=True)
    if any(parameter.dtype != torch.float32 or parameter.is_meta for parameter in encoder.parameters()):
        raise ValueError("Strict pretrained encoder must contain materialized FP32 parameters")
    pretrained_parameters = sum(parameter.numel() for parameter in encoder.parameters())
    indices = list(range(12)) if layers == 12 else [1, 3, 5, 7, 9, 11]
    original_layers = encoder.encoder.layer
    encoder.encoder.layer = torch.nn.ModuleList([original_layers[index] for index in indices])
    encoder.config.num_hidden_layers = len(indices)
    model = StudentProbe(encoder, head_seed).eval()
    manifest = {
        "model_id": MODEL_ID, "revision": REVISION, "source_manifest_sha256": manifest_sha,
        "model_source_sha256": source["files"]["pytorch_model.bin"]["sha256"],
        "config_sha256": source["files"]["config.json"]["sha256"],
        "source_files_sha256": {name: entry["sha256"] for name, entry in source["files"].items()},
        "pretrained_state_dict_strict": True, "pretrained_parameters": pretrained_parameters,
        "selected_layer_indices": indices, "layer_selection_is_untrained": layers == 6,
        "pooler_loaded_but_unused": has_pooler, "verified_nonpersistent_buffers": nonpersistent,
        "attention_implementation": "eager", "head_seed": head_seed, "head_nonzero": True,
        "head_classes": len(BIO_LABELS), "bio_labels": BIO_LABELS, "head_untrained": True,
        "anonymization_quality_verified": False, "total_parameters": sum(p.numel() for p in model.parameters()),
    }
    return model, manifest


def load_tokenizer(model_directory, manifest_path=None):
    directory, _, _ = verify_manifest(model_directory, manifest_path)
    tokenizer = XLMRobertaTokenizer.from_pretrained(str(directory), local_files_only=True)
    if len(tokenizer) > 250037:
        raise ValueError("Local XLM-R tokenizer exceeds the pinned MiniLM embedding vocabulary")
    return tokenizer


def load_trained_student(model_directory, checkpoint_path, layers=12, head_seed=17, manifest_path=None):
    checkpoint = Path(checkpoint_path)
    if checkpoint.is_dir():
        checkpoint = checkpoint / "checkpoint.pt"
    report_path = checkpoint.parent / "report.json"
    report = json.loads(report_path.read_text())
    if report.get("status") != "completed_supervised_trial" or report.get("checkpoint_head_trained") is not True:
        raise ValueError("Trained checkpoint requires a completed supervised trial report")
    if report.get("dtype") != "float32" or report.get("teacher_loss_applied") is not False:
        raise ValueError("Only explicit FP32 supervised checkpoints without teacher loss are supported")
    if report.get("synthetic_smoke_only") is not True or report.get("heldout_acceptance_fixtures_used") is not False:
        raise ValueError("Checkpoint report does not preserve the synthetic training/acceptance separation")
    if report.get("frozen_embeddings") is not True or report.get("frozen_unused_pooler") is not True:
        raise ValueError("Checkpoint requires unchanged public embeddings and unused pooler")
    if report.get("completed_steps", 0) < 1 or report.get("settings", {}).get("layers") != layers:
        raise ValueError("Completed trial does not match the requested encoder layer selection")
    digest = file_sha256(checkpoint)
    if report.get("checkpoint_sha256") != digest:
        raise ValueError("Trained checkpoint fingerprint differs from its completed trial report")
    model, metadata = load_student(model_directory, layers, head_seed, manifest_path)
    source = report.get("source", {})
    source_keys = ("model_id", "revision", "source_manifest_sha256", "model_source_sha256", "config_sha256",
                   "source_files_sha256", "selected_layer_indices", "head_seed", "head_classes")
    if any(source.get(key) != metadata[key] for key in source_keys) or source.get("bio_labels") != list(BIO_LABELS):
        raise ValueError("Trained checkpoint source, BIO label order or architecture differs from the pinned model")
    state = torch.load(checkpoint, map_location="cpu", weights_only=True)
    if not isinstance(state, dict) or not state or any(not isinstance(name, str) or not isinstance(value, torch.Tensor) for name, value in state.items()):
        raise ValueError("Trained checkpoint must contain a plain tensor state dictionary")
    reference = model.state_dict()
    frozen = [name for name in reference if name.startswith(("encoder.embeddings.", "encoder.pooler."))]
    if any(name not in state or not torch.equal(state[name], reference[name]) for name in frozen):
        raise ValueError("Trained checkpoint modified a frozen public embedding or pooler parameter")
    model.load_state_dict(state, strict=True, assign=True)
    if any(parameter.dtype != torch.float32 or parameter.is_meta or not torch.isfinite(parameter).all().item()
           for parameter in model.parameters()):
        raise ValueError("Trained checkpoint requires finite materialized FP32 parameters")
    if torch.count_nonzero(model.head.weight).item() == 0:
        raise ValueError("Trained BIO head cannot be a constant zero projection")
    metadata.update({
        "head_untrained": False, "checkpoint_sha256": digest, "training_report_sha256": file_sha256(report_path),
        "layer_selection_is_untrained": False, "encoder_supervised_training_applied": True,
        "completed_supervised_steps": report["completed_steps"], "teacher_applied": False,
        "teacher_loss_applied": False, "frozen_public_parameters_verified": frozen,
        "synthetic_smoke_only": True, "anonymization_quality_verified": False,
    })
    return model.eval(), metadata
