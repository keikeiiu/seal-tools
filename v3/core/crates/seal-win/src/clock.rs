//! The local wall clock, for the per-run text logs.
//!
//! v2's log lines used `DateTime.Now` (`PetTool.Log`, `FileLogger` callers), i.e. LOCAL wall-clock
//! time. Rust's `std` only exposes UTC, so the local clock is one more Win32 call — and this crate is
//! where every Win32 call lives. The result is plain `u16`s so callers (the core's `logs.rs`) never
//! see a `SYSTEMTIME` and the formatting stays testable data.
//!
//! Non-Windows builds fall back to UTC. The suite is Windows-only by design (`AGENTS.md`), but a named
//! fallback keeps the crate buildable elsewhere without a time dependency.

/// `[year, month, day, hour, minute, second, milliseconds]`, LOCAL time.
#[must_use]
pub fn local_now() -> [u16; 7] {
    #[cfg(windows)]
    {
        local_now_windows()
    }
    #[cfg(not(windows))]
    {
        local_now_utc()
    }
}

#[cfg(windows)]
fn local_now_windows() -> [u16; 7] {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;

    let mut time = SYSTEMTIME {
        wYear: 0,
        wMonth: 0,
        wDayOfWeek: 0,
        wDay: 0,
        wHour: 0,
        wMinute: 0,
        wSecond: 0,
        wMilliseconds: 0,
    };
    // SAFETY: `GetLocalTime` takes one `*mut SYSTEMTIME` and writes exactly one `SYSTEMTIME`
    // through it (it returns `()` and has no failure case). `time` is a live, correctly aligned
    // stack value of exactly that type and outlives the call; the API keeps no reference to it and
    // reads nothing else.
    unsafe { GetLocalTime(&raw mut time) };
    [
        time.wYear,
        time.wMonth,
        time.wDay,
        time.wHour,
        time.wMinute,
        time.wSecond,
        time.wMilliseconds,
    ]
}

#[cfg(not(windows))]
fn local_now_utc() -> [u16; 7] {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let seconds_of_day = u32::try_from(secs % 86_400).unwrap_or(0);
    let (year, month, day) = civil_from_days(days);
    [
        u16::try_from(year).unwrap_or(1970),
        u16::try_from(month).unwrap_or(1),
        u16::try_from(day).unwrap_or(1),
        u16::try_from(seconds_of_day / 3600).unwrap_or(0),
        u16::try_from((seconds_of_day % 3600) / 60).unwrap_or(0),
        u16::try_from(seconds_of_day % 60).unwrap_or(0),
        u16::try_from(now.subsec_millis()).unwrap_or(0),
    ]
}

/// Howard Hinnant's days-to-civil conversion, for the non-Windows fallback only.
#[cfg(not(windows))]
fn civil_from_days(days: i64) -> (i32, i32, i32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (
        i32::try_from(year).unwrap_or(1970),
        i32::try_from(month).unwrap_or(1),
        i32::try_from(day).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_local_clock_is_plausible() {
        let [year, month, day, hour, minute, second, millis] = super::local_now();
        assert!((2020..=2100).contains(&year), "year {year}");
        assert!((1..=12).contains(&month), "month {month}");
        assert!((1..=31).contains(&day), "day {day}");
        assert!(hour < 24, "hour {hour}");
        assert!(minute < 60, "minute {minute}");
        assert!(second < 60, "second {second}");
        assert!(millis < 1_000, "milliseconds {millis}");
    }
}
