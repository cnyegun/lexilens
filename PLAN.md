# LexiLens Rust Plan

## Fresh Agent Start Here

This repository currently contains a Python desktop prototype. Treat it as legacy reference only.

Do not continue the Python app unless the user explicitly asks for a demo patch. New engineering work should create a Rust workspace and implement the architecture in this file.

First implementation target:

```text
Rust desktop prototype
live camera preview
mouse rectangle selection
immediate tracked patch overlay
async local GLM-OCR through Ollama
Kokoro TTS integration path
```

Start with Milestone 1 and Milestone 2. Do not start with headset SDKs. Do not start with hand tracking. Do not start with full OCR refinement. The first real Rust deliverable is a live camera app where the selected region becomes an immediate patch while the camera keeps running.

Expected first PR shape:

- create `Cargo.toml` workspace
- add crates `app`, `camera`, `selection`, `patch`, `ocr`, `text`, `config`
- open a desktop window
- show live camera frames
- support mouse rectangle selection
- create an in-memory `PatchState` with `Reading...`
- keep rendering while a fake async OCR task sleeps and then returns sample text

Only after that works should the implementation call real Ollama GLM-OCR.

Definition of done for the first Rust slice:

- camera does not freeze after selection
- patch appears immediately
- app remains responsive while the async job runs
- patch text updates when the async job completes
- code has no Python dependency
- module boundaries match this plan

LexiLens should not be a control-heavy OCR app. It should feel like a real-time reading layer over the world.

The correct engineering architecture is:

```text
real-time tracking and patch rendering
+ async OCR and speech jobs
+ stable world/page anchoring
```

The patch must appear immediately. OCR does not need to finish immediately.

## Product Goal

The user looks at printed text, selects it, and sees a readable patch attached to that text through the camera view or headset display.

The patch should:

- appear as soon as the user selects text
- stay attached to the page or text region
- show `Reading...` while OCR runs
- update with readable text when OCR finishes
- read the text aloud
- highlight the current line during speech
- recover gracefully if tracking quality drops

## Core Rule

Do not run OCR in the render loop.

Real-time work:

- camera frame acquisition
- document/page tracking
- patch pose updates
- rendering
- user input

Async work:

- OCR
- text cleanup
- readable layout
- speech synthesis
- line timing

This is the difference between a stable AR system and a slow demo.

## Why Rust

Rust is a good fit because LexiLens needs low-latency camera work, tracking, async jobs, and predictable memory behavior.

Use Rust for:

- camera pipeline
- frame buffers
- tracking state
- patch rendering
- OCR job orchestration
- TTS playback orchestration
- preferences and app state

Do not force every ML model to run inside the main Rust process. Treat heavy models as replaceable inference backends behind clean Rust interfaces.

## System Loops

LexiLens has three independent loops.

### Render Loop

Target: 60 FPS desktop, 90 FPS headset.

Responsibilities:

- draw camera passthrough or camera preview
- draw selection UI
- draw anchored patch
- draw reading highlight
- show tracking state

This loop must never wait on OCR or TTS.

### Tracking Loop

Target: 30-60 FPS.

Responsibilities:

- estimate page or text-plane motion
- update patch transform
- calculate tracking confidence
- request reacquisition when confidence drops

For laptop prototype, use OpenCV homography tracking.

For headset product, use platform spatial anchors plus document-plane tracking.

### AI Job Loop

Target: async, cancellable, observable.

Responsibilities:

- crop selected region
- rectify crop into a front-facing image
- send image to OCR backend
- clean OCR text
- create readable line segments
- generate or play TTS

This loop reports progress back to app state. It does not own UI state.

## End-To-End User Flow

```text
camera starts
live view renders
user selects text region
app captures the selected crop
app creates an anchored patch immediately
patch shows Reading...
camera continues live
tracker keeps patch attached to the page
OCR job runs in background
OCR result updates patch text
TTS starts
highlight moves through patch line by line
user can stop, replay, or select another region
```

## Patch Behavior

The patch should be a real rendered object, not a generated image.

Patch content:

- high-contrast background
- readable text
- dyslexia-friendly font when available
- large font size
- increased line spacing
- current-line highlight
- optional reading ruler

Patch placement:

- anchor to selected text region
- expand near the text if the readable version does not fit over the original paragraph
- keep a visual connection to the source region
- avoid covering the whole page

For dyslexic reading support, a same-size replacement is usually too small. The readable patch should prioritize readability over exact physical size.

