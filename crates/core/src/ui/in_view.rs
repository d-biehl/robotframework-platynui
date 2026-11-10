//! Whether an element lies fully inside the area its ancestors leave visible,
//! the geometry behind `control:IsInView` (spec `in-view-attribute`).

use crate::types::Rect;

/// The part of the screen a node leaves visible for its descendants: the
/// intersection of its own bounds and those of its ancestors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VisibleArea {
    /// No ancestor has narrowed the area.
    Unbounded,
    /// Only this rectangle is visible.
    Within(Rect),
    /// Nothing is visible: an ancestor is hidden, or the ancestors' bounds do
    /// not overlap.
    Nothing,
}

impl VisibleArea {
    /// The area left inside one more ancestor, given what that ancestor
    /// reports as `IsVisible` and `Bounds`. A hidden ancestor leaves nothing.
    /// Missing or empty bounds do not narrow the area.
    #[must_use]
    pub fn narrow(self, visible: Option<bool>, bounds: Option<Rect>) -> Self {
        if visible == Some(false) {
            return Self::Nothing;
        }
        let Some(bounds) = bounds.filter(|bounds| !bounds.is_empty()) else {
            return self;
        };
        match self {
            Self::Unbounded => Self::Within(bounds),
            Self::Within(area) => area.intersection(&bounds).map_or(Self::Nothing, Self::Within),
            Self::Nothing => Self::Nothing,
        }
    }

