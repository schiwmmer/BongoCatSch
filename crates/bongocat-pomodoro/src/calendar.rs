//! The calendar arithmetic the month view is built on.
//!
//! A local day is the day number in Unix time shifted by the caller's UTC
//! offset, and that shift happens exactly once, where the timestamp enters.
//! Keeping the offset out of every later step is what stops a grid from
//! disagreeing with the counts drawn beside it: a day number is already a
//! local day, so converting it to a date is plain arithmetic.
//!
//! The offset is the caller's, not this crate's: reading the machine's local
//! offset needs a platform call, and the surfaces that own one already know it.
//! A month grid is arithmetic, not a calendar service.
//!
//! The conversion from day number to calendar date is the standard
//! days-from-civil algorithm, chosen over a date library because the only
//! questions asked here are "which month does this timestamp fall in, locally"
//! and "which cells does this month draw".

use std::collections::BTreeMap;

use crate::CalendarTotals;

/// Seconds in a day.
/// Seconds in a local day, so nothing has to write 86_400 a second time.
pub(crate) const SECONDS_PER_DAY: i64 = 86_400;

/// A local calendar day, as the day number in Unix time.
///
/// The number is already offset-adjusted: a value here names a day a user would
/// write on a calendar, so no later call has to know which timezone produced
/// it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CalendarDay(i64);

impl CalendarDay {
    /// The local day a timestamp falls in, for a caller-supplied UTC offset.
    pub fn from_unix(at_unix: i64, utc_offset_seconds: i32) -> Self {
        Self(
            at_unix
                .saturating_add(i64::from(utc_offset_seconds))
                .div_euclid(SECONDS_PER_DAY),
        )
    }

    /// A day from a number that is already a local day number.
    pub const fn from_day_number(day_number: i64) -> Self {
        Self(day_number)
    }

    /// The local day number this day carries.
    pub const fn day_number(self) -> i64 {
        self.0
    }

    /// The calendar date this day falls on.
    ///
    /// No offset argument: the day number already had one applied.
    pub fn to_civil(self) -> CivilDate {
        let (year, month, day) = civil_from_days(self.0);
        CivilDate {
            year,
            month: u32::try_from(month).unwrap_or(1),
            day: u32::try_from(day).unwrap_or(1),
        }
    }
}

/// A calendar date, in local terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CivilDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl CivilDate {
    /// The local day this date is.
    pub fn to_day(self) -> CalendarDay {
        CalendarDay::from_day_number(days_from_civil(self.year, self.month, self.day))
    }
}

/// One rendered month: the cells to lay out, in whole Monday-first weeks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MonthGrid {
    /// Every cell in the grid, including the leading and trailing days of the
    /// neighbouring months, so a fixed-height grid can always be drawn.
    pub cells: Vec<CalendarDay>,
    /// How many cells precede the first of the month, which is what a renderer
    /// needs in order to dim the leading days.
    pub leading_days: usize,
    pub days_in_month: u32,
    pub year: i32,
    pub month: u32,
}

impl MonthGrid {
    /// How many week rows this month occupies.
    pub fn rows(&self) -> usize {
        self.cells.len() / 7
    }
}

/// Lay out one calendar month as a whole number of Monday-first weeks.
///
/// Only whole weeks are produced, so the grid has no ragged last row. A month
/// therefore occupies five rows or six depending on where its first falls, which
/// is what every printed calendar does: padding every month to six would leave
/// an empty week on eleven months of the year.
pub fn month_grid(year: i32, month: u32) -> MonthGrid {
    let month = month.clamp(1, 12);
    let days_in_month = days_in_month(year, month);
    let first = CalendarDay::from_day_number(days_from_civil(year, month, 1));
    // 1970-01-01 was a Thursday, so day number 0 sits at offset 3 of a
    // Monday-first week.
    let leading_days = (first.day_number().rem_euclid(7) as usize + 3) % 7;
    let rows = (leading_days as u32 + days_in_month).div_ceil(7);
    let cells = (0..rows * 7)
        .map(|offset| {
            CalendarDay::from_day_number(
                first.day_number() - leading_days as i64 + i64::from(offset),
            )
        })
        .collect();
    MonthGrid {
        cells,
        leading_days,
        days_in_month,
        year,
        month,
    }
}

