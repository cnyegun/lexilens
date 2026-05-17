#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImagePoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageRect {
    pub min: ImagePoint,
    pub max: ImagePoint,
}

#[derive(Debug, Default)]
pub struct RectangleSelector {
    start: Option<ImagePoint>,
    current: Option<ImagePoint>,
}

impl ImagePoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl ImageRect {
    pub fn new(a: ImagePoint, b: ImagePoint) -> Self {
        Self {
            min: ImagePoint::new(a.x.min(b.x), a.y.min(b.y)),
            max: ImagePoint::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    pub fn from_min_size(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            min: ImagePoint::new(x, y),
            max: ImagePoint::new(x + width, y + height),
        }
    }

    pub fn from_points(points: &[ImagePoint]) -> Option<Self> {
        let first = points.first()?;
        let mut min_x = first.x;
        let mut min_y = first.y;
        let mut max_x = first.x;
        let mut max_y = first.y;

        for point in points.iter().skip(1) {
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
        }

        Some(Self {
            min: ImagePoint::new(min_x, min_y),
            max: ImagePoint::new(max_x, max_y),
        })
    }

    pub fn corners(&self) -> [ImagePoint; 4] {
        [
            self.min,
            ImagePoint::new(self.max.x, self.min.y),
            self.max,
            ImagePoint::new(self.min.x, self.max.y),
        ]
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn is_large_enough(&self, min_size: f32) -> bool {
        self.width() >= min_size && self.height() >= min_size
    }

    pub fn clamp_to_image(self, image_width: u32, image_height: u32) -> Self {
        let max_x = image_width.saturating_sub(1) as f32;
        let max_y = image_height.saturating_sub(1) as f32;

        Self {
            min: ImagePoint::new(self.min.x.clamp(0.0, max_x), self.min.y.clamp(0.0, max_y)),
            max: ImagePoint::new(self.max.x.clamp(0.0, max_x), self.max.y.clamp(0.0, max_y)),
        }
    }

    pub fn pixel_bounds(
        &self,
        image_width: u32,
        image_height: u32,
    ) -> Option<(u32, u32, u32, u32)> {
        let rect = self.clamp_to_image(image_width, image_height);
        let x0 = rect.min.x.floor().max(0.0) as u32;
        let y0 = rect.min.y.floor().max(0.0) as u32;
        let x1 = rect.max.x.ceil().min(image_width as f32) as u32;
        let y1 = rect.max.y.ceil().min(image_height as f32) as u32;

        let width = x1.saturating_sub(x0);
        let height = y1.saturating_sub(y0);

        if width == 0 || height == 0 {
            None
        } else {
            Some((x0, y0, width, height))
        }
    }
}

impl RectangleSelector {
    pub fn begin(&mut self, point: ImagePoint) {
        self.start = Some(point);
        self.current = Some(point);
    }

    pub fn update(&mut self, point: ImagePoint) {
        if self.start.is_some() {
            self.current = Some(point);
        }
    }

    pub fn current_rect(&self) -> Option<ImageRect> {
        Some(ImageRect::new(self.start?, self.current?))
    }

    pub fn finish(&mut self, min_size: f32) -> Option<ImageRect> {
        let rect = self.current_rect();
        self.clear();

        rect.filter(|rect| rect.is_large_enough(min_size))
    }

    pub fn clear(&mut self) {
        self.start = None;
        self.current = None;
    }

    pub fn is_selecting(&self) -> bool {
        self.start.is_some()
    }
}
