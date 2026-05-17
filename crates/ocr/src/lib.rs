use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct OcrImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrResult {
    pub text: String,
}

#[derive(Debug, Error)]
pub enum OcrError {
    #[error("OCR image is empty")]
    EmptyImage,
}

#[async_trait]
pub trait OcrEngine: Send + Sync {
    async fn recognize(&self, image: OcrImage) -> Result<OcrResult, OcrError>;
}

#[derive(Debug, Clone)]
pub struct FakeOcrEngine {
    delay: Duration,
}

impl Default for FakeOcrEngine {
    fn default() -> Self {
        Self {
            delay: Duration::from_millis(900),
        }
    }
}

impl FakeOcrEngine {
    pub fn new(delay: Duration) -> Self {
        Self { delay }
    }
}

#[async_trait]
impl OcrEngine for FakeOcrEngine {
    async fn recognize(&self, image: OcrImage) -> Result<OcrResult, OcrError> {
        if image.width == 0 || image.height == 0 || image.rgba.is_empty() {
            return Err(OcrError::EmptyImage);
        }

        sleep(self.delay).await;

        Ok(OcrResult {
            text: "Sample OCR text from the selected crop.\nThe camera stays live while this patch updates.".to_owned(),
        })
    }
}
