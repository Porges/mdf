use icu_calendar::{
    types::{FormattableMonth, FormattableYear, IsoHour, IsoMinute},
    Gregorian,
};
use serde::Deserialize;

pub enum Date {
    Simple(SimpleDate),
    Range(DateRange),
    Recurring(RecurringDate),
    Approximate(SimpleDate),
    ApproximateRange(DateRange),
}

fn parse_date(value: &str) -> Result<Date, &'static str> {
    let value = value.trim();
    match value.chars().next() {
        Some('+') | Some('-') => Ok(Date::Simple(parse_simple(value)?)),
        Some('R') => Ok(Date::Recurring(parse_recurring(value)?)),
        // => Ok(Date::Range(parse_range(value)?)),
        // Some('A')
        Some(_) => Err("unknown date type"),
        None => Err("empty string"),
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Date;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str(
                    "a GEDCOM X Date (simple, range, recurring, or approximate) as a string",
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                todo!() // Err(de::Error::invalid_value(unexp, exp))
            }
        }

        deserializer.deserialize_str(V)
    }
}

pub enum SimpleDate {
    Y(FormattableYear),
    YM(FormattableYear, FormattableMonth),
    YMD(icu_calendar::Date<Gregorian>),
    YMDH(icu_calendar::Date<Gregorian>, IsoHour),
    YMDHM(icu_calendar::Date<Gregorian>, IsoHour, IsoMinute),
    HMDHMS(icu_calendar::DateTime<Gregorian>),
}

fn parse_simple(value: &str) -> Result<SimpleDate, &'static str> {
    todo!()
}

pub enum DateRange {
    StartEnd(SimpleDate, SimpleDate),
    StartDuration(SimpleDate, iso8601_duration::Duration),
    Start(SimpleDate),
    End(SimpleDate),
}

fn parse_range(value: &str) -> Result<DateRange, &'static str> {
    todo!()
}

pub struct RecurringDate {
    start_date: SimpleDate,
    interval: iso8601_duration::Duration,
    recurrences: Option<u64>,
}

fn parse_recurring(value: &str) -> Result<RecurringDate, &'static str> {
    todo!()
}
