use selection::{ImagePoint, ImageRect};

const MAX_FEATURES: usize = 220;
const MIN_FEATURES: usize = 18;
const MIN_GOOD_INLIERS: usize = 24;
const MIN_WEAK_INLIERS: usize = 12;
const FEATURE_SPACING_PX: f32 = 8.0;
const TRACK_PATCH_RADIUS: i32 = 4;
const TRACK_SEARCH_RADIUS: i32 = 16;
const LOST_SEARCH_RADIUS: i32 = 28;
const MAX_TRACK_ERROR: f32 = 0.26;
const RANSAC_ITERATIONS: usize = 260;
const RANSAC_REPROJECTION_ERROR: f32 = 5.0;
const MIN_CORNER_SCORE: f32 = 1200.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingQuality {
    Good,
    Weak,
    Lost,
}

#[derive(Debug, Clone, Copy)]
pub struct TrackingUpdate {
    pub region: ImageRect,
    pub quad: [ImagePoint; 4],
    pub confidence: f32,
    pub quality: TrackingQuality,
    pub tracked_points: usize,
    pub inliers: usize,
}

#[derive(Debug, Clone)]
pub struct ImageTracker {
    reference_gray: GrayImage,
    previous_gray: GrayImage,
    reference_points: Vec<Point2>,
    previous_points: Vec<Point2>,
    reference_quad: [Point2; 4],
    current_quad: [Point2; 4],
    current_region: ImageRect,
    lost_frames: u32,
}

#[derive(Debug, Clone)]
struct GrayImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
struct Point2 {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Copy)]
struct FeatureCandidate {
    point: Point2,
    score: f32,
}

#[derive(Debug, Clone)]
struct HomographyEstimate {
    h: [f32; 9],
    inliers: Vec<usize>,
    mean_error: f32,
}

impl ImageTracker {
    pub fn from_region(
        frame_width: u32,
        frame_height: u32,
        rgba: &[u8],
        region: ImageRect,
    ) -> Option<Self> {
        let region = region.clamp_to_image(frame_width, frame_height);
        let gray = GrayImage::from_rgba(frame_width, frame_height, rgba);
        let feature_region = expanded_feature_region(region, frame_width, frame_height);
        let reference_points = select_features(&gray, feature_region);

        if reference_points.len() < MIN_FEATURES {
            return None;
        }

        let reference_quad = image_quad_to_points(region.corners());

        Some(Self {
            reference_gray: gray.clone(),
            previous_gray: gray,
            previous_points: reference_points.clone(),
            reference_points,
            reference_quad,
            current_quad: reference_quad,
            current_region: region,
            lost_frames: 0,
        })
    }