/// One day's contribution to a month.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CalendarDayTotal {
    /// Focus sessions finished on this day.
    pub pomodoros: u32,
    /// Distinct tasks those sessions belonged to.
    pub tasks: u32,
}

/// One task's finished sessions on one day, as a calendar cell names it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalendarTaskActivity {
    /// The task's name at the time of the read.
    pub task_name: String,
    /// Focus sessions of this task finished on that day.
    pub pomodoros: u32,
}

/// The month totals a header shows: how many focus sessions were finished and
/// how many distinct tasks they belonged to.
///
/// "Tasks" counts a task once per day it contributed to rather than once per
/// session, so a user who ran ten sessions on one task sees `10` sessions and
/// `1` task for that day — the pair that explains the day rather than two
/// numbers that look like they ought to match.
pub fn month_totals(days: &BTreeMap<CalendarDay, CalendarDayTotal>) -> CalendarTotals {
    let mut totals = CalendarTotals::default();
    for day in days.values() {
        totals.pomodoros = totals.pomodoros.saturating_add(day.pomodoros);
        totals.tasks = totals.tasks.saturating_add(day.tasks);
    }
    totals
}

/// How many days a month has.
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 30,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
///
/// Howard Hinnant's `days_from_civil`: the month is shifted so that March is
/// month one, which puts the leap day at the end of the year where it cannot be
/// skipped.
fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = i64::from(year) - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The inverse of [`days_from_civil`].
fn civil_from_days(days: i64) -> (i32, i32, i32) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = shifted_month + if shifted_month < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (
        i32::try_from(year).unwrap_or(1970),
        i32::try_from(month).unwrap_or(1),
        i32::try_from(day).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const UTC: i32 = 0;
    /// China Standard Time, the offset the surfaces actually pass.
    const CST: i32 = 8 * 3600;

    #[test]
    fn the_epoch_is_a_thursday() {
        // A Monday-first grid therefore starts three cells into the week.
        let day = CalendarDay::from_day_number(0);
        assert_eq!(
            day.to_civil(),
            CivilDate {
                year: 1970,
                month: 1,
                day: 1
            }
        );
    }

    #[test]
    fn a_civil_date_round_trips_through_the_day_number() {
        for (year, month, day) in [
            (1970, 1, 1),
            (2000, 2, 29),
            (2026, 10, 15),
            (2024, 12, 31),
            (1999, 3, 1),
            (1900, 3, 1),
        ] {
            let days = days_from_civil(year, month, day);
            assert_eq!(
                civil_from_days(days),
                (
                    year,
                    i32::try_from(month).expect("a month fits in i32"),
                    i32::try_from(day).expect("a day fits in i32")
                ),
                "{year}-{month}-{day}"
            );
            let civil = CivilDate { year, month, day };
            assert_eq!(civil.to_day().day_number(), days);
        }
    }

    #[test]
    fn a_late_evening_timestamp_lands_on_the_next_local_day() {
        // 2026-01-01T23:00:00Z is already 2026-01-01 07:00 in China, so both
        // readings name the same date but the day numbers differ by the offset.
        let late = 1_767_308_400;
        let utc_day = CalendarDay::from_unix(late, UTC);
        let cst_day = CalendarDay::from_unix(late, CST);
        // An offset shorter than a day can only move a day number by at most
        // one, and a late-evening timestamp east of Greenwich crosses the
        // boundary while the same instant west of it does not.
        assert_eq!(cst_day.day_number() - utc_day.day_number(), 1);
        assert_eq!(
            CalendarDay::from_unix(late, -CST).day_number(),
            utc_day.day_number(),
            "west of Greenwich the same instant stays on its UTC day"
        );
        // A timestamp an hour before UTC midnight reads as the next day east
        // of Greenwich and the previous day west of it.
        let just_before = late - 7_200;
        assert_eq!(
            CalendarDay::from_unix(just_before, UTC).to_civil(),
            CivilDate {
                year: 2026,
                month: 1,
                day: 1
            }
        );
        assert_eq!(
            CalendarDay::from_unix(just_before, CST).to_civil(),
            CivilDate {
                year: 2026,
                month: 1,
                day: 2
            }
        );
        // The local day converts to the local date with no further offset.
        assert_eq!(cst_day.to_civil().year, 2026);
    }

    #[test]
    fn a_timestamp_before_the_epoch_still_names_a_day() {
        // One second before the epoch is 1969-12-31 in UTC and 1970-01-01 in
        // China, which is why the division floors rather than truncates.
        assert_eq!(
            CalendarDay::from_unix(-1, UTC).to_civil(),
            CivilDate {
                year: 1969,
                month: 12,
                day: 31
            }
        );
        assert_eq!(
            CalendarDay::from_unix(-1, CST).to_civil(),
            CivilDate {
                year: 1970,
                month: 1,
                day: 1
            }
        );
    }

    #[test]
    fn a_grid_is_whole_weeks_and_starts_on_the_first_of_the_month() {
        let grid = month_grid(2026, 10);
        assert_eq!(grid.days_in_month, 31);
        assert_eq!(grid.cells.len() % 7, 0);
        assert!(grid.cells.len() >= grid.days_in_month as usize);
        let first_of_month = grid.cells[grid.leading_days];
        assert_eq!(
            first_of_month.to_civil(),
            CivilDate {
                year: 2026,
                month: 10,
                day: 1
            }
        );
    }

    #[test]
    fn a_grid_is_five_or_six_rows_and_never_a_ragged_one() {
        for year in [2025, 2026] {
            for month in 1..=12 {
                let grid = month_grid(year, month);
                assert_eq!(grid.cells.len() % 7, 0, "{year}-{month}");
                assert!(
                    (5..=6).contains(&grid.rows()),
                    "{year}-{month} produced {} rows",
                    grid.rows()
                );
                // Every day of the month has exactly one cell.
                assert_eq!(
                    grid.cells[grid.leading_days..grid.leading_days + grid.days_in_month as usize]
                        .len(),
                    grid.days_in_month as usize
                );
            }
        }
    }

    #[test]
    fn a_grid_starts_on_a_monday_and_ends_on_a_sunday() {
        for month in 1..=12 {
            let grid = month_grid(2026, month);
            // Day number 0 is a Thursday, so a Monday sits at offset 3.
            assert_eq!(grid.cells[0].day_number().rem_euclid(7), 4);
            assert_eq!(
                grid.cells
                    .last()
                    .expect("a last cell")
                    .day_number()
                    .rem_euclid(7),
                3
            );
        }
    }

    #[test]
    fn february_2024_is_a_leap_month() {
        assert_eq!(month_grid(2024, 2).days_in_month, 29);
        assert_eq!(month_grid(2025, 2).days_in_month, 28);
        assert_eq!(month_grid(1900, 2).days_in_month, 28);
        assert_eq!(month_grid(2000, 2).days_in_month, 29);
    }

    #[test]
    fn an_out_of_range_month_is_read_as_a_clamped_one() {
        assert_eq!(month_grid(2026, 13).month, 12);
        assert_eq!(month_grid(2026, 0).month, 1);
    }

    #[test]
    fn the_month_total_counts_sessions_and_distinct_tasks_per_day() {
        let mut days = BTreeMap::new();
        days.insert(
            CalendarDay::from_day_number(20_000),
            CalendarDayTotal {
                pomodoros: 3,
                tasks: 1,
            },
        );
        days.insert(
            CalendarDay::from_day_number(20_001),
            CalendarDayTotal {
                pomodoros: 1,
                tasks: 1,
            },
        );
        let totals = month_totals(&days);
        assert_eq!(totals.pomodoros, 4);
        assert_eq!(totals.tasks, 2);
    }
}
