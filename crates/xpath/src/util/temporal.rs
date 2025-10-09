//! Shared temporal parsing utilities for xs:date, xs:time, xs:dateTime.
//! Goals:
//! * Single lexical implementation (avoid drift between evaluator & functions)
//! * Fractional seconds up to 9 digits (truncate beyond)
//! * Enforce seconds < 60, minutes < 60, hours 0–23
//! * Support optional timezone (Z or ±HH:MM) with bounds: HH <= 14, MM < 60
//! * Support negative years (no year 0) – XML Schema maps year 0 lexical to invalid
//! * Return rich but simple error classification for mapping to FORG0001
use chrono::{FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Offset};

#[derive(Debug, Clone, Copy)]
pub enum TemporalErr {
    Lexical,
    Range,
}

fn parse_year(s: &str) -> Result<i32, TemporalErr> {
    if s.is_empty() {
        return Err(TemporalErr::Lexical);
    }
    let negative = s.starts_with('-');
    let digits = if negative { &s[1..] } else { s };
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(TemporalErr::Lexical);
    }
    // Allow more than 4 digits (XML Schema allows extended year representation)
    let mut y: i64 = digits.parse().map_err(|_| TemporalErr::Lexical)?;
    if y == 0 {
        return Err(TemporalErr::Range);
    }
    if negative {
        y = -y;
    }
    // i32 range check
    if y < i32::MIN as i64 || y > i32::MAX as i64 {
        return Err(TemporalErr::Range);
    }
    Ok(y as i32)
}

fn parse_fraction(frac: &str) -> Result<u32, TemporalErr> {
    if frac.is_empty() {
        return Err(TemporalErr::Lexical);
    }
    let capped = if frac.len() > 9 { &frac[..9] } else { frac };
    let v: u32 = capped.parse().map_err(|_| TemporalErr::Lexical)?;
    Ok(v * 10u32.pow(9 - capped.len() as u32))
}

fn split_tz(s: &str) -> (&str, Option<&str>) {
    if let Some(stripped) = s.strip_suffix('Z') {
        return (stripped, Some("Z"));
    }
    if let Some(pos) = s.rfind(['+', '-'])
        && s.len() - pos == 6
    {
        return (&s[..pos], Some(&s[pos..]));
    }
    (s, None)
}

fn parse_tz(tz: &str) -> Result<FixedOffset, TemporalErr> {
    if tz == "Z" {
        // UTC zero offset
        return Ok(chrono::Utc.fix());
    }
    if tz.len() != 6 {
        return Err(TemporalErr::Lexical);
    }
    let sign = &tz[0..1];
    let hh: i32 = tz[1..3].parse().map_err(|_| TemporalErr::Lexical)?;
    let mm: i32 = tz[4..6].parse().map_err(|_| TemporalErr::Lexical)?;
    if hh > 14 || mm > 59 {
        return Err(TemporalErr::Range);
    }
    let secs = hh * 3600 + mm * 60;
    match sign {
        "+" => FixedOffset::east_opt(secs),
        "-" => FixedOffset::west_opt(secs),
        _ => None,
    }
    .ok_or(TemporalErr::Range)
}

pub fn parse_date_lex(s: &str) -> Result<(NaiveDate, Option<FixedOffset>), TemporalErr> {
    let (main, tz_opt) = split_tz(s);
    let (neg, body) = if let Some(stripped) = main.strip_prefix('-') { (true, stripped) } else { (false, main) };
    let parts: Vec<&str> = body.split('-').collect();
    if parts.len() != 3 {
        return Err(TemporalErr::Lexical);
    }
    let mut year_str = parts[0].to_string();
    if neg {
        year_str.insert(0, '-');
    }
    let y = parse_year(&year_str)?;
    let m: u32 = parts[1].parse().map_err(|_| TemporalErr::Lexical)?;
    let d: u32 = parts[2].parse().map_err(|_| TemporalErr::Lexical)?;
    let date = NaiveDate::from_ymd_opt(y, m, d).ok_or(TemporalErr::Range)?;
    let tz = match tz_opt {
        Some(t) => Some(parse_tz(t)?),
        None => None,
    };
    Ok((date, tz))
}

pub fn parse_time_lex(s: &str) -> Result<(NaiveTime, Option<FixedOffset>), TemporalErr> {
    let (main, tz_opt) = split_tz(s);
    let comps: Vec<&str> = main.split(':').collect();
    if comps.len() != 3 {
        return Err(TemporalErr::Lexical);
    }
    let h: u32 = comps[0].parse().map_err(|_| TemporalErr::Lexical)?;
    let m: u32 = comps[1].parse().map_err(|_| TemporalErr::Lexical)?;
    let mut sec_iter = comps[2].splitn(2, '.');
    let s_whole_str = sec_iter.next().ok_or(TemporalErr::Lexical)?;
    if s_whole_str.is_empty() {
        return Err(TemporalErr::Lexical);
    }
    let s_whole: u32 = s_whole_str.parse().map_err(|_| TemporalErr::Lexical)?;
    if h > 23 || m > 59 || s_whole >= 60 {
        return Err(TemporalErr::Range);
    }
    let frac_opt = sec_iter.next();
    let nanos = if let Some(f) = frac_opt { parse_fraction(f)? } else { 0 };
    let time =
        NaiveTime::from_num_seconds_from_midnight_opt(h * 3600 + m * 60 + s_whole, nanos).ok_or(TemporalErr::Range)?;
    let tz = match tz_opt {
        Some(t) => Some(parse_tz(t)?),
        None => None,
    };
    Ok((time, tz))
}

