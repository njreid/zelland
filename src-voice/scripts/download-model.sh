#!/usr/bin/env bash
set -euo pipefail

MODEL_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/voice-terminal/models/moonshine-tiny"
REPO="https://huggingface.co/onnx-community/moonshine-tiny-ONNX/resolve/main"

mkdir -p "$MODEL_DIR"

download() {
    local url="$1"
    local dest="$2"
    if [ -f "$dest" ]; then
        echo "Already exists: $dest"
        return
    fi
    echo "Downloading: $url"
    curl -L --progress-bar -o "$dest" "$url"
    echo "Saved: $dest"
}

download "$REPO/onnx/encoder_model.onnx" "$MODEL_DIR/encoder_model.onnx"
download "$REPO/onnx/decoder_model_merged.onnx" "$MODEL_DIR/decoder_model_merged.onnx"
download "$REPO/tokenizer.json" "$MODEL_DIR/tokenizer.json"

echo ""
echo "Models downloaded to: $MODEL_DIR"
echo "Total size:"
du -sh "$MODEL_DIR"
