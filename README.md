# LexiLens

Desktop reading support. Point a webcam at printed text, draw a rectangle, and get OCR, re-rendered text, and read-aloud with tracking.

## Rust Prototype (current)

A native eframe/egui app using Linux V4L2 camera capture, running on a dedicated Tokio runtime for async OCR.

```bash
cargo run -p app
# Override camera device:
LEXILENS_CAMERA_SOURCE=/dev/video0 cargo run -p app
cargo run -p app -- --camera-source /dev/video0
```

The app supports `--camera`, `--camera-width`, `--camera-height` args and corresponding env vars `LEXILENS_CAMERA_SOURCE`, `LEXILENS_CAMERA_WIDTH`, `LEXILENS_CAMERA_HEIGHT`.

### How it works

1. Live camera feed displayed in window
2. Drag a rectangle around printed text
3. Frame is frozen, crop is extracted, feature points are initialised for tracking
4. OCR runs asynchronously (currently `FakeOcrEngine`, ready to swap in a real one)
5. The crop area is tracked frame-to-frame with feature-point matching, patch tracking, RANSAC homography estimation, and quality checks
6. OCR text is rendered onto the tracked quadrilateral, font-fitted to the box
7. If tracking degrades, the status overlay reports "weak" or "lost" instead of silently drifting

### Crate architecture

| Crate | Role |
|---|---|
| `app` | eframe UI, event loop, orchestration |
| `camera` | V4L2 capture (MJPG, YUYV, YU12, RGB3, BGR3), frame rotation, cropping |
| `config` | CLI args + env var config with defaults |
| `ocr` | `OcrEngine` trait + `FakeOcrEngine` (async) |
| `patch` | Patch state machine (reading / text-ready / error) |
| `selection` | `ImagePoint`, `ImageRect`, `RectangleSelector` |
| `text` | OCR text cleaning and segment splitting |
| `tracking` | Feature detection, KLT-style patch tracking, RANSAC homography, quality estimation |

## Legacy Python Prototype (reference only)

The Python app in `src/` and `app.py` is a legacy prototype (OpenCV camera, GLM-OCR via Ollama, Kokoro TTS). It is kept for reference but should not be extended.

See `PLAN.md` for the Rust implementation roadmap.

## Requirements

- Linux with V4L2 camera device
- Rust
