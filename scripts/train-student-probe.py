#!/usr/bin/env python3
"""Bounded synthetic supervision trial; no production model or teacher distillation."""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import random
import tempfile
import time

os.environ["PYTORCH_ENABLE_MPS_FALLBACK"] = "0"
os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"

import torch
from transformers import XLMRobertaTokenizerFast

from student_probe_model import BIO_LABELS, CATEGORIES, load_student, load_tokenizer, load_trained_student


def digest(path):
    with Path(path).open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def read_corpus(directory):
    manifest = json.loads((directory / "manifest.json").read_text())
    if manifest.get("synthetic_only") is not True or manifest.get("existing_acceptance_fixtures_imported") is not False:
        raise ValueError("Training requires an independent synthetic corpus manifest")
    splits = {split: [json.loads(line) for line in (directory / f"{split}.jsonl").read_text().splitlines()]
              for split in ("train", "dev")}
    keys = {}
    for split, rows in splits.items():
        if not rows or any(row["split"] != split for row in rows):
            raise ValueError("Corpus split labels are missing or inconsistent")
        keys[split] = {key for row in rows for key in row["group_keys"]}
    if keys["train"] & keys["dev"]:
        raise ValueError("Synthetic training/development group leakage")
    if len({row["id"] for rows in splits.values() for row in rows}) != sum(map(len, splits.values())):
        raise ValueError("Duplicate synthetic document identifiers")
    return splits, manifest


def prepare(row, tokenizer, max_length):
    text, spans = row["text"], row["spans"]
    prefix = [0]
    for char in text:
        prefix.append(prefix[-1] + len(char.encode("utf-8")))
    boundaries, previous = set(prefix), 0
    for span in spans:
        start, end = span["start"], span["end"]
        if span["category"] not in CATEGORIES or start not in boundaries or end not in boundaries or not previous <= start < end:
            raise ValueError(f"Invalid synthetic UTF-8 gold span: {row['id']}")
        previous = end
    if row["annotation_status"] == "review":
        if spans:
            raise ValueError("Unreviewed synthetic text must not claim gold spans")
        return {"row": row, "reason": "review_required"}
    if row["annotation_status"] != "synthetic_gold" or not text:
        raise ValueError("Unsupported synthetic annotation status or empty document")
    encoded = tokenizer(text, return_offsets_mapping=True, return_special_tokens_mask=True,
                        padding="max_length", max_length=max_length, truncation=False)
    if len(encoded["input_ids"]) > max_length:
        return {"row": row, "reason": "exceeds_training_window"}
    offsets, labels, hits = [], [], [[] for _ in spans]
    for index, ((start, end), special, active) in enumerate(zip(
        encoded["offset_mapping"], encoded["special_tokens_mask"], encoded["attention_mask"], strict=True,
    )):
        if not 0 <= start <= end <= len(text):
            raise ValueError("Tokenizer offset is outside the original character coordinates")
        while start < end and text[start].isspace():
            start += 1
        while end > start and text[end - 1].isspace():
            end -= 1
        byte_start, byte_end = prefix[start], prefix[end]
        offsets.append((byte_start, byte_end))
        if special or not active or start == end:
            labels.append(-100)
            continue
        overlap = [i for i, span in enumerate(spans) if byte_start < span["end"] and byte_end > span["start"]]
        if not overlap:
            labels.append(0)
        elif len(overlap) != 1 or not spans[overlap[0]]["start"] <= byte_start < byte_end <= spans[overlap[0]]["end"]:
            return {"row": row, "reason": "token_crosses_gold_boundary"}
        else:
            span_index = overlap[0]
            category = spans[span_index]["category"]
            labels.append(BIO_LABELS.index(f"{'I' if hits[span_index] else 'B'}-{category}"))
            hits[span_index].append(index)
    for span, indices in zip(spans, hits, strict=True):
        if not indices or offsets[indices[0]][0] != span["start"] or offsets[indices[-1]][1] != span["end"]:
            return {"row": row, "reason": "gold_boundary_not_representable"}
    return {"row": row, "reason": None, "input_ids": encoded["input_ids"],
            "attention_mask": encoded["attention_mask"], "labels": labels, "offsets": offsets}


