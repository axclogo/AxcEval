pub(crate) fn parse_date(value: &str) -> Option<i64> {
    if value.len() != 10 || &value[4..5] != "-" || &value[7..8] != "-" {
        return None;
    }
    let year = value[0..4].parse::<i64>().ok()?;
    let month = value[5..7].parse::<u32>().ok()?;
    let day = value[8..10].parse::<u32>().ok()?;
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

// Howard Hinnant's civil calendar algorithm, returning days relative to 1970-01-01.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let adjusted_year = year - i64::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = i64::from(month) + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

pub(crate) fn parse_datetime(value: &str) -> Option<i128> {
    if value.len() < 20 || value.as_bytes().get(10).copied()? != b'T' {
        return None;
    }
    let days = i128::from(parse_date(&value[..10])?);
    let hour = value.get(11..13)?.parse::<i128>().ok()?;
    let minute = value.get(14..16)?.parse::<i128>().ok()?;
    let second = value.get(17..19)?.parse::<i128>().ok()?;
    if value.get(13..14)? != ":" || value.get(16..17)? != ":" || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let mut cursor = 19;
    let mut nanos = 0_i128;
    if value.as_bytes().get(cursor) == Some(&b'.') {
        cursor += 1;
        let start = cursor;
        while value.as_bytes().get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
        }
        let fraction = value.get(start..cursor)?;
        if fraction.is_empty() || fraction.len() > 9 {
            return None;
        }
        nanos = fraction.parse::<i128>().ok()? * 10_i128.pow(9 - fraction.len() as u32);
    }
    let offset_seconds = match value.get(cursor..)? {
        "Z" => 0_i128,
        offset if offset.len() == 6 && matches!(offset.as_bytes()[0], b'+' | b'-') && &offset[3..4] == ":" => {
            let hours = offset[1..3].parse::<i128>().ok()?;
            let minutes = offset[4..6].parse::<i128>().ok()?;
            if hours > 23 || minutes > 59 {
                return None;
            }
            let seconds = hours * 3600 + minutes * 60;
            if offset.as_bytes()[0] == b'-' {
                -seconds
            } else {
                seconds
            }
        }
        _ => return None,
    };
    Some((((days * 86_400 + hour * 3600 + minute * 60 + second) - offset_seconds) * 1_000_000_000) + nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_calendar_dates_and_offsets() {
        assert!(parse_date("2024-02-29").is_some());
        assert!(parse_date("2023-02-29").is_none());
        assert_eq!(
            parse_datetime("2024-01-01T00:00:00Z"),
            parse_datetime("2024-01-01T08:00:00+08:00")
        );
        assert!(parse_datetime("2024-01-01T00:00:00").is_none());
        assert!(parse_date("-001-01-01").is_some());
        assert!(parse_datetime("2024-01-01T00:00:00.5Z").is_some());
        assert!(parse_datetime("2024-01-01T00:00:00-08:00").is_some());
        assert!(parse_datetime("2024-01-01T00:00:00+xx:00").is_none());
        assert!(parse_datetime("2024-01-01T00:00:00+00:xx").is_none());
        assert!(parse_datetime("2024-01-01T00:00:00+00").is_none());
    }
}
