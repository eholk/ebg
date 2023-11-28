use std::ops::Range;

use chrono::{DateTime, Local, NaiveDateTime, ParseResult, Utc};
use serde::{Deserialize, Deserializer};
use tracing::trace;

use super::Date;

pub fn deserialize_date<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Date>, D::Error> {
    let s = <Option<&str> as Deserialize>::deserialize(d)?;
    // TODO: support optional timezone information
    let date = s
        .map(date_from_str)
        .transpose()
        .map_err(serde::de::Error::custom)?;
    Ok(date)
}

fn date_from_str(s: &str) -> ParseResult<Date> {
    DateTime::parse_from_rfc3339(s)
        .or_else(|_| DateTime::parse_from_str(s, "%F %T %z"))
        .map(|date| date.with_timezone(&Utc))
        .or_else(|_| {
            NaiveDateTime::parse_from_str(s, "%F %R")
                .map(|date| date.and_local_timezone(Local).unwrap().with_timezone(&Utc))
        })
}

pub fn deserialize_comma_separated_list<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Vec<String>, D::Error> {
    let s = <&str as Deserialize>::deserialize(d)?;
    Ok(s.split(',').map(|s| s.trim().to_string()).collect())
}

const FRONTMATTER_DELIMITER: &str = "---";

/// Finds either the frontmatter delimiter (`---` starting line by itself)
/// and if found returns a range from the index of the start of the delimiter
/// to the index of the first character after the trailing newline.
pub fn find_frontmatter_delimiter(s: &str) -> Option<Range<usize>> {
    let mut start = 0;
    loop {
        trace!("searching for delimiter in {:?}", &s[start..]);
        if s[start..].starts_with(FRONTMATTER_DELIMITER) {
            break;
        }

        start += s[start..].find('\n')? + 1;
    }

    let remainder = &s[(start + FRONTMATTER_DELIMITER.len())..];
    trace!("clearing whitespace in {remainder:?}");
    for (i, c) in remainder.char_indices() {
        if c == '\n' {
            return Some(start..(start + FRONTMATTER_DELIMITER.len() + i + 1));
        }
        if !c.is_whitespace() {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod test {
    use chrono::{FixedOffset, Local, NaiveDate, TimeZone, Utc};
    use miette::IntoDiagnostic;

    use super::{date_from_str, find_frontmatter_delimiter};

    #[test]
    fn parse_date_with_timezone() -> miette::Result<()> {
        let date = "2019-10-13T16:06:57-07:00";
        assert_eq!(
            date_from_str(date).into_diagnostic()?,
            NaiveDate::from_ymd_opt(2019, 10, 13)
                .unwrap()
                .and_hms_opt(16, 6, 57)
                .unwrap()
                .and_local_timezone(FixedOffset::west_opt(7 * 3600).unwrap())
                .unwrap()
        );

        let date = "2016-07-28 20:52:28 -0700";
        assert_eq!(
            date_from_str(date).into_diagnostic()?,
            NaiveDate::from_ymd_opt(2016, 7, 28)
                .unwrap()
                .and_hms_opt(20, 52, 28)
                .unwrap()
                .and_local_timezone(FixedOffset::west_opt(7 * 3600).unwrap())
                .unwrap()
        );

        Ok(())
    }

    #[test]
    fn parse_legacy_date() -> miette::Result<()> {
        let date = "2012-11-27 19:40";
        let expected = Local
            .with_ymd_and_hms(2012, 11, 27, 19, 40, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(date_from_str(date).into_diagnostic()?, expected);
        Ok(())
    }

    #[test]
    fn find_starting_frontmatter_delimiter() {
        assert_eq!(
            find_frontmatter_delimiter("---\n after delimiter"),
            Some(0..4)
        );
    }

    #[test]
    fn find_starting_frontmatter_delimiter_crlf() {
        assert_eq!(
            find_frontmatter_delimiter("---\r\n after delimiter"),
            Some(0..5)
        );
    }

    #[test]
    fn find_middle_frontmatter_delimiter() {
        assert_eq!(
            find_frontmatter_delimiter("before\n---\n after delimiter"),
            Some(7..11)
        );
    }

    #[test]
    fn find_middle_frontmatter_delimiter_crlf() {
        let s = "\r\nbefore\r\n---\r\n after delimiter";
        let delim = find_frontmatter_delimiter(s).unwrap();
        assert_eq!(&s[..(delim.start)], "\r\nbefore\r\n");
        assert_eq!(&s[(delim.end)..], " after delimiter");
    }

    #[test]
    fn find_middle_frontmatter_delimiter_trailing_whitespace() {
        assert_eq!(
            find_frontmatter_delimiter("before\n---   \n after delimiter"),
            Some(7..14)
        );
    }

    #[test]
    fn find_fake_frontmatter_delimiter() {
        assert_eq!(
            find_frontmatter_delimiter("before ---\n after fake delimiter"),
            None
        );
    }

    #[test]
    fn find_no_frontmatter_delimiter() {
        assert_eq!(find_frontmatter_delimiter("before\n after"), None);
    }
}