    pub fn track(&mut self, frame_width: u32, frame_height: u32, rgba: &[u8]) -> TrackingUpdate {
        let current_gray = GrayImage::from_rgba(frame_width, frame_height, rgba);
        let search_radius = if self.lost_frames == 0 {
            TRACK_SEARCH_RADIUS
        } else {
            LOST_SEARCH_RADIUS
        };

        let mut tracked_reference = Vec::new();
        let mut tracked_current = Vec::new();

        for (reference, previous) in self.reference_points.iter().zip(&self.previous_points) {
            let tracked = if self.lost_frames == 0 {
                track_feature(
                    &self.previous_gray,
                    &current_gray,
                    *previous,
                    *previous,
                    search_radius,
                )
            } else {
                track_feature(
                    &self.reference_gray,
                    &current_gray,
                    *reference,
                    *previous,
                    search_radius,
                )
            };

            let Some((current, error)) = tracked else {
                continue;
            };

            if error <= MAX_TRACK_ERROR {
                tracked_reference.push(*reference);
                tracked_current.push(current);
            }
        }

        if tracked_reference.len() < MIN_WEAK_INLIERS {
            self.lost_frames += 1;
            return self.lost_update(tracked_reference.len(), 0);
        }

        let Some(estimate) = estimate_homography_ransac(&tracked_reference, &tracked_current)
        else {
            self.lost_frames += 1;
            return self.lost_update(tracked_reference.len(), 0);
        };

        let inlier_count = estimate.inliers.len();
        let inlier_ratio = inlier_count as f32 / tracked_reference.len() as f32;
        let quality = if inlier_count >= MIN_GOOD_INLIERS && inlier_ratio >= 0.45 {
            TrackingQuality::Good
        } else if inlier_count >= MIN_WEAK_INLIERS && inlier_ratio >= 0.28 {
            TrackingQuality::Weak
        } else {
            TrackingQuality::Lost
        };

        if quality == TrackingQuality::Lost {
            self.lost_frames += 1;
            return self.lost_update(tracked_reference.len(), inlier_count);
        }

        let quad = transform_quad(&estimate.h, self.reference_quad, frame_width, frame_height);
        if !quad_is_plausible(self.reference_quad, quad, frame_width, frame_height) {
            self.lost_frames += 1;
            return self.lost_update(tracked_reference.len(), inlier_count);
        }

        let inlier_reference = estimate
            .inliers
            .iter()
            .map(|index| tracked_reference[*index])
            .collect::<Vec<_>>();
        let inlier_current = estimate
            .inliers
            .iter()
            .map(|index| tracked_current[*index])
            .collect::<Vec<_>>();

        self.previous_gray = current_gray;
        self.reference_points = inlier_reference;
        self.previous_points = inlier_current;
        self.current_quad = quad;
        self.current_region = region_from_quad(quad, frame_width, frame_height);
        self.lost_frames = 0;

        TrackingUpdate {
            region: self.current_region,
            quad: points_to_image_quad(quad),
            confidence: tracking_confidence(estimate.mean_error, inlier_ratio),
            quality,
            tracked_points: tracked_current.len(),
            inliers: inlier_count,
        }
    }

    fn lost_update(&self, tracked_points: usize, inliers: usize) -> TrackingUpdate {
        TrackingUpdate {
            region: self.current_region,
            quad: points_to_image_quad(self.current_quad),
            confidence: 0.0,
            quality: TrackingQuality::Lost,
            tracked_points,
            inliers,
        }
    }
}

impl GrayImage {
    fn from_rgba(width: u32, height: u32, rgba: &[u8]) -> Self {
        let mut pixels = Vec::with_capacity((width * height) as usize);

        for pixel in rgba.chunks_exact(4).take((width * height) as usize) {
            let luma =
                ((54u16 * pixel[0] as u16 + 183u16 * pixel[1] as u16 + 19u16 * pixel[2] as u16)
                    >> 8) as u8;
            pixels.push(luma);
        }

        Self {
            width,
            height,
            pixels,
        }
    }

    fn pixel(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return 16;
        }

        self.pixels[(y as u32 * self.width + x as u32) as usize]
    }

    fn patch_inside(&self, point: Point2, radius: i32) -> bool {
        let x = point.x.round() as i32;
        let y = point.y.round() as i32;
        x - radius >= 1
            && y - radius >= 1
            && x + radius + 1 < self.width as i32
            && y + radius + 1 < self.height as i32
    }
}

fn expanded_feature_region(region: ImageRect, frame_width: u32, frame_height: u32) -> ImageRect {
    let margin_x = region.width().max(40.0) * 0.65;
    let margin_y = region.height().max(30.0) * 0.65;

    ImageRect {
        min: ImagePoint::new(region.min.x - margin_x, region.min.y - margin_y),
        max: ImagePoint::new(region.max.x + margin_x, region.max.y + margin_y),
    }
    .clamp_to_image(frame_width, frame_height)
}

