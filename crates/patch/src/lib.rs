use selection::ImageRect;

#[derive(Debug, Clone)]
pub struct PatchState {
    pub id: u64,
    pub region: ImageRect,
    pub quad: [selection::ImagePoint; 4],
    pub status: PatchStatus,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchStatus {
    Reading,
    TextReady,
    Error,
}

impl PatchState {
    pub fn reading(id: u64, region: ImageRect) -> Self {
        Self {
            id,
            region,
            quad: region.corners(),
            status: PatchStatus::Reading,
            text: "Reading...".to_owned(),
        }
    }

    pub fn set_anchor(&mut self, region: ImageRect, quad: [selection::ImagePoint; 4]) {
        self.region = region;
        self.quad = quad;
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.status = PatchStatus::TextReady;
        self.text = text.into();
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.status = PatchStatus::Error;
        self.text = message.into();
    }
}
