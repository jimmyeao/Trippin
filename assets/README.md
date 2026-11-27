# Bundled assets

## selfie_segmentation_landscape.onnx

The camera's person cut-out (`src/segment.rs`): Google's **MediaPipe
Selfie Segmentation** (landscape, 256x144 input), in the ONNX conversion
published at
<https://huggingface.co/onnx-community/mediapipe_selfie_segmentation_landscape>
(`onnx/model.onnx`, downloaded 2026-11-26; SHA-256
`7a0adcfdb1715d3b0ff0f61486d8a181c33f4a208343a5abfccbc6540e872d24`, pinned by
the `bundled_model_is_pinned` test).

Licensed under the Apache License, Version 2.0
(<https://www.apache.org/licenses/LICENSE-2.0>). Copyright Google LLC.
It is used unmodified. It is the stand-in: the cut-out prefers MODNet
(portrait matting, ZHKKKe/MODNet, Apache 2.0), downloaded on first use
from <https://huggingface.co/Xenova/modnet> at a pinned commit and checked
against its SHA-256 (`src/segment.rs`).
