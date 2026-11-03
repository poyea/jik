// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

//! Where a window shows in an area, as fractions of the free space in that area.

/// The space between a window and the edge of the work area, at 100% display scale.
pub const SCREEN_MARGIN: f32 = 12.0;

/// `x` and `y` go from 0 (the left or top edge) to 1 (the right or bottom edge) of the free
/// space, that is, the area less the window size and a margin on each side. Thus 0.5 is the
/// center, and the same placement gives the same relative position at all resolutions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    x: f32,
    y: f32,
}

impl Default for Placement {
    fn default() -> Self {
        Self::new(1.0, 1.0)
    }
}

impl Placement {
    /// Values out of 0..=1 are clamped. A value that is not a number becomes 0.5.
    pub fn new(x: f32, y: f32) -> Self {
        let fraction = |v: f32| if v.is_nan() { 0.5 } else { v.clamp(0.0, 1.0) };
        Self {
            x: fraction(x),
            y: fraction(y),
        }
    }

    pub fn x(self) -> f32 {
        self.x
    }

    pub fn y(self) -> f32 {
        self.y
    }

    /// The top-left corner of a window of this size in `area` (left, top, right, bottom).
    /// If the window is larger than the area, its top-left corner stays in the area, so the
    /// start of the content is visible.
    pub fn place(
        self,
        area: (i32, i32, i32, i32),
        width: i32,
        height: i32,
        margin: i32,
    ) -> (i32, i32) {
        let (left, top, right, bottom) = area;
        (
            along(left, right, width, margin, self.x),
            along(top, bottom, height, margin, self.y),
        )
    }

    /// The placement that puts the top-left corner of a window of this size at `(x, y)`.
    pub fn at(
        area: (i32, i32, i32, i32),
        width: i32,
        height: i32,
        margin: i32,
        (x, y): (i32, i32),
    ) -> Self {
        let (left, top, right, bottom) = area;
        Self::new(
            fraction_at(left, right, width, margin, x),
            fraction_at(top, bottom, height, margin, y),
        )
    }
}

fn along(start: i32, end: i32, size: i32, margin: i32, fraction: f32) -> i32 {
    let free = (end - start - 2 * margin - size) as f32;
    (start + margin + (fraction * free).round() as i32).max(start)
}

fn fraction_at(start: i32, end: i32, size: i32, margin: i32, position: i32) -> f32 {
    let free = end - start - 2 * margin - size;
    if free <= 0 {
        return 0.5;
    }
    (position - start - margin) as f32 / free as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: (i32, i32, i32, i32) = (0, 0, 1000, 800);

    #[test]
    fn places_corners_and_center() {
        assert_eq!(Placement::new(0.0, 0.0).place(AREA, 100, 50, 10), (10, 10));
        assert_eq!(
            Placement::new(1.0, 1.0).place(AREA, 100, 50, 10),
            (890, 740)
        );
        assert_eq!(
            Placement::new(0.5, 0.5).place(AREA, 100, 50, 10),
            (450, 375)
        );
        assert_eq!(
            Placement::new(0.5, 1.0).place(AREA, 100, 50, 10),
            (450, 740)
        );
    }

    #[test]
    fn places_between_the_edges() {
        assert_eq!(
            Placement::new(0.25, 0.75).place(AREA, 100, 50, 10),
            (230, 558)
        );
    }

    #[test]
    fn window_taller_than_area_keeps_its_top_visible() {
        assert_eq!(Placement::new(1.0, 1.0).place(AREA, 100, 900, 10), (890, 0));
        assert_eq!(Placement::new(0.5, 0.5).place(AREA, 100, 900, 10), (450, 0));
        assert_eq!(Placement::new(0.0, 0.0).place(AREA, 100, 900, 10), (10, 10));
    }

    #[test]
    fn window_wider_than_area_keeps_its_left_visible() {
        assert_eq!(Placement::new(1.0, 1.0).place(AREA, 1200, 50, 10), (0, 740));
    }

    #[test]
    fn values_are_clamped() {
        assert_eq!(Placement::new(-1.0, 2.0), Placement::new(0.0, 1.0));
        assert_eq!(Placement::new(f32::NAN, 0.3).x(), 0.5);
    }

    #[test]
    fn at_is_the_inverse_of_place() {
        let p = Placement::new(0.25, 0.75);
        let corner = p.place(AREA, 100, 50, 10);
        let back = Placement::at(AREA, 100, 50, 10, corner);
        assert!((back.x() - 0.25).abs() < 0.01 && (back.y() - 0.75).abs() < 0.01);
    }

    #[test]
    fn at_clamps_outside_the_free_space() {
        assert_eq!(
            Placement::at(AREA, 100, 50, 10, (-50, 2000)),
            Placement::new(0.0, 1.0)
        );
        assert_eq!(Placement::at(AREA, 2000, 50, 10, (0, 0)).x(), 0.5);
    }
}
