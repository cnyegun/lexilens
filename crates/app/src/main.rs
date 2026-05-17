use std::sync::mpsc::{Receiver, Sender, channel};

use camera::{CameraFrame, CameraSettings, CameraStatus, CameraStream};
use config::AppConfig;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, StrokeKind, TextureHandle, Vec2};
use ocr::{FakeOcrEngine, OcrEngine, OcrImage};
use patch::{PatchState, PatchStatus};
use selection::{ImagePoint, ImageRect, RectangleSelector};
use tracking::{ImageTracker, TrackingQuality};

const MIN_SELECTION_IMAGE_PIXELS: f32 = 8.0;

fn main() -> eframe::Result<()> {
    let config = AppConfig::from_env_and_args(std::env::args());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("LexiLens Rust Prototype"),
        ..Default::default()
    };

    eframe::run_native(
        "LexiLens Rust Prototype",
        options,
        Box::new(move |creation_context| {
            Ok(Box::new(LexiLensApp::new(creation_context, config.clone())))
        }),
    )
}

struct LexiLensApp {
    config: AppConfig,
    camera: CameraStream,
    latest_frame: Option<CameraFrame>,
    texture: Option<TextureHandle>,
    selector: RectangleSelector,
    tracker: Option<ImageTracker>,
    tracking_quality: TrackingQuality,
    tracking_confidence: f32,
    tracking_points: usize,
    tracking_inliers: usize,
    pointer_was_down: bool,
    patch: Option<PatchState>,
    next_patch_id: u64,
    ocr_runtime: tokio::runtime::Runtime,
    ocr_sender: Sender<OcrUpdate>,
    ocr_receiver: Receiver<OcrUpdate>,
}

#[derive(Debug)]
struct OcrUpdate {
    patch_id: u64,
    result: Result<String, String>,
}

impl LexiLensApp {
    fn new(_creation_context: &eframe::CreationContext<'_>, config: AppConfig) -> Self {
        let camera_settings = CameraSettings::new(
            config.camera.v4l2_device().to_path_buf(),
            config.camera.width,
            config.camera.height,
        );
        let camera = CameraStream::start(camera_settings);
        let (ocr_sender, ocr_receiver) = channel();
        let ocr_runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_time()
            .worker_threads(2)
            .thread_name("lexilens-ocr")
            .build()
            .expect("failed to start OCR runtime");

        Self {
            config,
            camera,
            latest_frame: None,
            texture: None,
            selector: RectangleSelector::default(),
            tracker: None,
            tracking_quality: TrackingQuality::Lost,
            tracking_confidence: 0.0,
            tracking_points: 0,
            tracking_inliers: 0,
            pointer_was_down: false,
            patch: None,
            next_patch_id: 1,
            ocr_runtime,
            ocr_sender,
            ocr_receiver,
        }
    }

