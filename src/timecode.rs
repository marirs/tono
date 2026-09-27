//! Parsing of user-supplied clip boundaries (`--from` / `--to`).
//!
//! Accepted forms: `SS`, `MM:SS`, `HH:MM:SS`, each optionally with a
//! fractional seconds part (`01:04.250`). Minutes and seconds components
//! after the first must be < 60 so typos like `01:75` fail loudly instead of
//! silently meaning 01:15 of the next minute.

use anyhow::{bail, Context, Result};

pub fn parse_timecode_to_seconds(raw_timecode: &str) -> Result<f64> {
    let trimmed = raw_timecode.trim();
    if trimmed.is_empty() {
        bail!("empty timecode");
    }

    let components: Vec<&str> = trimmed.split(':').collect();
    if components.len() > 3 {
        bail!("timecode `{raw_timecode}` has too many `:` separators (expected HH:MM:SS)");
    }

    let mut total_seconds = 0.0_f64;
    let last_index = components.len() - 1;
    for (index, component) in components.iter().enumerate() {
        let is_seconds_component = index == last_index;
        let value: f64 = if is_seconds_component {
            component
                .parse()
                .with_context(|| format!("invalid seconds `{component}` in `{raw_timecode}`"))?
        } else {
            // Hours and minutes must be whole numbers; fractions only make
            // sense in the trailing seconds field.
            component
                .parse::<u32>()
                .with_context(|| format!("invalid component `{component}` in `{raw_timecode}`"))?
                as f64
        };

        if !value.is_finite() || value < 0.0 {
            bail!("timecode `{raw_timecode}` must be a non-negative number");
        }
        // Every component except the leading one is a base-60 digit.
        if index > 0 && value >= 60.0 {
            bail!("component `{component}` in `{raw_timecode}` must be < 60");
        }

        total_seconds = total_seconds * 60.0 + value;
    }
    Ok(total_seconds)
}

/// Validates an optional clip window and returns `(start, optional_end)`.
pub fn parse_clip_window(
    from_timecode: Option<&str>,
    to_timecode: Option<&str>,
) -> Result<(f64, Option<f64>)> {
    let start_seconds = match from_timecode {
        Some(raw) => parse_timecode_to_seconds(raw).context("--from")?,
        None => 0.0,
    };
    let end_seconds = match to_timecode {
        Some(raw) => Some(parse_timecode_to_seconds(raw).context("--to")?),
        None => None,
    };
    if let Some(end) = end_seconds {
        if end <= start_seconds {
            bail!("--to ({end:.3}s) must be after --from ({start_seconds:.3}s)");
        }
    }
    Ok((start_seconds, end_seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
    }

    #[test]
    fn parses_plain_seconds() {
        assert_close(parse_timecode_to_seconds("42").unwrap(), 42.0);
        assert_close(parse_timecode_to_seconds("7.5").unwrap(), 7.5);
    }

    #[test]
    fn parses_minutes_and_seconds() {
        assert_close(parse_timecode_to_seconds("00:20").unwrap(), 20.0);
        assert_close(parse_timecode_to_seconds("01:04").unwrap(), 64.0);
        assert_close(parse_timecode_to_seconds("01:04.250").unwrap(), 64.25);
    }

    #[test]
    fn parses_hours_minutes_seconds() {
        assert_close(parse_timecode_to_seconds("01:02:03").unwrap(), 3723.0);
        assert_close(parse_timecode_to_seconds(" 00:00:01.5 ").unwrap(), 1.5);
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in ["", "abc", "1:2:3:4", "01:75", "01:60:00", "-5", "1.5:00", "::"] {
            assert!(parse_timecode_to_seconds(bad).is_err(), "accepted `{bad}`");
        }
    }

    #[test]
    fn clip_window_requires_end_after_start() {
        assert!(parse_clip_window(Some("01:00"), Some("00:30")).is_err());
        assert!(parse_clip_window(Some("00:30"), Some("00:30")).is_err());
        let (start, end) = parse_clip_window(Some("00:20"), Some("01:04")).unwrap();
        assert_close(start, 20.0);
        assert_close(end.unwrap(), 64.0);
        assert_eq!(parse_clip_window(None, None).unwrap(), (0.0, None));
    }
}