fn select_features(gray: &GrayImage, region: ImageRect) -> Vec<Point2> {
    let Some((x, y, width, height)) = region.pixel_bounds(gray.width, gray.height) else {
        return Vec::new();
    };

    let border = (TRACK_PATCH_RADIUS + LOST_SEARCH_RADIUS + 2) as u32;
    let x_start = x.saturating_add(border).min(gray.width);
    let y_start = y.saturating_add(border).min(gray.height);
    let x_end = x
        .saturating_add(width)
        .saturating_sub(border)
        .min(gray.width);
    let y_end = y
        .saturating_add(height)
        .saturating_sub(border)
        .min(gray.height);

    if x_end <= x_start || y_end <= y_start {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    for point_y in (y_start..y_end).step_by(2) {
        for point_x in (x_start..x_end).step_by(2) {
            let score = corner_score(gray, point_x as i32, point_y as i32);
            if score >= MIN_CORNER_SCORE {
                candidates.push(FeatureCandidate {
                    point: Point2 {
                        x: point_x as f32,
                        y: point_y as f32,
                    },
                    score,
                });
            }
        }
    }

    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut features = Vec::new();
    let min_distance_sq = FEATURE_SPACING_PX * FEATURE_SPACING_PX;

    for candidate in candidates {
        if features.len() >= MAX_FEATURES {
            break;
        }

        if features.iter().all(|feature: &Point2| {
            let dx = feature.x - candidate.point.x;
            let dy = feature.y - candidate.point.y;
            dx * dx + dy * dy >= min_distance_sq
        }) {
            features.push(candidate.point);
        }
    }

    features
}

fn corner_score(gray: &GrayImage, x: i32, y: i32) -> f32 {
    let mut xx = 0.0;
    let mut xy = 0.0;
    let mut yy = 0.0;

    for dy in -2..=2 {
        for dx in -2..=2 {
            let px = x + dx;
            let py = y + dy;
            let gx = gray.pixel(px + 1, py) as f32 - gray.pixel(px - 1, py) as f32;
            let gy = gray.pixel(px, py + 1) as f32 - gray.pixel(px, py - 1) as f32;
            xx += gx * gx;
            xy += gx * gy;
            yy += gy * gy;
        }
    }

    let trace = xx + yy;
    let determinant = xx * yy - xy * xy;
    let discriminant = (trace * trace - 4.0 * determinant).max(0.0).sqrt();

    0.5 * (trace - discriminant)
}

fn track_feature(
    template: &GrayImage,
    current: &GrayImage,
    template_point: Point2,
    search_center: Point2,
    search_radius: i32,
) -> Option<(Point2, f32)> {
    if !template.patch_inside(template_point, TRACK_PATCH_RADIUS) {
        return None;
    }

    let template_x = template_point.x.round() as i32;
    let template_y = template_point.y.round() as i32;
    let search_x = search_center.x.round() as i32;
    let search_y = search_center.y.round() as i32;
    let mut best_point = None;
    let mut best_error = f32::MAX;

    for y in search_y - search_radius..=search_y + search_radius {
        for x in search_x - search_radius..=search_x + search_radius {
            let candidate = Point2 {
                x: x as f32,
                y: y as f32,
            };

            if !current.patch_inside(candidate, TRACK_PATCH_RADIUS) {
                continue;
            }

            let error = patch_error(template, current, template_x, template_y, x, y);
            if error < best_error {
                best_error = error;
                best_point = Some(candidate);
            }
        }
    }

    best_point.map(|point| (point, best_error))
}

fn patch_error(
    previous: &GrayImage,
    current: &GrayImage,
    previous_x: i32,
    previous_y: i32,
    current_x: i32,
    current_y: i32,
) -> f32 {
    let mut total = 0u32;
    let mut count = 0u32;

    for dy in -TRACK_PATCH_RADIUS..=TRACK_PATCH_RADIUS {
        for dx in -TRACK_PATCH_RADIUS..=TRACK_PATCH_RADIUS {
            let a = previous.pixel(previous_x + dx, previous_y + dy);
            let b = current.pixel(current_x + dx, current_y + dy);
            total += a.abs_diff(b) as u32;
            count += 1;
        }
    }

    total as f32 / count as f32 / 255.0
}

fn estimate_homography_ransac(
    source: &[Point2],
    destination: &[Point2],
) -> Option<HomographyEstimate> {
    if source.len() != destination.len() || source.len() < 4 {
        return None;
    }

    let mut rng = 0x9e37_79b9_7f4a_7c15u64 ^ source.len() as u64;
    let mut best_h = None;
    let mut best_inliers = Vec::new();
    let mut best_error = f32::MAX;

    for _ in 0..RANSAC_ITERATIONS {
        let sample = random_sample_4(source.len(), &mut rng);
        if !sample_is_valid(source, &sample) || !sample_is_valid(destination, &sample) {
            continue;
        }

        let Some(h) = homography_from_indices(source, destination, &sample) else {
            continue;
        };

        let (inliers, mean_error) = homography_inliers(source, destination, &h);
        if inliers.len() > best_inliers.len()
            || (inliers.len() == best_inliers.len() && mean_error < best_error)
        {
            best_h = Some(h);
            best_inliers = inliers;
            best_error = mean_error;
        }
    }

    if best_inliers.len() < 4 {
        return None;
    }

    let h = homography_from_indices(source, destination, &best_inliers).or(best_h)?;
    let (inliers, mean_error) = homography_inliers(source, destination, &h);

    Some(HomographyEstimate {
        h,
        inliers,
        mean_error,
    })
}

fn random_sample_4(len: usize, rng: &mut u64) -> [usize; 4] {
    let mut sample = [0; 4];
    let mut filled = 0;

    while filled < sample.len() {
        *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let index = ((*rng >> 32) as usize) % len;

        if sample[..filled].contains(&index) {
            continue;
        }

        sample[filled] = index;
        filled += 1;
    }

    sample
}

fn sample_is_valid(points: &[Point2], indices: &[usize]) -> bool {
    let mut max_area = 0.0;

    for a in 0..indices.len() {
        for b in a + 1..indices.len() {
            for c in b + 1..indices.len() {
                let area =
                    triangle_area(points[indices[a]], points[indices[b]], points[indices[c]]);
                if area > max_area {
                    max_area = area;
                }
            }
        }
    }

    max_area > 25.0
}

fn homography_from_indices(
    source: &[Point2],
    destination: &[Point2],
    indices: &[usize],
) -> Option<[f32; 9]> {
    let mut ata = [[0.0f32; 8]; 8];
    let mut atb = [0.0f32; 8];

    for index in indices {
        let src = source[*index];
        let dst = destination[*index];
        let row_u = [
            src.x,
            src.y,
            1.0,
            0.0,
            0.0,
            0.0,
            -dst.x * src.x,
            -dst.x * src.y,
        ];
        let row_v = [
            0.0,
            0.0,
            0.0,
            src.x,
            src.y,
            1.0,
            -dst.y * src.x,
            -dst.y * src.y,
        ];
        accumulate_normal_equation(&mut ata, &mut atb, row_u, dst.x);
        accumulate_normal_equation(&mut ata, &mut atb, row_v, dst.y);
    }

    let solution = solve_8x8(ata, atb)?;
    Some([
        solution[0],
        solution[1],
        solution[2],
        solution[3],
        solution[4],
        solution[5],
        solution[6],
        solution[7],
        1.0,
    ])
}

fn accumulate_normal_equation(
    ata: &mut [[f32; 8]; 8],
    atb: &mut [f32; 8],
    row: [f32; 8],
    value: f32,
) {
    for y in 0..8 {
        atb[y] += row[y] * value;
        for x in 0..8 {
            ata[y][x] += row[y] * row[x];
        }
    }
}

fn solve_8x8(mut a: [[f32; 8]; 8], mut b: [f32; 8]) -> Option<[f32; 8]> {
    for pivot in 0..8 {
        let mut best_row = pivot;
        let mut best_value = a[pivot][pivot].abs();

        for row in pivot + 1..8 {
            let value = a[row][pivot].abs();
            if value > best_value {
                best_value = value;
                best_row = row;
            }
        }

        if best_value < 1.0e-6 {
            return None;
        }

        if best_row != pivot {
            a.swap(pivot, best_row);
            b.swap(pivot, best_row);
        }

        let pivot_value = a[pivot][pivot];
        for col in pivot..8 {
            a[pivot][col] /= pivot_value;
        }
        b[pivot] /= pivot_value;

        for row in 0..8 {
            if row == pivot {
                continue;
            }

            let factor = a[row][pivot];
            for col in pivot..8 {
                a[row][col] -= factor * a[pivot][col];
            }
            b[row] -= factor * b[pivot];
        }
    }

    Some(b)
}

fn homography_inliers(
    source: &[Point2],
    destination: &[Point2],
    h: &[f32; 9],
) -> (Vec<usize>, f32) {
    let mut inliers = Vec::new();
    let mut total_error = 0.0;

    for (index, (src, dst)) in source.iter().zip(destination).enumerate() {
        let projected = transform_point(h, *src);
        let error = distance(projected, *dst);
        if error <= RANSAC_REPROJECTION_ERROR {
            inliers.push(index);
            total_error += error;
        }
    }

    let mean_error = if inliers.is_empty() {
        f32::MAX
    } else {
        total_error / inliers.len() as f32
    };

    (inliers, mean_error)
}

fn transform_quad(
    h: &[f32; 9],
    quad: [Point2; 4],
    frame_width: u32,
    frame_height: u32,
) -> [Point2; 4] {
    quad.map(|point| clamp_point(transform_point(h, point), frame_width, frame_height))
}

fn transform_point(h: &[f32; 9], point: Point2) -> Point2 {
    let denominator = h[6] * point.x + h[7] * point.y + h[8];
    if denominator.abs() < 1.0e-6 {
        return point;
    }

    Point2 {
        x: (h[0] * point.x + h[1] * point.y + h[2]) / denominator,
        y: (h[3] * point.x + h[4] * point.y + h[5]) / denominator,
    }
}

fn quad_is_plausible(
    reference_quad: [Point2; 4],
    current_quad: [Point2; 4],
    frame_width: u32,
    frame_height: u32,
) -> bool {
    let reference_area = polygon_area(&reference_quad);
    let current_area = polygon_area(&current_quad);
    if reference_area <= 1.0 || current_area <= 1.0 {
        return false;
    }

    let area_ratio = current_area / reference_area;
    if !(0.20..=5.0).contains(&area_ratio) {
        return false;
    }

    current_quad.iter().all(|point| {
        point.x >= 0.0
            && point.y >= 0.0
            && point.x <= frame_width.saturating_sub(1) as f32
            && point.y <= frame_height.saturating_sub(1) as f32
    })
}

fn region_from_quad(quad: [Point2; 4], frame_width: u32, frame_height: u32) -> ImageRect {
    let image_quad = points_to_image_quad(quad);
    ImageRect::from_points(&image_quad)
        .unwrap_or_else(|| ImageRect::from_min_size(0.0, 0.0, 1.0, 1.0))
        .clamp_to_image(frame_width, frame_height)
}

fn tracking_confidence(mean_error: f32, inlier_ratio: f32) -> f32 {
    let geometry_score = (1.0 - mean_error / RANSAC_REPROJECTION_ERROR).clamp(0.0, 1.0);
    (geometry_score * 0.6 + inlier_ratio.clamp(0.0, 1.0) * 0.4).clamp(0.0, 1.0)
}

fn clamp_point(point: Point2, frame_width: u32, frame_height: u32) -> Point2 {
    Point2 {
        x: point.x.clamp(0.0, frame_width.saturating_sub(1) as f32),
        y: point.y.clamp(0.0, frame_height.saturating_sub(1) as f32),
    }
}

fn image_quad_to_points(quad: [ImagePoint; 4]) -> [Point2; 4] {
    quad.map(|point| Point2 {
        x: point.x,
        y: point.y,
    })
}

fn points_to_image_quad(quad: [Point2; 4]) -> [ImagePoint; 4] {
    quad.map(|point| ImagePoint::new(point.x, point.y))
}

fn triangle_area(a: Point2, b: Point2, c: Point2) -> f32 {
    ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() * 0.5
}

fn polygon_area(points: &[Point2; 4]) -> f32 {
    let mut area = 0.0;

    for index in 0..points.len() {
        let current = points[index];
        let next = points[(index + 1) % points.len()];
        area += current.x * next.y - next.x * current.y;
    }

    area.abs() * 0.5
}

fn distance(a: Point2, b: Point2) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}