def batch(rows):
    return {key: torch.tensor([row[key] for row in rows], dtype=torch.long, device="mps")
            for key in ("input_ids", "attention_mask", "labels")}


def decode(labels, prepared):
    result, current = set(), None
    for label, offset, valid in zip(labels, prepared["offsets"], prepared["labels"], strict=True):
        if offset[0] == offset[1]:
            continue
        category = BIO_LABELS[label][2:] if label else None
        begin = label and BIO_LABELS[label].startswith("B-")
        if valid == -100 or not category or begin or current is not None and current[2] != category:
            if current is not None:
                result.add(tuple(current))
                current = None
        if valid != -100 and category:
            if current is None:
                current = [offset[0], offset[1], category]
            else:
                current[1] = offset[1]
    if current is not None:
        result.add(tuple(current))
    return result


def scores(counts):
    tp, fp, fn = (counts[key] for key in ("tp", "fp", "fn"))
    return dict(counts, precision=tp / (tp + fp) if tp + fp else 0.0,
                recall=tp / (tp + fn) if tp + fn else 0.0,
                f1=2 * tp / (2 * tp + fp + fn) if 2 * tp + fp + fn else 0.0)


def evaluate(model, prepared, batch_size):
    model.eval()
    predictions = {}
    usable = [row for row in prepared if row["reason"] is None]
    with torch.inference_mode():
        for start in range(0, len(usable), batch_size):
            rows = usable[start:start + batch_size]
            inputs = batch(rows)
            labels = model(inputs["input_ids"], inputs["attention_mask"]).argmax(-1).cpu().tolist()
            predictions.update({row["row"]["id"]: decode(prediction, row)
                                for row, prediction in zip(rows, labels, strict=True)})
    total, categories, languages = Counter(), {category: Counter() for category in CATEGORIES}, {}
    details = []
    for item in prepared:
        row = item["row"]
        gold = {(span["start"], span["end"], span["category"]) for span in row["spans"]}
        predicted = predictions.get(row["id"], set())
        exact = item["reason"] is None and predicted == gold
        language = languages.setdefault(row["language"], Counter())
        for bucket in (total, language):
            bucket.update(documents=1, exact_documents=int(exact), unresolved_documents=int(item["reason"] is not None),
                          tp=len(gold & predicted), fp=len(predicted - gold), fn=len(gold - predicted))
        for category, bucket in categories.items():
            expected = {span for span in gold if span[2] == category}
            actual = {span for span in predicted if span[2] == category}
            bucket.update(tp=len(expected & actual), fp=len(actual - expected), fn=len(expected - actual))
        details.append({"id": row["id"], "language": row["language"], "reason": item["reason"],
                        "gold_spans": len(gold), "predicted_spans": len(predicted), "exact": exact})
    result = scores(total)
    result.update(exact_document_fraction=total["exact_documents"] / total["documents"],
                  all_dev_documents_in_denominator=True,
                  unresolved_reason_counts=dict(Counter(item["reason"] for item in prepared if item["reason"])),
                  by_category={key: scores(value) for key, value in categories.items()},
                  by_language={key: scores(value) for key, value in languages.items()}, document_results=details,
                  review_cases_have_no_entity_gold=True,
                  unrepresentable_gold_spans_count_as_false_negatives=True)
    return result


def memory():
    return {"allocated_bytes": torch.mps.current_allocated_memory(),
            "driver_allocated_bytes": torch.mps.driver_allocated_memory()}


