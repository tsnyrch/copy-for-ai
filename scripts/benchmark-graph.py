#!/usr/bin/env python3
"""Build a temporary Rust probe for verified experimental graphs only."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tomllib


def digest(path):
    checksum = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(8 * 1024 * 1024), b""):
            checksum.update(block)
    return checksum.hexdigest()


def prepare_workspace(root, output):
    workspace = output / "workspace"
    if workspace.exists():
        raise ValueError("use a fresh experiment directory")
    workspace.mkdir(parents=True)
    for name in ["core", "ner"]:
        shutil.copytree(root / "crates" / name, workspace / "crates" / name)
    shutil.copy2(root / "Cargo.lock", workspace / "Cargo.lock")
    cargo = (root / "Cargo.toml").read_text()
    members = 'members = ["crates/core", "crates/desktop", "crates/cli", "crates/ner"]'
    default = 'default-members = ["crates/desktop"]'
    if cargo.count(members) != 1 or cargo.count(default) != 1:
        raise ValueError("unexpected workspace configuration")
    cargo = cargo.replace(members, 'members = ["crates/core", "crates/ner"]')
    (workspace / "Cargo.toml").write_text(cargo.replace(default, 'default-members = ["crates/ner"]'))
    source = workspace / "crates/ner/src/lib.rs"
    contents = source.read_text()
    original = ".commit_from_file(directory.join(config.variant.file_name()))"
    replacement = '.commit_from_file(std::env::var_os("COPY_FOR_AI_GRAPH_PROBE").map_or_else(|| directory.join(config.variant.file_name()), std::path::PathBuf::from))'
    if contents.count(original) != 1:
        raise ValueError("unexpected model loading implementation")
    source.write_text(contents.replace(original, replacement))
    manifest = {"source_root": str(root), "source_files": {}, "probe_delta": {"original": original, "replacement": replacement}}
    for directory in [root / "crates/core", root / "crates/ner"]:
        for path in sorted(directory.rglob("*.rs")):
            manifest["source_files"][str(path.relative_to(root))] = digest(path)
    (output / "source-manifest.json").write_text(json.dumps(manifest, indent=2))
    version = subprocess.run(["rustc", "-vV"], capture_output=True, text=True, check=True).stdout
    host = next(line.removeprefix("host: ") for line in version.splitlines() if line.startswith("host: "))
    with (output / "workspace-resolution.txt").open("w") as log:
        subprocess.run(["cargo", "metadata", "--offline", "--format-version", "1", "--filter-platform", host], cwd=workspace, stdout=log, stderr=subprocess.STDOUT, check=True)
    original_packages = {(value["name"], value["version"], value.get("source")) for value in tomllib.loads((root / "Cargo.lock").read_text())["package"]}
    isolated_packages = {(value["name"], value["version"], value.get("source")) for value in tomllib.loads((workspace / "Cargo.lock").read_text())["package"]}
    if not isolated_packages <= original_packages:
        raise ValueError("isolated workspace changed dependency versions")
    return workspace


def verify_candidate(candidate, model_dir):
    manifest = json.loads((candidate / "manifest.json").read_text())
    checks = [(model_dir / "model.onnx", "source_sha256"), (candidate / "model.onnx", "model_sha256"), (candidate / "weights.bin", "weights_sha256")]
    for path, key in checks:
        if digest(path) != manifest[key]:
            raise ValueError("experimental graph checksum mismatch")
    return candidate / "model.onnx"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--model-dir", type=Path, required=True)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, action="append", required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--mode", choices=["speed_probe", "benchmark"], default="speed_probe")
    parser.add_argument("--executable", type=Path)
    parser.add_argument("--bytes", type=int, default=102400)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    model_dir = args.model_dir.resolve()
    candidates = [(path.resolve().name, verify_candidate(path.resolve(), model_dir)) for path in args.candidate]
    if args.executable:
        executable = args.executable.resolve()
    else:
        workspace = prepare_workspace(root, output)
        command = ["cargo", "build", "-p", "copy-for-ai-ner", "--release", "--example", args.mode, "--features", "runtime-probe", "--locked", "--offline", "--target-dir", str(args.target_dir.resolve())]
        with (output / "build.txt").open("w") as log:
            subprocess.run(command, cwd=workspace, stdout=log, stderr=subprocess.STDOUT, check=True)
        executable = args.target_dir.resolve() / "release/examples" / args.mode
    shutil.copy2(executable, output / args.mode)
    env = dict(os.environ, COPY_FOR_AI_MODEL_DIR=str(model_dir), ORT_DYLIB_PATH=str(args.library.resolve()))
    runs = [("baseline", None), *candidates]
    metadata = {"library": str(args.library.resolve()), "library_sha256": digest(args.library.resolve()), "model_dir": str(model_dir), "mode": args.mode, "candidates": [str(graph) for _, graph in candidates], "executable_sha256": digest(output / args.mode), "runs": []}
    for name, graph in runs:
        if graph is None:
            env.pop("COPY_FOR_AI_GRAPH_PROBE", None)
        else:
            env["COPY_FOR_AI_GRAPH_PROBE"] = str(graph)
        arguments = [str(output / args.mode)]
        if args.mode == "speed_probe":
            arguments += [str(args.corpus.resolve())]
        else:
            arguments += [str(args.bytes), "256", "1", "8", "fp32", "varied"]
        with (output / f"{name}.txt").open("w") as log:
            result = subprocess.run(arguments, env=env, stdout=log, stderr=subprocess.STDOUT)
        metadata["runs"].append({"name": name, "exit_code": result.returncode})
        (output / "run-manifest.json").write_text(json.dumps(metadata, indent=2))
        print(name, "exit", result.returncode, flush=True)
        if name == "baseline" and result.returncode:
            raise SystemExit("baseline failed")
    if any(row["exit_code"] for row in metadata["runs"]):
        raise SystemExit("one or more graph candidates failed")


if __name__ == "__main__":
    main()
