#!/usr/bin/env bash
set -euo pipefail

task_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
task_profile="debug"
if [[ "${1:-}" == "--release" ]]; then
  task_profile="release"
elif [[ $# -ne 0 ]]; then
  echo "Usage: scripts/package-macos.sh [--release]" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This package requires macOS." >&2
  exit 2
fi

cd "$task_root"
if [[ "$task_profile" == "release" ]]; then
  cargo build --locked -p copy-for-ai-desktop --release
else
  cargo build --locked -p copy-for-ai-desktop
fi
task_bundle="$task_root/target/$task_profile/Copy for AI.app"
python3 scripts/install-model.py --verify-only
mkdir -p "$task_bundle/Contents/MacOS" "$task_bundle/Contents/Resources/models/gliner-multi"
cp -c models/gliner-multi/model.onnx models/gliner-multi/tokenizer.json "$task_bundle/Contents/Resources/models/gliner-multi/"
mkdir -p "$task_bundle/Contents/Resources"
cp "crates/desktop/assets/CopyAI.icns" "$task_bundle/Contents/Resources/CopyAI.icns"
cp "target/$task_profile/copy-for-ai-desktop" "$task_bundle/Contents/MacOS/copy-for-ai-desktop"
cat > "$task_bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>local.copyforai.app</string>
  <key>CFBundleName</key><string>Copy for AI</string>
  <key>CFBundleDisplayName</key><string>Copy for AI</string>
  <key>CFBundleExecutable</key><string>copy-for-ai-desktop</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleIconFile</key><string>CopyAI.icns</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
codesign --force --sign - "$task_bundle"
echo "$task_bundle"