def run(args):
    output = args.output.resolve()
    if not any(output.is_relative_to(root.resolve()) for root in (Path("/private/tmp"), Path(tempfile.gettempdir()))):
        raise ValueError("Trial checkpoints belong in an isolated temporary directory")
    if output.exists():
        raise ValueError("Training requires a fresh output directory")
    if not 1 <= args.steps <= 2000 or not 1 <= args.batch_size <= 64 or not 16 <= args.max_length <= 512 or args.max_seconds <= 0:
        raise ValueError("Invalid bounded training limits")
    if not torch.backends.mps.is_available():
        raise RuntimeError("MPS is unavailable; CPU fallback is disabled")
    torch.manual_seed(args.seed)
    torch.mps.manual_seed(args.seed)
    rng = random.Random(args.seed)
    splits, corpus_manifest = read_corpus(args.corpus)
    tokenizer = XLMRobertaTokenizerFast.from_pretrained(str(args.model), local_files_only=True)
    model, source = (load_trained_student(args.model, args.evaluate_checkpoint, layers=args.layers)
                     if args.evaluate_checkpoint else load_student(args.model, layers=args.layers, head_seed=17))
    reference_tokenizer = load_tokenizer(args.model)
    if tokenizer.get_vocab() != reference_tokenizer.get_vocab():
        raise ValueError("Fast tokenizer token IDs differ from the verified SentencePiece tokenizer")
    if max(tokenizer.get_vocab().values()) >= model.encoder.embeddings.word_embeddings.num_embeddings:
        raise ValueError("Fast tokenizer token IDs exceed the verified MiniLM embedding")
    del reference_tokenizer
    prepared = {split: [prepare(row, tokenizer, args.max_length) for row in rows] for split, rows in splits.items()}
    if args.evaluate_checkpoint:
        checkpoint = args.evaluate_checkpoint / "checkpoint.pt" if args.evaluate_checkpoint.is_dir() else args.evaluate_checkpoint
        original_report = checkpoint.parent / "report.json"
        original_hashes = {str(path): digest(path) for path in (checkpoint, original_report)}
        model = model.to(device="mps", dtype=torch.float32).eval()
        output.mkdir(parents=True)
        started = time.monotonic()
        report = {"status": "evaluating_dev", "mode": "existing_checkpoint_dev_recheck", "source": source,
                  "synthetic_smoke_only": True, "production_accepted": False, "teacher_loss_applied": False,
                  "heldout_acceptance_fixtures_used": False, "training_performed": False,
                  "decoder_policy": "skip_empty_trimmed_offsets_v2", "original_artifact_hashes": original_hashes,
                  "training_script_sha256": digest(__file__), "dtype": "float32", "device": "mps",
                  "corpus_hashes": {name: digest(args.corpus / name) for name in ("train.jsonl", "dev.jsonl", "manifest.json")},
                  "memory_scope": "Current allocator/driver snapshots; these are not a measured peak"}
        try:
            report["dev"] = evaluate(model, prepared["dev"], args.batch_size)
            if any(digest(path) != expected for path, expected in original_hashes.items()):
                raise RuntimeError("Original training artifacts changed during read-only re-evaluation")
            report.update(status="completed_dev_recheck", elapsed_seconds=time.monotonic() - started,
                          memory_after_evaluation=memory(), original_artifacts_unchanged=True)
        except Exception as error:
            report.update(status="failed", error_type=type(error).__name__)
            raise
        finally:
            (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps({"status": report["status"], "dev_f1": report["dev"]["f1"],
                          "dev_exact_document_fraction": report["dev"]["exact_document_fraction"]}))
        return
    train = [row for row in prepared["train"] if row["reason"] is None]
    if not train:
        raise ValueError("No representable, reviewed synthetic training examples")
    for parameter in model.encoder.embeddings.parameters():
        parameter.requires_grad_(False)
    if model.encoder.pooler is not None:
        for parameter in model.encoder.pooler.parameters():
            parameter.requires_grad_(False)
    model = model.to(device="mps", dtype=torch.float32).train()
    trainable = [parameter for parameter in model.parameters() if parameter.requires_grad]
    optimizer = torch.optim.AdamW(trainable, lr=args.learning_rate, weight_decay=0.01,
                                 foreach=False, fused=False, capturable=False)
    output.mkdir(parents=True)
    report = {"status": "training", "synthetic_smoke_only": True, "production_accepted": False,
              "teacher_loss_applied": False, "heldout_acceptance_fixtures_used": False,
              "source": source, "corpus": corpus_manifest, "settings": vars(args) | {"model": str(args.model), "corpus": str(args.corpus), "output": str(output)},
              "corpus_hashes": {name: digest(args.corpus / name) for name in ("train.jsonl", "dev.jsonl", "manifest.json")},
              "frozen_embeddings": True, "frozen_unused_pooler": True, "dtype": "float32", "device": "mps",
              "trainable_parameters": sum(parameter.numel() for parameter in trainable),
              "memory_scope": "Current allocator/driver snapshots; these are not a measured peak",
              "training_script_sha256": digest(__file__),
              "train_excluded_reasons": dict(Counter(row["reason"] for row in prepared["train"] if row["reason"])),
              "train_examples": len(train), "memory_before_training": memory(), "losses": []}
    started = time.monotonic()
    try:
        for step in range(args.steps):
            inputs = batch(rng.choices(train, k=args.batch_size))
            optimizer.zero_grad(set_to_none=True)
            logits = model(inputs["input_ids"], inputs["attention_mask"])
            loss = torch.nn.functional.cross_entropy(logits.reshape(-1, len(BIO_LABELS)), inputs["labels"].reshape(-1))
            if not torch.isfinite(loss).item():
                raise RuntimeError("Nonfinite supervised training loss")
            loss.backward()
            norm = torch.nn.utils.clip_grad_norm_(trainable, 1.0, foreach=False, error_if_nonfinite=True)
            if step == 0:
                modules = {f"encoder_layer_{index}": layer for index, layer in enumerate(model.encoder.encoder.layer)} | {"head": model.head}
                active = {name: any(p.grad is not None and torch.count_nonzero(p.grad).item() > 0 for p in module.parameters())
                          for name, module in modules.items()}
                if not all(active.values()) or norm.item() <= 0:
                    raise RuntimeError("The selected encoder layers and head must receive nonzero gradients")
                report["first_backward"] = {"finite": True, "gradient_norm": norm.item(), "active_modules": active, "memory": memory()}
            optimizer.step()
            report["losses"].append(loss.item())
            if step == 0:
                report["memory_after_optimizer_state"] = memory()
            if step == 0 or (step + 1) % 25 == 0:
                print(json.dumps({"step": step + 1, "loss": loss.item(), "elapsed_seconds": time.monotonic() - started}), flush=True)
            if time.monotonic() - started >= args.max_seconds:
                break
        report.update(completed_steps=len(report["losses"]), training_seconds=time.monotonic() - started, memory_after_training=memory())
        report["dev"] = evaluate(model, prepared["dev"], args.batch_size)
        torch.save(model.cpu().state_dict(), output / "checkpoint.pt")
        model.encoder.config.num_labels = len(BIO_LABELS)
        model.encoder.config.id2label = dict(enumerate(BIO_LABELS))
        model.encoder.config.label2id = {label: index for index, label in enumerate(BIO_LABELS)}
        model.encoder.config.save_pretrained(output)
        tokenizer.save_pretrained(output)
        report.update(status="completed_supervised_trial", checkpoint_sha256=digest(output / "checkpoint.pt"),
                      checkpoint_head_trained=True, dev_generalization_proof=False)
    except Exception as error:
        report.update(status="failed", error_type=type(error).__name__, elapsed_seconds=time.monotonic() - started)
        raise
    finally:
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"status": report["status"], "completed_steps": report["completed_steps"],
                      "dev_f1": report["dev"]["f1"], "dev_exact_document_fraction": report["dev"]["exact_document_fraction"]}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--steps", type=int, default=200)
    parser.add_argument("--batch-size", type=int, default=8)
    parser.add_argument("--layers", type=int, choices=(6, 12), default=6)
    parser.add_argument("--max-length", type=int, default=256)
    parser.add_argument("--max-seconds", type=float, default=600)
    parser.add_argument("--learning-rate", type=float, default=2e-5)
    parser.add_argument("--seed", type=int, default=17)
    parser.add_argument("--evaluate-checkpoint", type=Path)
    run(parser.parse_args())


if __name__ == "__main__":
    main()