## Tracking Strategy

Do not depend on pure optical flow alone. It drifts.

Laptop prototype:

- detect features around the selected region
- track features with optical flow
- estimate homography between selected frame and current frame
- update patch quadrilateral from that homography
- drop confidence when inlier count is low
- ask user to hold the page steady when tracking is lost

Headset product:

- use headset SLAM for world pose
- detect the document plane
- place a world-space anchor on the plane
- track local page features for correction
- keep patch pose in plane coordinates
- reacquire when the page leaves view and returns

Tracking state machine:

```text
unselected
selected_pending_ocr
tracking_good
tracking_weak
tracking_lost
reacquiring
```

Patch states:

```text
empty
reading
text_ready
speaking
tracking_lost
error
```

## OCR Strategy

Do not OCR every frame.

Production OCR should use a two-stage path:

```text
fast OCR -> immediate rough text
GLM-OCR -> corrected high-quality text
```

The fast OCR path gives quick feedback. GLM-OCR improves quality when it finishes.

Current local option:

- GLM-OCR through Ollama
- good quality
- slow on CPU
- acceptable only if the patch appears immediately and shows progress

Production options:

- GPU local GLM-OCR
- edge device GLM-OCR
- cloud OCR when privacy policy allows it
- faster local OCR for first-pass text

Do not hide OCR latency. Show the patch immediately and let users see progress.

## Rust Architecture

Use a Rust workspace.

```text
lexilens-rs/
  crates/
    app/
    camera/
    tracking/
    selection/
    patch/
    ocr/
    tts/
    text/
    config/
    platform/
```

### `app`

Owns app state and state transitions.

Responsibilities:

- start subsystems
- route user input
- create OCR jobs
- update patch state
- coordinate TTS and highlighting

### `camera`

Owns camera input.

Desktop options:

- `nokhwa` for camera capture
- OpenCV capture only if `nokhwa` is not stable enough

Output:

- timestamped frames
- frame dimensions
- pixel format
- frame IDs

### `selection`

Owns user selection.

Desktop input:

- mouse drag rectangle

Headset input:

- gaze ray
- hand ray
- pinch start
- pinch end
- voice command such as `read this`

Output:

- selected region in image coordinates
- selected region in document-plane coordinates when available

### `tracking`

Owns page and patch tracking.

Laptop implementation:

- OpenCV feature detection
- KLT optical flow
- ORB matching when reacquiring
- RANSAC homography

Rust options:

- `opencv` crate for proven CV operations
- custom Rust math for patch transforms

Output:

- patch quadrilateral
- confidence score
- tracking state

### `patch`

Owns readable patch rendering data.

Responsibilities:

- patch geometry
- patch text layout
- colors
- font size
- line spacing
- highlight index
- tracking confidence visualization

Rendering options:

- `wgpu` for GPU rendering
- `winit` for desktop windowing
- `egui` only for debug/settings panels

Do not build the main patch as a webview. Render it natively.

### `ocr`

Owns OCR jobs.

Interface:

```rust
pub trait OcrEngine {
    async fn recognize(&self, image: OcrImage) -> Result<OcrResult, OcrError>;
}
```

Initial engine:

- `OllamaGlmOcrEngine`
- sends selected crop to local `glm-ocr`
- uses `reqwest`
- uses `serde` for request/response JSON

Later engines:

- `FastOcrEngine`
- `CloudOcrEngine`
- `EdgeOcrEngine`

Do not let OCR engine code know about UI widgets.

### `text`

Owns text cleanup and segmentation.

Responsibilities:

- normalize whitespace
- preserve useful line breaks
- split text into readable segments
- prepare line data for highlighting

### `tts`

Owns speech synthesis and audio playback.

Initial clean option:

- Kokoro-82M through ONNX Runtime if model export is stable
- `ort` crate for ONNX inference
- `cpal` for audio output

Pragmatic bridge option:

- Rust app talks to a local Kokoro service
- service boundary is acceptable if it is explicit and productionized

The TTS interface should stay simple:

```rust
pub trait SpeechEngine {
    async fn speak_segments(&self, segments: Vec<TextSegment>) -> Result<(), SpeechError>;
}
```

The app should receive events:

```text
segment_started(index)
segment_finished(index)
speech_finished
speech_failed(error)
```

### `config`

Owns preferences.

Persist:

- font choice
- font size
- line spacing
- high contrast
- auto-read
- TTS voice
- OCR endpoint
- OCR crop size limit

