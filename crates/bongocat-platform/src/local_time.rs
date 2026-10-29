//! The machine's current UTC offset.
//!
//! The pomodoro calendar buckets a completion timestamp into a local day, and
//! the offset is what turns an instant into the day a user would write on a
//! calendar. Reading it is a platform call, so it lives here rather than in the
//! domain crate that uses the answer.
//!
//! The value is read once per call and is not cached: a user who crosses a
//! timezone boundary, or a machine whose zone changes, must see the month they
//! are actually in. An offset that is wrong by an hour moves a completion into
//! the wrong day, and a wrong day is a wrong history.

/// The machine's UTC offset in seconds east of Greenwich, at this moment.
///
/// An offset can fall outside ±14 hours, which the zone database allows, so the
/// result is not clamped to whole hours.
pub fn local_utc_offset_seconds() -> i32 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as libc::time_t)
        .unwrap_or(0);
    read_offset(now)
}

#[cfg(unix)]
fn read_offset(at: libc::time_t) -> i32 {
    // SAFETY: `localtime_r` fills a caller-provided `tm` for the given time and
    // returns a pointer to that same storage, or null. The storage outlives the
    // read below, and the returned `tm_gmtoff` is read before it goes out of
    // scope. `localtime_r` is the reentrant form, so it needs no lock and is
    // safe to call from any thread — unlike `localtime`.
    unsafe {
        let mut local: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&at, &mut local).is_null() {
            return 0;
        }
        i32::try_from(local.tm_gmtoff).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offset_is_within_the_range_a_timezone_can_hold() {
        let offset = local_utc_offset_seconds();
        assert!(
            offset.abs() <= 14 * 3600,
            "a UTC offset beyond ±14 hours is not a timezone: {offset}"
        );
    }

    #[test]
    fn the_offset_is_unchanged_within_one_call_sequence() {
        // Two reads a moment apart can only differ if the machine crosses a
        // zone boundary mid-test, which is not something a test can plan for;
        // what this rules out is a read that is not stable at all, such as one
        // that allocates or reads uninitialised memory each time.
        assert_eq!(local_utc_offset_seconds(), local_utc_offset_seconds());
    }
}