    fn receive_camera_frame(&mut self, context: &egui::Context) {
        if let Some(frame) = self.camera.latest_frame() {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [frame.width as usize, frame.height as usize],
                &frame.rgba,
            );

            if let Some(texture) = &mut self.texture {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                self.texture =
                    Some(context.load_texture("camera", image, egui::TextureOptions::LINEAR));
            }

            self.latest_frame = Some(frame);
        }
    }

    fn receive_ocr_updates(&mut self) {
        while let Ok(update) = self.ocr_receiver.try_recv() {
            let Some(patch) = &mut self.patch else {
                continue;
            };

            if patch.id != update.patch_id {
                continue;
            }

            match update.result {
                Ok(text) => patch.set_text(text::clean_ocr_text(&text)),
                Err(message) => patch.set_error(message),
            }
        }
    }

    fn update_tracking(&mut self) {
        let Some(frame) = &self.latest_frame else {
            return;
        };
        let Some(tracker) = &mut self.tracker else {
            return;
        };

        let update = tracker.track(frame.width, frame.height, &frame.rgba);
        self.tracking_quality = update.quality;
        self.tracking_confidence = update.confidence;
        self.tracking_points = update.tracked_points;
        self.tracking_inliers = update.inliers;

        if let Some(patch) = &mut self.patch {
            patch.set_anchor(update.region, update.quad);
        }
    }

    fn handle_pointer(
        &mut self,
        context: &egui::Context,
        image_rect: Rect,
        frame_width: u32,
        frame_height: u32,
    ) {
        let (pointer_down, pointer_pos) =
            context.input(|input| (input.pointer.primary_down(), input.pointer.interact_pos()));

        if pointer_down {
            if let Some(position) = pointer_pos {
                if !self.pointer_was_down && image_rect.contains(position) {
                    self.selector.begin(screen_to_image(
                        position,
                        image_rect,
                        frame_width,
                        frame_height,
                    ));
                } else if self.selector.is_selecting() {
                    self.selector.update(screen_to_image(
                        position,
                        image_rect,
                        frame_width,
                        frame_height,
                    ));
                }
            }
        } else if self.pointer_was_down {
            if let Some(region) = self.selector.finish(MIN_SELECTION_IMAGE_PIXELS) {
                self.start_patch_for_selection(
                    region.clamp_to_image(frame_width, frame_height),
                    frame_width,
                    frame_height,
                );
            }
        }

        self.pointer_was_down = pointer_down;
    }

    fn start_patch_for_selection(
        &mut self,
        region: ImageRect,
        frame_width: u32,
        frame_height: u32,
    ) {
        let patch_id = self.next_patch_id;
        self.next_patch_id += 1;

        self.tracker = None;
        self.tracking_quality = TrackingQuality::Lost;
        self.tracking_confidence = 0.0;
        self.tracking_points = 0;
        self.tracking_inliers = 0;
        self.patch = Some(PatchState::reading(patch_id, region));

        let Some((x, y, width, height)) = region.pixel_bounds(frame_width, frame_height) else {
            if let Some(patch) = &mut self.patch {
                patch.set_error("Selection was empty. Try again.");
            }
            return;
        };

        let Some(frame) = &self.latest_frame else {
            if let Some(patch) = &mut self.patch {
                patch.set_error("No camera frame was available. Try again.");
            }
            return;
        };

        let Some(crop) = frame.crop_rgba(x, y, width, height) else {
            if let Some(patch) = &mut self.patch {
                patch.set_error("Could not crop the selection. Try again.");
            }
            return;
        };

        let tracker = ImageTracker::from_region(frame.width, frame.height, &frame.rgba, region);
        let Some(tracker) = tracker else {
            if let Some(patch) = &mut self.patch {
                patch.set_error("Selected area does not have enough visual features to track.");
            }
            return;
        };

        self.tracker = Some(tracker);
        self.tracking_quality = TrackingQuality::Good;
        self.tracking_confidence = 1.0;
        if let Some(patch) = &mut self.patch {
            patch.set_anchor(region, region.corners());
        }
        let sender = self.ocr_sender.clone();
        self.ocr_runtime.spawn(async move {
            let engine = FakeOcrEngine::default();
            let image = OcrImage {
                width: crop.width,
                height: crop.height,
                rgba: crop.rgba,
            };
            let result = engine
                .recognize(image)
                .await
                .map(|result| result.text)
                .map_err(|error| error.to_string());
            let _ = sender.send(OcrUpdate { patch_id, result });
        });
    }
}

impl eframe::App for LexiLensApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive_camera_frame(context);
        self.update_tracking();
        self.receive_ocr_updates();

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(context, |ui| {
                let available = ui.max_rect();
                let painter = ui.painter_at(available);
                painter.rect_filled(available, 0.0, Color32::BLACK);

                let Some((frame_width, frame_height)) = self
                    .latest_frame
                    .as_ref()
                    .map(|frame| (frame.width, frame.height))
                else {
                    draw_camera_status(&painter, available, &self.camera.status(), &self.config);
                    return;
                };

                let image_rect = fit_rect(available, frame_width as f32, frame_height as f32);

                if let Some(texture) = &self.texture {
                    painter.image(
                        texture.id(),
                        image_rect,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }

                self.handle_pointer(context, image_rect, frame_width, frame_height);

                if let Some(selection_rect) = self.selector.current_rect() {
                    let screen_rect =
                        image_to_screen_rect(selection_rect, image_rect, frame_width, frame_height);
                    painter.rect_stroke(
                        screen_rect,
                        0.0,
                        Stroke::new(2.0, Color32::from_rgb(255, 230, 90)),
                        StrokeKind::Outside,
                    );
                }

                if let Some(patch) = &self.patch {
                    draw_patch(
                        &painter,
                        patch,
                        image_rect,
                        frame_width,
                        frame_height,
                        self.tracking_quality,
                        self.tracking_confidence,
                        self.tracking_points,
                        self.tracking_inliers,
                    );
                }

                draw_camera_status(&painter, available, &self.camera.status(), &self.config);
            });

        context.request_repaint();
    }
}

