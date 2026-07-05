//! Human-readable formatting helpers. Pure and GTK-free so they're unit
//! testable; `now` is injected rather than read from the clock.

use chrono::{DateTime, Datelike, Local, TimeZone};

/// "10h 1m", "10h", "45m", or "<1m" for anything under a minute.
pub fn humanize_secs(secs: i64) -> String {
    if secs < 60 {
        return "<1m".into();
    }
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    match (hours, minutes) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

/// Relative rendering of a unix timestamp against `now`'s local calendar:
/// "never" (epoch 0 / unset), "today", "yesterday", "N days ago",
/// "N weeks ago", "N months ago", then an absolute "Mon YYYY".
pub fn relative_date(epoch_secs: i64, now: DateTime<Local>) -> String {
    if epoch_secs <= 0 {
        return "never".into();
    }
    let Some(then) = now.timezone().timestamp_opt(epoch_secs, 0).single() else {
        return "never".into();
    };
    // Calendar-day difference, not 24-hour windows: reading at 23:50
    // yesterday is "yesterday" even if it was ten minutes ago.
    let days = (now.date_naive() - then.date_naive()).num_days();
    match days {
        i64::MIN..=-1 => "in the future".into(), // clock skew; don't lie confidently
        0 => "today".into(),
        1 => "yesterday".into(),
        2..=13 => format!("{days} days ago"),
        14..=60 => format!("{} weeks ago", days / 7),
        61..=365 => format!("{} months ago", days / 30),
        _ => format!("{} {}", month_abbr(then.month()), then.year()),
    }
}

/// Absolute short date: "Jul 24 2026".
pub fn short_date(date: chrono::NaiveDate) -> String {
    format!(
        "{} {} {}",
        month_abbr(date.month()),
        date.day(),
        date.year()
    )
}

/// Hour-of-day (0..=23) as a friendly clock label: "midnight", "noon",
/// "7 AM", "10 PM".
pub fn hour_label(hour: u32) -> String {
    match hour % 24 {
        0 => "midnight".into(),
        12 => "noon".into(),
        h if h < 12 => format!("{h} AM"),
        h => format!("{} PM", h - 12),
    }
}

fn month_abbr(month: u32) -> &'static str {
    [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(month as usize) - 1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 7, 3, 12, 0, 0).unwrap()
    }

    fn ts(y: i32, m: u32, d: u32, h: u32) -> i64 {
        Local
            .with_ymd_and_hms(y, m, d, h, 0, 0)
            .unwrap()
            .timestamp()
    }

    #[test]
    fn humanize_covers_the_shapes() {
        assert_eq!(humanize_secs(0), "<1m");
        assert_eq!(humanize_secs(59), "<1m");
        assert_eq!(humanize_secs(60), "1m");
        assert_eq!(humanize_secs(3600), "1h");
        assert_eq!(humanize_secs(3661), "1h 1m");
        assert_eq!(humanize_secs(36060), "10h 1m");
    }

    #[test]
    fn hour_label_reads_like_a_clock() {
        assert_eq!(hour_label(0), "midnight");
        assert_eq!(hour_label(7), "7 AM");
        assert_eq!(hour_label(12), "noon");
        assert_eq!(hour_label(22), "10 PM");
    }

    #[test]
    fn relative_date_uses_calendar_days() {
        assert_eq!(relative_date(0, now()), "never");
        assert_eq!(relative_date(ts(2026, 7, 3, 1), now()), "today");
        // Late last night is still "yesterday" even though it's < 24 h ago.
        assert_eq!(relative_date(ts(2026, 7, 2, 23), now()), "yesterday");
        assert_eq!(relative_date(ts(2026, 6, 30, 12), now()), "3 days ago");
        assert_eq!(relative_date(ts(2026, 6, 15, 12), now()), "2 weeks ago");
        assert_eq!(relative_date(ts(2026, 4, 1, 12), now()), "3 months ago");
        assert_eq!(relative_date(ts(2024, 11, 20, 12), now()), "Nov 2024");
    }
}
