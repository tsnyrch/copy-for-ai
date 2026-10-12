import json
import os
from pathlib import Path
import runpy
import sys
from types import SimpleNamespace

os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
os.environ["PYTORCH_ENABLE_MPS_FALLBACK"] = "0"
os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
os.environ["TOKENIZERS_PARALLELISM"] = "false"
root = Path("/Users/tsnyrch/my-projects/copy-for-ai")
sys.path.insert(0, str(root / "scripts"))
helpers = runpy.run_path(str(root / "scripts/evaluate-student-quality.py"))
ct, np, torch = helpers["ct"], helpers["np"], helpers["torch"]
torch.set_num_threads(8)
torch.set_num_interop_threads(1)
base = Path("/private/tmp/copy-ai-student-2026-10-13")
args = SimpleNamespace(layers=12, batch=8, bucket=256, overlap=32)
report = {"case": "case-13-00", "synthetic_only": True, "gpu_predictions": 0,
          "mps_fallback": os.environ["PYTORCH_ENABLE_MPS_FALLBACK"], "tolerance_unchanged": 0.0001}


class CaptureRuntime:
    def __init__(self, runtime):
        self.runtime = runtime

    def predict(self, feed):
        report["gpu_predictions"] += 1
        if report["gpu_predictions"] > 1:
            raise ValueError("diagnostic_allows_only_one_gpu_prediction")
        self.feed = feed
        self.output = self.runtime.predict(feed)
        return self.output


class CaptureReference:
    def __init__(self, model):
        self.model = model

    def __call__(self, *inputs):
        self.output = self.model(*inputs)
        return self.output


def stats(mask, difference, old_labels, new_labels):
    count = int(mask.sum())
    return {"positions": count, "max_abs": float(difference[mask].max()) if count else None,
            "argmax_mismatch": int(np.count_nonzero(old_labels[mask] != new_labels[mask]))}


try:
    model, metadata = helpers["load_trained_student"](base / "model", base / "train-12-200", layers=12)
    tokenizer = helpers["XLMRobertaTokenizerFast"](vocab_file=str(base / "model/sentencepiece.bpe.model"))
    runtime = ct.models.MLModel(str(base / "minilm-12-trained-b8.mlpackage"), compute_units=ct.ComputeUnit.CPU_AND_GPU)
    helpers["package_check"](runtime, metadata, args)
    raw = Path("/private/tmp/copy-ai-speed-2026-10-13/corpus/quality/case-13-00.txt").read_bytes()
    text = raw.decode("utf-8")
    report["input_bytes"] = len(raw)
    offsets, windows, empty = helpers["encode"](text, tokenizer, args.bucket, args.overlap)
    report["windows"] = len(windows)
    report["original_tokens"] = len(offsets)
    report["trimmed_empty_tokens"] = empty
    if len(windows) != 1:
        raise ValueError("diagnostic_case_requires_exactly_one_window")
    capture, reference = CaptureRuntime(runtime), CaptureReference(model)
    try:
        result = helpers["process"](text, tokenizer, capture, reference, args)
        report["process_completed"] = True
    except ValueError as error:
        report["process_completed"] = False
        report["process_error_code"] = str(error)
    actual = capture.output["token_logits"]
    expected = reference.output.detach().numpy()
    difference = np.abs(expected.astype(np.float64) - actual.astype(np.float64))
    old_labels, new_labels = expected.argmax(-1), actual.argmax(-1)
    tokens = np.zeros(actual.shape[:2], dtype=bool)
    tokens[0, 1:1 + len(offsets)] = True
    nonempty = tokens.copy()
    for index, (start, end) in enumerate(offsets):
        if start == end:
            nonempty[0, index + 1] = False
    special = np.zeros_like(tokens)
    special[0, [0, len(offsets) + 1]] = True
    padding = np.zeros_like(tokens)
    padding[0] = capture.feed["attention_mask"][0] == 0
    dummy = np.zeros_like(tokens)
    dummy[1:] = True
    report["delta"] = {
        "all_positions": stats(np.ones_like(tokens), difference, old_labels, new_labels),
        "real_original_tokens": stats(tokens, difference, old_labels, new_labels),
        "owned_nonempty_original_tokens": stats(nonempty, difference, old_labels, new_labels),
        "trimmed_empty_original_tokens": stats(tokens & ~nonempty, difference, old_labels, new_labels),
        "real_specials": stats(special, difference, old_labels, new_labels),
        "real_padding": stats(padding, difference, old_labels, new_labels),
        "unscored_dummy_rows": stats(dummy, difference, old_labels, new_labels),
    }
    report["largest_delta_coordinates"] = [int(index) for index in np.unravel_index(difference.argmax(), difference.shape)]
    report["checkpoint_sha256"] = metadata["checkpoint_sha256"]
    report["training_report_sha256"] = metadata["training_report_sha256"]
    report["diagnostic_completed"] = True
except Exception as error:
    report["diagnostic_completed"] = False
    report["diagnostic_error_type"] = type(error).__name__
finally:
    (base / "case-13-diagnostic.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
