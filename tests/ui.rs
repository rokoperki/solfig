//! Integration tests for the UI formatting helpers.

use ratatui::layout::Rect;
use solfig::ui::{abbrev, centered, format_eta, group_thousands, progress_bar};

#[test]
fn group_thousands_inserts_separators() {
    assert_eq!(group_thousands(0), "0");
    assert_eq!(group_thousands(999), "999");
    assert_eq!(group_thousands(1_000), "1,000");
    assert_eq!(group_thousands(1_234_567), "1,234,567");
}

#[test]
fn abbrev_scales_to_b_m_or_thousands() {
    assert_eq!(abbrev(999), "999");
    assert_eq!(abbrev(12_345), "12,345");
    assert_eq!(abbrev(4_560_000), "4.56M");
    assert_eq!(abbrev(1_230_000_000), "1.23B");
}

#[test]
fn format_eta_picks_largest_unit() {
    assert_eq!(format_eta(0), "0m");
    // ~150 slots * 0.4s = 60s = 1m
    assert_eq!(format_eta(150), "1m");
    // ~10000 slots * 0.4s = 4000s = 1h 6m
    assert_eq!(format_eta(10_000), "1h 6m");
    // ~250000 slots * 0.4s = 100000s = 1d 3h
    assert_eq!(format_eta(250_000), "1d 3h");
}

#[test]
fn progress_bar_fits_width_and_clamps() {
    let bar = progress_bar(0.5, 20);
    // The rendered bar (counting wide block chars as 1) fills the width.
    assert_eq!(bar.chars().count(), 20);
    assert!(bar.contains("50%"));

    // Ratios outside [0,1] are clamped, never overflowing or panicking.
    assert!(progress_bar(1.5, 20).contains("100%"));
    assert!(progress_bar(-1.0, 20).contains("0%"));
}

#[test]
fn centered_clamps_to_area_and_centers() {
    let area = Rect::new(0, 0, 100, 40);
    let r = centered(50, 20, area);
    assert_eq!((r.width, r.height), (50, 20));
    assert_eq!((r.x, r.y), (25, 10));

    // A request larger than the area is clamped to the area.
    let big = centered(200, 80, area);
    assert_eq!((big.width, big.height), (100, 40));
    assert_eq!((big.x, big.y), (0, 0));
}
