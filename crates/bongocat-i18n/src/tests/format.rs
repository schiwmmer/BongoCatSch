//! Interpolation is the one thing a caller reaches that is not a lookup.

use super::*;

#[test]
fn format_text_interpolation_is_available() {
    assert_eq!(
        format_text(
            "en-US",
            "update.current_version",
            &[("version", "1.2.3".to_string())],
        ),
        "Current version 1.2.3"
    );
    assert_eq!(
        format_text(
            "zh-CN",
            "update.current_version",
            &[("version", "1.2.3".to_string())],
        ),
        "当前版本 1.2.3"
    );
    // A region variant of a shipped language interpolates the same catalog it
    // falls back to, rather than dropping to English.
    assert_eq!(
        format_text(
            "zh-Hant",
            "update.current_version",
            &[("version", "1.2.3".to_string())],
        ),
        "当前版本 1.2.3"
    );
}

/// A count under a thousand is printed exactly, because there is nothing to
/// shorten and the exact figure is the one the user is watching move.
#[test]
fn a_count_under_a_thousand_is_printed_in_full() {
    assert_eq!(compact_number(0), "0");
    assert_eq!(compact_number(7), "7");
    assert_eq!(compact_number(999), "999");
}

/// A count past a thousand is printed in its largest unit, so a growth figure
/// fits beside the bar it fills instead of pushing it off the panel.
#[test]
fn a_count_past_a_thousand_is_printed_in_its_unit() {
    assert_eq!(compact_number(1_000), "1K");
    assert_eq!(compact_number(1_049), "1K");
    assert_eq!(compact_number(1_050), "1.1K");
    assert_eq!(compact_number(54_683), "54.7K");
    assert_eq!(compact_number(317), "317");
    assert_eq!(compact_number(1_500_000), "1.5M");
    assert_eq!(compact_number(2_400_000_000), "2.4B");
    assert_eq!(compact_number(3_000_000_000_000), "3T");
}

/// From a million up the figure is printed to two decimals: a tenth of a
/// million is a hundred thousand, so two counts tens of thousands apart would
/// otherwise read the same. A whole figure keeps no decimals at all.
#[test]
fn a_count_in_the_millions_keeps_two_decimals() {
    assert_eq!(compact_number(1_234_567), "1.23M");
    assert_eq!(compact_number(1_250_000), "1.25M");
    assert_eq!(compact_number(1_005_000), "1.01M");
    assert_eq!(compact_number(1_000_000), "1M");
    assert_eq!(compact_number(1_230_000_000), "1.23B");
    assert_eq!(compact_number(1_230_000_000_000), "1.23T");
}

/// The unit is chosen after the rounding, not before it: a figure that rounds
/// up to four digits reads as the next unit rather than as `1000K`.
#[test]
fn a_count_that_rounds_up_moves_to_the_next_unit() {
    assert_eq!(compact_number(999_999), "1M");
    assert_eq!(compact_number(999_999_999), "1B");
    // Past a trillion there is no larger unit, so the trillions figure is
    // printed without a decimal rather than as a four-digit one.
    assert_eq!(compact_number(9_999_999_999_999), "10T");
}