Use:

- `serde`
- `toml` or `json`
- platform config directory

## Rust Runtime Choices

Recommended crates:

- `tokio` for async jobs
- `reqwest` for Ollama HTTP calls
- `serde` and `serde_json` for data exchange
- `opencv` for tracking and homography
- `image` for image encoding and resizing
- `wgpu` for rendering
- `winit` for desktop windowing
- `egui` for debug and settings UI
- `cpal` for audio output
- `tracing` for logs and timing
- `thiserror` for typed errors
- `anyhow` only at app boundaries

Avoid:

- running OCR on the UI thread
- hiding inference latency behind frozen camera frames
- adding many buttons to the main UI
- mixing tracking, OCR, rendering, and TTS in one module
- treating the patch as a screenshot instead of a rendered object

## Performance Budgets

Desktop prototype:

- camera preview under 33 ms per frame
- tracking update under 16-33 ms
- patch render under 16 ms
- OCR job starts under 100 ms after selection
- OCR result ideally under 3 seconds with GPU, acceptable under 10 seconds on CPU
- TTS first audio under 1 second after text is ready

Headset product:

- render loop at headset refresh rate
- tracking update below frame budget
- patch pose jitter low enough to read comfortably
- OCR async with visible progress
- no blocking work in render loop

## Stability Requirements

Tracking must expose confidence.

If confidence is high:

- render patch normally

If confidence is weak:

- soften patch opacity
- show subtle `Hold steady` state

If confidence is lost:

- keep last known patch for a short timeout
- stop moving it if drift is unsafe
- show `Looking for page...`
- reacquire from stored features

Do not let a drifting patch pretend it is correct.

## Privacy Requirements

Default to local processing.

Local components:

- camera frames
- selected crops
- OCR through local Ollama
- TTS through local Kokoro

If cloud OCR is added later:

- make it opt-in
- show what is sent
- never upload full camera frames by default
- send only selected crops
- document retention policy

## Laptop Prototype Milestones

### Milestone 1: Rust Camera And Selection

Build:

- Rust desktop window
- live camera preview
- mouse rectangle selection
- crop extraction
- no OCR yet

Success:

- camera stays smooth while selecting
- crop matches selected text region

### Milestone 2: Immediate Patch Overlay

Build:

- patch appears immediately after selection
- camera resumes live immediately
- patch shows `Reading...`
- patch overlays selected region in 2D

Success:

- no frozen-camera wait
- user sees the app is alive during OCR

### Milestone 3: Local GLM-OCR Job

Build:

- async Ollama GLM-OCR call
- crop resizing before OCR
- OCR timing logs
- result updates patch text

Success:

- OCR never blocks render loop
- patch updates when text is ready

### Milestone 4: 2D Patch Tracking

Build:

- feature tracking around selected region
- homography update
- confidence state
- reacquisition path

Success:

- patch follows page motion on laptop webcam
- patch reports lost tracking instead of drifting badly

### Milestone 5: Kokoro Speech And Highlighting

Build:

- segment-level TTS
- current-line highlight
- stop/replay

Success:

- highlight and speech stay aligned at segment level

### Milestone 6: Headset Feasibility Spike

Test candidate headsets for:

- raw camera frame access
- world-anchored rendering
- hand/gaze input
- spatial anchors
- deployment constraints

Success:

- choose a headset platform based on API access, not hype

## Headset Architecture

Use Rust as the core engine.

Platform shell options:

- Rust OpenXR app when platform allows it
- Unity shell with Rust native plugin when platform tooling demands Unity
- thin C ABI between platform shell and Rust core

Rust core should own:

- state machine
- selection model
- document-plane tracking logic
- OCR job orchestration
- patch layout data
- TTS event model

Platform shell should own:

- headset camera access
- hand/gaze APIs
- world anchors
- final rendering integration

## Platform Selection Rule

Do not pick a headset until these questions are answered:

- Can we access camera frames for OCR?
- Can we render world-anchored overlays?
- Can we use hand and gaze input?
- Can we run or call local ML models?
- Can we persist anchors or reacquire page pose?

If a headset blocks camera frames, it is a poor fit for LexiLens.

## Engineering Principle

Keep the product simple for the user and explicit in the code.

User model:

```text
select text -> patch appears -> LexiLens reads it
```

Engineering model:

```text
selection event -> patch anchor -> async OCR job -> text layout -> TTS events -> highlight updates
```

This is the stable path from laptop prototype to headset product.