    /// Whether `bounds` lie inside the area. Edges that coincide count as
    /// inside, and `tolerance` allows the bounds to exceed the area by that
    /// much on each side. Empty bounds are never inside.
    #[must_use]
    pub fn contains(&self, bounds: Rect, tolerance: f64) -> bool {
        if bounds.is_empty() {
            return false;
        }
        match self {
            Self::Unbounded => true,
            Self::Within(area) => {
                bounds.left() >= area.left() - tolerance
                    && bounds.top() >= area.top() - tolerance
                    && bounds.right() <= area.right() + tolerance
                    && bounds.bottom() <= area.bottom() + tolerance
            }
            Self::Nothing => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    const TOLERANCE: f64 = 1.0;

    fn rect(left: f64, top: f64, right: f64, bottom: f64) -> Rect {
        Rect::new(left, top, right - left, bottom - top)
    }

    fn area_of(ancestors: &[(Option<bool>, Option<Rect>)]) -> VisibleArea {
        ancestors.iter().fold(VisibleArea::Unbounded, |area, &(visible, bounds)| area.narrow(visible, bounds))
    }

    /// The ancestors of the tabs of a tab bar too narrow for them, outermost
    /// first: desktop, application (no bounds), dialog, filler, the tab list
    /// at x 100..400.
    fn tab_list_area() -> VisibleArea {
        area_of(&[
            (Some(true), Some(rect(0.0, 0.0, 1920.0, 1080.0))),
            (None, None),
            (Some(true), Some(Rect::new(90.0, 0.0, 320.0, 400.0))),
            (Some(true), Some(Rect::new(95.0, 15.0, 310.0, 380.0))),
            (Some(true), Some(Rect::new(100.0, 20.0, 300.0, 30.0))),
        ])
    }

    #[rstest]
    // Not scrolled: the last two tabs lie right of the tab list.
    #[case::start_tab_1(100.0, 160.0, true)]
    #[case::start_tab_2(160.0, 220.0, true)]
    #[case::start_tab_3(220.0, 280.0, true)]
    #[case::start_tab_4(280.0, 340.0, true)]
    #[case::start_tab_5(340.0, 400.0, true)]
    #[case::start_tab_6(400.0, 460.0, false)]
    #[case::start_tab_7(460.0, 520.0, false)]
    // Scrolled by 50: the first and the sixth tab are cut by the edges.
    #[case::partly_tab_1(50.0, 110.0, false)]
    #[case::partly_tab_2(110.0, 170.0, true)]
    #[case::partly_tab_3(170.0, 230.0, true)]
    #[case::partly_tab_4(230.0, 290.0, true)]
    #[case::partly_tab_5(290.0, 350.0, true)]
    #[case::partly_tab_6(350.0, 410.0, false)]
    #[case::partly_tab_7(410.0, 470.0, false)]
    // Scrolled to the end: the first two tabs lie left of the tab list.
    #[case::end_tab_1(-20.0, 40.0, false)]
    #[case::end_tab_2(40.0, 100.0, false)]
    #[case::end_tab_3(100.0, 160.0, true)]
    #[case::end_tab_4(160.0, 220.0, true)]
    #[case::end_tab_5(220.0, 280.0, true)]
    #[case::end_tab_6(280.0, 340.0, true)]
    #[case::end_tab_7(340.0, 400.0, true)]
    fn a_tab_is_inside_only_when_the_tab_list_holds_it_fully(
        #[case] left: f64,
        #[case] right: f64,
        #[case] inside: bool,
    ) {
        assert_eq!(tab_list_area().contains(rect(left, 20.0, right, 50.0), TOLERANCE), inside);
    }

    #[test]
    fn a_tab_ending_at_the_edge_of_the_tab_list_is_inside() {
        let area = area_of(&[(Some(true), Some(rect(100.0, 20.0, 400.0, 50.0)))]);
        assert!(area.contains(rect(340.0, 20.0, 400.0, 50.0), 0.0));
        assert!(area.contains(rect(100.0, 20.0, 160.0, 50.0), 0.0));
    }

    #[test]
    fn a_hidden_ancestor_with_empty_bounds_leaves_nothing() {
        // Web content of a closed dialog: the dialog and its filler report
        // IsVisible = false and 0x0, the web content plausible bounds.
        let area = area_of(&[
            (Some(true), Some(rect(0.0, 0.0, 1920.0, 1080.0))),
            (Some(false), Some(Rect::new(0.0, 0.0, 0.0, 0.0))),
            (Some(false), Some(Rect::new(0.0, 0.0, 0.0, 0.0))),
            (Some(true), Some(Rect::new(10.0, 100.0, 800.0, 600.0))),
        ]);
        assert_eq!(area, VisibleArea::Nothing);
        assert!(!area.contains(Rect::new(20.0, 110.0, 25.0, 19.0), TOLERANCE));
    }

    #[test]
    fn a_hidden_ancestor_leaves_nothing_whatever_its_bounds() {
        let area = area_of(&[(Some(false), Some(rect(0.0, 0.0, 100.0, 100.0)))]);
        assert_eq!(area, VisibleArea::Nothing);
    }

    #[test]
    fn ancestors_without_bounds_or_with_empty_bounds_do_not_narrow_the_area() {
        let outer = rect(0.0, 0.0, 100.0, 100.0);
        let area = area_of(&[
            (Some(true), Some(outer)),
            (None, None),
            (Some(true), Some(Rect::new(0.0, 0.0, 0.0, 0.0))),
            (None, Some(Rect::new(50.0, 50.0, 0.0, 10.0))),
        ]);
        assert_eq!(area, VisibleArea::Within(outer));
    }

    #[test]
    fn ancestors_that_do_not_overlap_leave_nothing() {
        let area = area_of(&[
            (Some(true), Some(rect(0.0, 0.0, 100.0, 100.0))),
            (Some(true), Some(rect(200.0, 0.0, 300.0, 100.0))),
        ]);
        assert_eq!(area, VisibleArea::Nothing);
    }

    #[test]
    fn an_element_beyond_the_desktop_is_not_inside() {
        // A table column header that extends past the right edge of the desktop.
        let area = area_of(&[
            (Some(true), Some(rect(0.0, 0.0, 1920.0, 1080.0))),
            (Some(true), Some(rect(1400.0, 900.0, 2000.0, 1060.0))),
        ]);
        assert!(!area.contains(rect(1880.0, 950.0, 1990.0, 970.0), TOLERANCE));
        assert!(area.contains(rect(1850.0, 950.0, 1920.0, 970.0), TOLERANCE));
    }

    #[test]
    fn bounds_may_exceed_the_area_by_the_tolerance_and_no_more() {
        // Inline text against its line box: y 39..58 inside y 40..57.
        let area = area_of(&[(Some(true), Some(rect(10.0, 40.0, 140.0, 57.0)))]);
        assert!(area.contains(rect(12.0, 39.0, 40.0, 58.0), TOLERANCE));
        assert!(!area.contains(rect(12.0, 38.0, 40.0, 57.0), TOLERANCE));
        assert!(!area.contains(rect(12.0, 40.0, 40.0, 59.0), TOLERANCE));
    }

    #[test]
    fn empty_bounds_are_never_inside() {
        assert!(!VisibleArea::Unbounded.contains(Rect::new(10.0, 10.0, 0.0, 0.0), TOLERANCE));
        let area = area_of(&[(Some(true), Some(rect(0.0, 0.0, 100.0, 100.0)))]);
        assert!(!area.contains(Rect::new(10.0, 10.0, 0.0, 5.0), TOLERANCE));
    }

    #[test]
    fn nothing_contains_no_bounds() {
        assert!(!VisibleArea::Nothing.contains(Rect::new(0.0, 0.0, 1.0, 1.0), TOLERANCE));
    }
}
