use std::env;
use std::path::{Path, PathBuf};

const DEFAULT_V4L2_DEVICE: &str = "/dev/video4";
const DEFAULT_CAMERA_WIDTH: u32 = 1280;
const DEFAULT_CAMERA_HEIGHT: u32 = 720;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub camera: CameraConfig,
}

#[derive(Debug, Clone)]
pub struct CameraConfig {
    pub source: CameraSource,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub enum CameraSource {
    LinuxV4l2 { device: PathBuf },
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            camera: CameraConfig::default(),
        }
    }
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            source: CameraSource::LinuxV4l2 {
                device: PathBuf::from(DEFAULT_V4L2_DEVICE),
            },
            width: DEFAULT_CAMERA_WIDTH,
            height: DEFAULT_CAMERA_HEIGHT,
        }
    }
}

impl CameraConfig {
    pub fn v4l2_device(&self) -> &Path {
        match &self.source {
            CameraSource::LinuxV4l2 { device } => device,
        }
    }
}

impl AppConfig {
    pub fn from_env_and_args(args: impl IntoIterator<Item = String>) -> Self {
        let mut config = Self::default();

        if let Some(device) = env::var("LEXILENS_CAMERA_SOURCE")
            .ok()
            .or_else(|| env::var("LEXILENS_V4L2_DEVICE").ok())
        {
            config.camera.source = CameraSource::LinuxV4l2 {
                device: PathBuf::from(device),
            };
        }

        if let Ok(width) = env::var("LEXILENS_CAMERA_WIDTH") {
            if let Ok(width) = width.parse::<u32>() {
                config.camera.width = width;
            }
        }

        if let Ok(height) = env::var("LEXILENS_CAMERA_HEIGHT") {
            if let Ok(height) = height.parse::<u32>() {
                config.camera.height = height;
            }
        }

        let mut args = args.into_iter();
        let _program = args.next();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--camera" | "--camera-source" | "--v4l2-device" => {
                    if let Some(device) = args.next() {
                        config.camera.source = CameraSource::LinuxV4l2 {
                            device: PathBuf::from(device),
                        };
                    }
                }
                "--camera-width" => {
                    if let Some(width) = args.next().and_then(|value| value.parse::<u32>().ok()) {
                        config.camera.width = width;
                    }
                }
                "--camera-height" => {
                    if let Some(height) = args.next().and_then(|value| value.parse::<u32>().ok()) {
                        config.camera.height = height;
                    }
                }
                _ => {
                    if let Some(device) = arg.strip_prefix("--camera=") {
                        config.camera.source = CameraSource::LinuxV4l2 {
                            device: PathBuf::from(device),
                        };
                    } else if let Some(device) = arg.strip_prefix("--camera-source=") {
                        config.camera.source = CameraSource::LinuxV4l2 {
                            device: PathBuf::from(device),
                        };
                    } else if let Some(device) = arg.strip_prefix("--v4l2-device=") {
                        config.camera.source = CameraSource::LinuxV4l2 {
                            device: PathBuf::from(device),
                        };
                    } else if let Some(width) = arg
                        .strip_prefix("--camera-width=")
                        .and_then(|value| value.parse::<u32>().ok())
                    {
                        config.camera.width = width;
                    } else if let Some(height) = arg
                        .strip_prefix("--camera-height=")
                        .and_then(|value| value.parse::<u32>().ok())
                    {
                        config.camera.height = height;
                    }
                }
            }
        }

        config
    }
}