pub fn parse_date_time_lex(s: &str) -> Result<(NaiveDate, NaiveTime, Option<FixedOffset>), TemporalErr> {
    let (main, tz_opt) = split_tz(s);
    let split: Vec<&str> = main.split('T').collect();
    if split.len() != 2 {
        return Err(TemporalErr::Lexical);
    }
    let (date, _) = parse_date_lex(split[0])?; // timezone determined later from full string
    let (time, tz_from_time) = parse_time_lex(split[1])?;
    // Prefer outer timezone (if any) because time parser consumed only local part when tz present in full string
    let tz = match tz_opt {
        Some(t) => Some(parse_tz(t)?),
        None => tz_from_time,
    };
    Ok((date, time, tz))
}

pub fn parse_g_year(s: &str) -> Result<(i32, Option<FixedOffset>), TemporalErr> {
    let (main, tz_opt) = split_tz(s);
    if main.is_empty() {
        return Err(TemporalErr::Lexical);
    }
    let year = parse_year(main)?;
    let tz = match tz_opt {
        Some(t) => Some(parse_tz(t)?),
        None => None,
    };
    Ok((year, tz))
}

pub fn parse_g_year_month(s: &str) -> Result<(i32, u8, Option<FixedOffset>), TemporalErr> {
    let (main, tz_opt) = split_tz(s);
    let (neg, body) = if let Some(stripped) = main.strip_prefix('-') { (true, stripped) } else { (false, main) };
    let parts: Vec<&str> = body.split('-').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(TemporalErr::Lexical);
    }
    let mut year_str = parts[0].to_string();
    if neg {
        year_str.insert(0, '-');
    }
    let year = parse_year(&year_str)?;
    let month: u32 = parts[1].parse().map_err(|_| TemporalErr::Lexical)?;
    if !(1..=12).contains(&month) {
        return Err(TemporalErr::Range);
    }
    let tz = match tz_opt {
        Some(t) => Some(parse_tz(t)?),
        None => None,
    };
    Ok((year, month as u8, tz))
}

pub fn parse_g_month(s: &str) -> Result<(u8, Option<FixedOffset>), TemporalErr> {
    let (main, tz_opt) = split_tz(s);
    if !main.starts_with("--") {
        return Err(TemporalErr::Lexical);
    }
    let month_str = &main[2..];
    if month_str.len() != 2 || !month_str.chars().all(|c| c.is_ascii_digit()) {
        return Err(TemporalErr::Lexical);
    }
    let month: u32 = month_str.parse().map_err(|_| TemporalErr::Lexical)?;
    if !(1..=12).contains(&month) {
        return Err(TemporalErr::Range);
    }
    let tz = match tz_opt {
        Some(t) => Some(parse_tz(t)?),
        None => None,
    };
    Ok((month as u8, tz))
}

pub fn parse_g_month_day(s: &str) -> Result<(u8, u8, Option<FixedOffset>), TemporalErr> {
    let (main, tz_opt) = split_tz(s);
    if !main.starts_with("--") {
        return Err(TemporalErr::Lexical);
    }
    let body = &main[2..];
    let parts: Vec<&str> = body.split('-').collect();
    if parts.len() != 2 {
        return Err(TemporalErr::Lexical);
    }
    let month: u32 = parts[0].parse().map_err(|_| TemporalErr::Lexical)?;
    let day: u32 = parts[1].parse().map_err(|_| TemporalErr::Lexical)?;
    if !(1..=12).contains(&month) {
        return Err(TemporalErr::Range);
    }
    if !(1..=31).contains(&day) {
        return Err(TemporalErr::Range);
    }
    let tz = match tz_opt {
        Some(t) => Some(parse_tz(t)?),
        None => None,
    };
    Ok((month as u8, day as u8, tz))
}

pub fn parse_g_day(s: &str) -> Result<(u8, Option<FixedOffset>), TemporalErr> {
    let (main, tz_opt) = split_tz(s);
    if !main.starts_with("---") {
        return Err(TemporalErr::Lexical);
    }
    let day_str = &main[3..];
    if day_str.len() != 2 || !day_str.chars().all(|c| c.is_ascii_digit()) {
        return Err(TemporalErr::Lexical);
    }
    let day: u32 = day_str.parse().map_err(|_| TemporalErr::Lexical)?;
    if !(1..=31).contains(&day) {
        return Err(TemporalErr::Range);
    }
    let tz = match tz_opt {
        Some(t) => Some(parse_tz(t)?),
        None => None,
    };
    Ok((day as u8, tz))
}

pub fn build_naive_datetime(
    date: NaiveDate,
    time: NaiveTime,
    tz: Option<FixedOffset>,
) -> chrono::DateTime<FixedOffset> {
    let naive = NaiveDateTime::new(date, time);
    match tz {
        Some(ofs) => chrono::DateTime::from_naive_utc_and_offset(naive - ofs, ofs), // adjust naive local -> UTC naive
        None => chrono::DateTime::from_naive_utc_and_offset(naive, chrono::Utc.fix()),
    }
}
