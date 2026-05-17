use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread;

use image::GenericImageView;
use thiserror::Error;
use v4l::buffer::Type;
use v4l::io::mmap::Stream as MmapStream;
use v4l::io::traits::CaptureStream;
use v4l::video::Capture;
use v4l::{Device, FourCC};

#[derive(Debug, Clone)]
pub struct CameraSettings {
    pub device_path: PathBuf,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct CameraStream {
    receiver: Receiver<CameraFrame>,
    status: Arc<Mutex<CameraStatus>>,
    stop: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct CameraFrame {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct CroppedFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum CameraStatus {
    Starting {
        device: PathBuf,
    },
    Running {
        device: PathBuf,
        width: u32,
        height: u32,
        format: String,
    },
    Failed {
        device: PathBuf,
        message: String,
    },
}

#[derive(Debug, Error)]
enum CameraError {
    #[error("V4L2 error: {0}")]
    V4l(String),
    #[error("image decode error: {0}")]
    Image(String),
    #[error("unsupported V4L2 pixel format {0}")]
    UnsupportedFormat(String),
}

impl CameraSettings {
    pub fn new(device_path: impl Into<PathBuf>, width: u32, height: u32) -> Self {
        Self {
            device_path: device_path.into(),
            width,
            height,
        }
    }
}

impl CameraStream {
    pub fn start(settings: CameraSettings) -> Self {
        let (sender, receiver) = sync_channel(2);
        let status = Arc::new(Mutex::new(CameraStatus::Starting {
            device: settings.device_path.clone(),
        }));
        let stop = Arc::new(AtomicBool::new(false));

        let thread_status = Arc::clone(&status);
        let thread_stop = Arc::clone(&stop);

        thread::spawn(move || {
            if let Err(error) = capture_loop(
                settings.clone(),
                sender,
                Arc::clone(&thread_status),
                thread_stop,
            ) {
                let mut status = thread_status.lock().expect("camera status lock poisoned");
                *status = CameraStatus::Failed {
                    device: settings.device_path,
                    message: error.to_string(),
                };
            }
        });

        Self {
            receiver,
            status,
            stop,
        }
    }

    pub fn latest_frame(&self) -> Option<CameraFrame> {
        let mut latest = None;
        while let Ok(frame) = self.receiver.try_recv() {
            latest = Some(frame);
        }
        latest
    }

    pub fn status(&self) -> CameraStatus {
        self.status
            .lock()
            .expect("camera status lock poisoned")
            .clone()
    }
}

impl Drop for CameraStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl CameraFrame {
    pub fn rotate_90_clockwise(self) -> Self {
        if self.width == 0 || self.height == 0 || self.rgba.is_empty() {
            return self;
        }

        let source_width = self.width as usize;
        let source_height = self.height as usize;
        let target_width = source_height;
        let target_height = source_width;
        let mut rotated = vec![0; self.rgba.len()];

        for source_y in 0..source_height {
            for source_x in 0..source_width {
                let target_x = source_height - 1 - source_y;
                let target_y = source_x;
                let source_index = (source_y * source_width + source_x) * 4;
                let target_index = (target_y * target_width + target_x) * 4;
                rotated[target_index..target_index + 4]
                    .copy_from_slice(&self.rgba[source_index..source_index + 4]);
            }
        }

        Self {
            id: self.id,
            width: target_width as u32,
            height: target_height as u32,
            rgba: rotated,
        }
    }

    pub fn crop_rgba(&self, x: u32, y: u32, width: u32, height: u32) -> Option<CroppedFrame> {
        let x0 = x.min(self.width);
        let y0 = y.min(self.height);
        let x1 = x0.saturating_add(width).min(self.width);
        let y1 = y0.saturating_add(height).min(self.height);

        let crop_width = x1.saturating_sub(x0);
        let crop_height = y1.saturating_sub(y0);

        if crop_width == 0 || crop_height == 0 {
            return None;
        }

        let mut rgba = Vec::with_capacity((crop_width * crop_height * 4) as usize);
        let stride = (self.width * 4) as usize;
        let row_start = (x0 * 4) as usize;
        let row_end = (x1 * 4) as usize;

        for row in y0..y1 {
            let start = (row as usize * stride) + row_start;
            let end = (row as usize * stride) + row_end;
            rgba.extend_from_slice(&self.rgba[start..end]);
        }

        Some(CroppedFrame {
            width: crop_width,
            height: crop_height,
            rgba,
        })
    }
}

fn capture_loop(
    settings: CameraSettings,
    sender: SyncSender<CameraFrame>,
    status: Arc<Mutex<CameraStatus>>,
    stop: Arc<AtomicBool>,
) -> Result<(), CameraError> {
    let mut device = Device::with_path(&settings.device_path)
        .map_err(|error| CameraError::V4l(error.to_string()))?;
    let format = configure_format(&mut device, settings.width, settings.height)?;

    {
        let mut status = status.lock().expect("camera status lock poisoned");
        *status = CameraStatus::Running {
            device: settings.device_path.clone(),
            width: format.width,
            height: format.height,
            format: fourcc_label(format.fourcc),
        };
    }

    let mut stream = MmapStream::with_buffers(&mut device, Type::VideoCapture, 4)
        .map_err(|error| CameraError::V4l(error.to_string()))?;
    let mut frame_id = 0;

    while !stop.load(Ordering::Relaxed) {
        let (data, _meta) = stream
            .next()
            .map_err(|error| CameraError::V4l(error.to_string()))?;
        let frame = decode_frame(frame_id, data, format.width, format.height, format.fourcc)?
            .rotate_90_clockwise();
        frame_id += 1;

        match sender.try_send(frame) {
            Ok(()) => {}
            Err(TrySendError::Full(_frame)) => {}
            Err(TrySendError::Disconnected(_frame)) => break,
        }
    }

    Ok(())
}

fn configure_format(
    device: &mut Device,
    width: u32,
    height: u32,
) -> Result<v4l::Format, CameraError> {
    let requested = [
        FourCC::new(b"MJPG"),
        FourCC::new(b"YUYV"),
        FourCC::new(b"YU12"),
        FourCC::new(b"RGB3"),
        FourCC::new(b"BGR3"),
    ];

    for fourcc in requested {
        let mut format = device
            .format()
            .map_err(|error| CameraError::V4l(error.to_string()))?;
        format.width = width;
        format.height = height;
        format.fourcc = fourcc;

        if let Ok(format) = device.set_format(&format) {
            if is_supported_format(format.fourcc) {
                return Ok(format);
            }
        }
    }

    let format = device
        .format()
        .map_err(|error| CameraError::V4l(error.to_string()))?;

    if is_supported_format(format.fourcc) {
        Ok(format)
    } else {
        Err(CameraError::UnsupportedFormat(fourcc_label(format.fourcc)))
    }
}

fn decode_frame(
    id: u64,
    data: &[u8],
    width: u32,
    height: u32,
    fourcc: FourCC,
) -> Result<CameraFrame, CameraError> {
    if fourcc == FourCC::new(b"MJPG") {
        return decode_mjpeg(id, data);
    }

    let rgba = if fourcc == FourCC::new(b"YUYV") {
        yuyv_to_rgba(data, width, height)
    } else if fourcc == FourCC::new(b"YU12") {
        yu12_to_rgba(data, width, height)
    } else if fourcc == FourCC::new(b"RGB3") {
        rgb_to_rgba(data, width, height)
    } else if fourcc == FourCC::new(b"BGR3") {
        bgr_to_rgba(data, width, height)
    } else {
        return Err(CameraError::UnsupportedFormat(fourcc_label(fourcc)));
    };

    Ok(CameraFrame {
        id,
        width,
        height,
        rgba,
    })
}

fn decode_mjpeg(id: u64, data: &[u8]) -> Result<CameraFrame, CameraError> {
    let image =
        image::load_from_memory(data).map_err(|error| CameraError::Image(error.to_string()))?;
    let (width, height) = image.dimensions();
    let rgba = image.to_rgba8().into_raw();

    Ok(CameraFrame {
        id,
        width,
        height,
        rgba,
    })
}

fn yuyv_to_rgba(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);

    for chunk in data.chunks_exact(4).take((width * height / 2) as usize) {
        let y0 = chunk[0];
        let u = chunk[1];
        let y1 = chunk[2];
        let v = chunk[3];

        push_yuv_pixel(&mut rgba, y0, u, v);
        push_yuv_pixel(&mut rgba, y1, u, v);
    }

    rgba
}

fn yu12_to_rgba(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let width = width as usize;
    let height = height as usize;
    let luma_len = width * height;
    let chroma_width = width.div_ceil(2);
    let chroma_height = height.div_ceil(2);
    let chroma_len = chroma_width * chroma_height;
    let u_start = luma_len;
    let v_start = u_start + chroma_len;
    let mut rgba = Vec::with_capacity(width * height * 4);

    for y in 0..height {
        for x in 0..width {
            let y_sample = data.get(y * width + x).copied().unwrap_or(16);
            let chroma_index = (y / 2) * chroma_width + (x / 2);
            let u = data.get(u_start + chroma_index).copied().unwrap_or(128);
            let v = data.get(v_start + chroma_index).copied().unwrap_or(128);
            push_yuv_pixel(&mut rgba, y_sample, u, v);
        }
    }

    rgba
}

fn rgb_to_rgba(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);

    for pixel in data.chunks_exact(3).take((width * height) as usize) {
        rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
    }

    rgba
}

fn bgr_to_rgba(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);

    for pixel in data.chunks_exact(3).take((width * height) as usize) {
        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
    }

    rgba
}

fn push_yuv_pixel(out: &mut Vec<u8>, y: u8, u: u8, v: u8) {
    let c = y as i32 - 16;
    let d = u as i32 - 128;
    let e = v as i32 - 128;

    let r = ((298 * c + 409 * e + 128) >> 8).clamp(0, 255) as u8;
    let g = ((298 * c - 100 * d - 208 * e + 128) >> 8).clamp(0, 255) as u8;
    let b = ((298 * c + 516 * d + 128) >> 8).clamp(0, 255) as u8;

    out.extend_from_slice(&[r, g, b, 255]);
}

fn is_supported_format(fourcc: FourCC) -> bool {
    fourcc == FourCC::new(b"MJPG")
        || fourcc == FourCC::new(b"YUYV")
        || fourcc == FourCC::new(b"YU12")
        || fourcc == FourCC::new(b"RGB3")
        || fourcc == FourCC::new(b"BGR3")
}

fn fourcc_label(fourcc: FourCC) -> String {
    fourcc.to_string()
}