fn fit_rect(bounds: Rect, content_width: f32, content_height: f32) -> Rect {
    let scale = (bounds.width() / content_width).min(bounds.height() / content_height);
    let size = Vec2::new(content_width * scale, content_height * scale);
    Rect::from_center_size(bounds.center(), size)
}

fn screen_to_image(
    position: Pos2,
    image_rect: Rect,
    image_width: u32,
    image_height: u32,
) -> ImagePoint {
    let x = ((position.x - image_rect.left()) / image_rect.width() * image_width as f32)
        .clamp(0.0, image_width.saturating_sub(1) as f32);
    let y = ((position.y - image_rect.top()) / image_rect.height() * image_height as f32)
        .clamp(0.0, image_height.saturating_sub(1) as f32);

    ImagePoint::new(x, y)
}

fn image_to_screen_rect(
    rect: ImageRect,
    image_rect: Rect,
    image_width: u32,
    image_height: u32,
) -> Rect {
    let left = image_rect.left() + rect.min.x / image_width as f32 * image_rect.width();
    let top = image_rect.top() + rect.min.y / image_height as f32 * image_rect.height();
    let right = image_rect.left() + rect.max.x / image_width as f32 * image_rect.width();
    let bottom = image_rect.top() + rect.max.y / image_height as f32 * image_rect.height();

    Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom))
}

fn draw_patch(
    painter: &egui::Painter,
    patch: &PatchState,
    image_rect: Rect,
    frame_width: u32,
    frame_height: u32,
    tracking_quality: TrackingQuality,
    tracking_confidence: f32,
    tracking_points: usize,
    tracking_inliers: usize,
) {
    let quad = patch
        .quad
        .map(|point| image_to_screen_point(point, image_rect, frame_width, frame_height));
    let patch_rect = rect_from_screen_points(&quad).intersect(image_rect);

    if patch_rect.width() < 4.0 || patch_rect.height() < 4.0 {
        return;
    }

    let background = match patch.status {
        PatchStatus::Reading => Color32::from_rgb(255, 250, 210),
        PatchStatus::TextReady => Color32::from_rgb(250, 250, 250),
        PatchStatus::Error => Color32::from_rgb(255, 225, 225),
    };
    let border = match patch.status {
        PatchStatus::Reading => Color32::from_rgb(230, 170, 40),
        PatchStatus::TextReady => tracking_border_color(tracking_quality),
        PatchStatus::Error => Color32::from_rgb(210, 70, 70),
    };

    painter.add(egui::Shape::convex_polygon(
        quad.to_vec(),
        background,
        Stroke::new(2.0, border),
    ));

    let padding = (patch_rect.height() * 0.08).clamp(2.0, 8.0);
    let text_rect = patch_rect.shrink(padding);
    let (font_size, lines) = fit_text_to_rect(&patch.text, text_rect);
    let line_height = font_size * 1.18;
    let total_height = line_height * lines.len() as f32;
    let mut y = text_rect.center().y - total_height / 2.0;

    for line in lines {
        painter.text(
            Pos2::new(text_rect.center().x, y),
            egui::Align2::CENTER_TOP,
            line,
            FontId::proportional(font_size),
            Color32::from_rgb(20, 20, 20),
        );
        y += line_height;
    }

    if tracking_quality != TrackingQuality::Good {
        let status_size = (patch_rect.height() * 0.16).clamp(8.0, 14.0);
        painter.text(
            Pos2::new(patch_rect.center().x, patch_rect.bottom() - padding),
            egui::Align2::CENTER_BOTTOM,
            tracking_label(
                tracking_quality,
                tracking_confidence,
                tracking_points,
                tracking_inliers,
            ),
            FontId::proportional(status_size),
            Color32::from_rgb(60, 60, 60),
        );
    }
}

