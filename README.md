# LexiLens

## Current Status

New development should follow `PLAN.md` and target a Rust implementation.

The Python app in this repository is a legacy prototype. Keep it as a reference for camera selection, local GLM-OCR calls, Kokoro TTS, and readable-panel behavior. Do not extend it unless the task explicitly asks for a Python demo fix.

## Legacy Python Prototype

LexiLens is a dead-simple desktop reading support demo. The user points a laptop webcam at printed text, draws one rectangle around the text, and LexiLens automatically extracts, OCRs, re-renders, and reads it aloud with line highlighting.

This is not a dyslexia treatment or cure. It is reading support and personalized readability.

## Local Camera Setup

Current Fedora camera devices on this laptop:

```text
DroidCam iPhone camera:
  /dev/video4

HP 5MP laptop camera:
  /dev/video0
  /dev/video1
  /dev/video2
  /dev/video3
```

Use OpenCV camera index `4` for DroidCam.

For the legacy Python prototype, instantiate the webcam as:

```python
WebcamController(camera_index=4)
```

## KISS Product Flow

```text
Open app
Draw around printed text
LexiLens freezes the frame
LexiLens crops the selected region
LexiLens runs OCR automatically
LexiLens renders a readable panel automatically
LexiLens reads aloud automatically
Current line highlights while reading
User can Stop, Read Again, or New Selection
```

Main screen controls are intentionally minimal:

- `Stop` / `Read Again` / `Try Again` as one mode-aware primary button
- `New Selection`
- `Settings`

All tuning controls live in `Settings`:

- Font
- Font size
- Line spacing
- High contrast
- Line focus
- Auto-read
- TTS speed

## OCR Strategy

OCR is local GLM-OCR through Ollama. There is no cloud OCR path and no secondary OCR engine.

```text
selected crop -> Ollama model glm-ocr -> readable text
GLM-OCR unavailable/fails -> show Try Again / New Selection
```

Local GLM-OCR setup:

```bash
ollama pull glm-ocr
ollama serve
```

In another terminal, run LexiLens:

```bash
source .venv312/bin/activate
python app.py
```

Optional local GLM overrides:

```bash
export LEXILENS_GLM_OCR_MODEL="glm-ocr"
export LEXILENS_GLM_OCR_ENDPOINT="http://127.0.0.1:11434/api/generate"
export LEXILENS_GLM_OCR_PROMPT="Text Recognition:"
```

Speed knobs for slow CPU-only machines:

```bash
export LEXILENS_OCR_MAX_IMAGE_EDGE="800"
export LEXILENS_OCR_NUM_PREDICT="768"
export LEXILENS_OCR_NUM_CTX="1536"
export LEXILENS_OCR_KEEP_ALIVE="30m"
```

The default keeps the longest crop edge at `960px`, caps generation at `1024` tokens, and keeps the model loaded for `30m`. Ollama will still be slow if `ollama ps` shows `100% CPU`; GPU acceleration is the real fix for large speedups.

## Run

Rust first-slice prototype:

```bash
cargo run -p app
```

The Rust path defaults to the DroidCam V4L2 device at `/dev/video4`. Override it with either:

```bash
LEXILENS_CAMERA_SOURCE=/dev/video0 cargo run -p app
cargo run -p app -- --camera-source /dev/video0
```

The desktop AR path tracks the selected page/text plane with feature points, frame-to-frame local patch tracking, RANSAC homography estimation, and inlier/confidence checks. The patch is drawn on the tracked quadrilateral and its font is fitted to that selected box. If tracking quality drops, the app reports weak/lost tracking rather than silently drifting.

Current limit: this is professional-style 2D planar camera tracking, not headset-grade 3D spatial anchoring. It can attach to a flat page in the camera image, but it does not know real-world depth or camera pose yet.

Legacy Python prototype:

```bash
python3.12 -m venv .venv312
source .venv312/bin/activate
pip install -r requirements.txt
python app.py
```

Python 3.12 is required for the current Kokoro package. Python 3.13+ is not supported by Kokoro yet.

Optional font setup: place `OpenDyslexic-Regular.otf` at `assets/fonts/OpenDyslexic-Regular.otf`. If it is missing, LexiLens quietly falls back to Verdana/Arial.

Kokoro-82M is the only TTS engine. The first run downloads the Kokoro model and the English spaCy voice support package. On Linux, install audio/system prerequisites if your distro does not already have them:

```bash
sudo apt install espeak-ng libportaudio2
```

Optional Kokoro voice overrides:

```bash
export LEXILENS_KOKORO_LANG="a"
export LEXILENS_KOKORO_VOICE="af_heart"
```

## Architecture

- `WebcamController`: opens camera index `0`, emits frames, provides snapshots.
- `FrameViewer`: displays live/frozen camera frames and handles mouse rectangle selection.
- `RectangleSelector`: owns drag state and normalized image-space rectangle.
- `ImageCropper`: clips and extracts the selected crop.
- `OCRService`: sends the selected crop to local GLM-OCR through Ollama.
- `TextPostProcessor`: cleans OCR output and splits it into readable segments.
- `AccessibleTextRenderer`: renders large, spaced, high-readability text with current-line highlighting.
- `TTSController`: reads one segment at a time with local Kokoro-82M speech synthesis.
- `HighlightSynchronizer`: connects TTS segment events to panel highlighting.
- `UserPreferences`: persists hidden settings in `~/.lexilens/preferences.json`.
- `AppController`: orchestrates the state machine and full pipeline.

## Folder Structure

```text
lexilens/
  app.py
  requirements.txt
  README.md
  assets/
    fonts/
      OpenDyslexic-Regular.otf
  src/
    camera/
      webcam_controller.py
    ui/
      main_window.py
      frame_viewer.py
      readable_panel.py
      controls.py
      settings_dialog.py
      image_utils.py
    selection/
      rectangle_selector.py
    vision/
      cropper.py
      ocr_service.py
    reading/
      text_postprocessor.py
      tts_controller.py
      highlight_sync.py
    config/
      preferences.py
    core/
      app_controller.py
```

## State Machine

```text
live
  user draws rectangle -> processing

processing
  OCR success -> ready -> auto-read -> reading
  OCR failure -> failed
  cancel -> live

reading
  segment_started -> highlight line
  stop -> ready
  playback complete -> ready
  new selection -> live

failed
  try again -> processing
  new selection -> live
```

## Pipeline Pseudocode

```text
start webcam
display live frames

on rectangle complete:
  frame = latest webcam snapshot
  freeze frame
  crop = selected image-space rectangle
  send crop to local GLM-OCR through Ollama in a worker thread
  clean OCR text
  split into line/sentence segments
  render readable panel
  if auto-read enabled:
    read segment by segment
    highlight current segment
```

## Demo Script

1. Open LexiLens.
2. Hold a printed page in front of the laptop camera.
3. Draw one rectangle around a paragraph.
4. Say: “No button hunting. After selection, LexiLens handles the rest.”
5. Show the readable panel appearing automatically.
6. Let the voice read while the current line highlights.
7. Press `New Selection` and repeat on another paragraph.
8. Open `Settings` only if judges ask about personalization.

## Pitch

LexiLens turns a laptop webcam into a fast reading support tool for printed text. Instead of making users fight a noisy camera view or a control-heavy OCR app, LexiLens uses one gesture: select the text. It then extracts the crop, uses local GLM-OCR for high-quality transcription, renders the text in a personalized readable panel, and reads it aloud with synchronized highlighting.