fn image_to_screen_point(
    point: ImagePoint,
    image_rect: Rect,
    image_width: u32,
    image_height: u32,
) -> Pos2 {
    Pos2::new(
        image_rect.left() + point.x / image_width as f32 * image_rect.width(),
        image_rect.top() + point.y / image_height as f32 * image_rect.height(),
    )
}

fn rect_from_screen_points(points: &[Pos2; 4]) -> Rect {
    let mut min_x = points[0].x;
    let mut min_y = points[0].y;
    let mut max_x = points[0].x;
    let mut max_y = points[0].y;

    for point in points.iter().skip(1) {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }

    Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y))
}

fn tracking_border_color(quality: TrackingQuality) -> Color32 {
    match quality {
        TrackingQuality::Good => Color32::from_rgb(60, 120, 220),
        TrackingQuality::Weak => Color32::from_rgb(220, 160, 50),
        TrackingQuality::Lost => Color32::from_rgb(210, 70, 70),
    }
}

fn tracking_label(
    quality: TrackingQuality,
    confidence: f32,
    tracked_points: usize,
    inliers: usize,
) -> String {
    match quality {
        TrackingQuality::Good => String::new(),
        TrackingQuality::Weak => format!(
            "Tracking weak {:.0}% ({}/{})",
            confidence * 100.0,
            inliers,
            tracked_points
        ),
        TrackingQuality::Lost => "Tracking lost".to_owned(),
    }
}

fn fit_text_to_rect(text: &str, rect: Rect) -> (f32, Vec<String>) {
    let max_font_size = (rect.height() * 0.68).clamp(7.0, 44.0);
    let mut font_size = max_font_size.floor();

    while font_size >= 7.0 {
        let max_chars = (rect.width() / (font_size * 0.55)).floor().max(1.0) as usize;
        let lines = wrap_text_for_chars(text, max_chars);
        let required_height = lines.len() as f32 * font_size * 1.18;

        if required_height <= rect.height() || font_size <= 7.0 {
            let max_lines = (rect.height() / (font_size * 1.18)).floor().max(1.0) as usize;
            return (font_size, lines.into_iter().take(max_lines).collect());
        }

        font_size -= 1.0;
    }

    (7.0, vec![text.to_owned()])
}

fn wrap_text_for_chars(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();

    for source_line in text.lines() {
        let mut current = String::new();

        for word in source_line.split_whitespace() {
            let next_len = current.len() + usize::from(!current.is_empty()) + word.len();
            if next_len > max_chars && !current.is_empty() {
                lines.push(current);
                current = String::new();
            }

            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push("Reading...".to_owned());
    }

    lines
}

fn draw_camera_status(
    painter: &egui::Painter,
    bounds: Rect,
    status: &CameraStatus,
    config: &AppConfig,
) {
    let text = match status {
        CameraStatus::Starting { device } => format!("Opening camera {}", device.display()),
        CameraStatus::Running {
            device,
            width,
            height,
            format,
        } => format!(
            "Live camera: {}  {}x{}  {}",
            device.display(),
            width,
            height,
            format
        ),
        CameraStatus::Failed { device, message } => format!(
            "Camera failed: {}\n{}\nSet LEXILENS_CAMERA_SOURCE or run with --camera-source /dev/videoN",
            device.display(),
            message
        ),
    };

    let fallback = format!(
        "Configured source: {}  target {}x{}",
        config.camera.v4l2_device().display(),
        config.camera.width,
        config.camera.height
    );

    painter.text(
        bounds.left_top() + Vec2::new(14.0, 14.0),
        egui::Align2::LEFT_TOP,
        format!("{}\n{}", text, fallback),
        FontId::proportional(16.0),
        Color32::WHITE,
    );
}
