//! 时间：UTC，精确到毫秒。JSON 里写成 `"2026-09-25T07:04:05.123Z"`，固定 24 个字符
//! （`docs/designs/03-事件模型.md` 第二节）。只存 UTC，显示成本地时间是头的事。
//!
//! 公历日期和天数的互转用的是 Howard Hinnant 的标准算法（days_from_civil），不引入日期库。

use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::format_error::FormatError;

const MS_PER_DAY: i64 = 86_400_000;
/// 0000-01-01T00:00:00.000Z
const MIN: i64 = -62_167_219_200_000;
/// 9999-12-31T23:59:59.999Z
const MAX: i64 = 253_402_300_799_999;

/// 一个时刻：从 1970-01-01T00:00:00.000Z 起的毫秒数，UTC。
///
/// 只收 0000 年到 9999 年，所以写出去永远是 24 个字符。内核自己不读时钟：
/// 事件的时间取自执行器送进来的输入（`02-内核.md` 第四节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(i64);

impl Timestamp {
    /// 由 Unix 毫秒数得到时刻。超出 0000 年到 9999 年返回 `None`。
    pub fn from_unix_millis(ms: i64) -> Option<Timestamp> {
        (MIN..=MAX).contains(&ms).then_some(Timestamp(ms))
    }

    /// 从 1970-01-01T00:00:00.000Z 起的毫秒数；1970 年以前是负数。
    pub fn unix_millis(self) -> i64 {
        self.0
    }

    /// 读 `2026-09-25T07:04:05.123Z` 这样的写法。
    ///
    /// # Errors
    ///
    /// 只认这一种写法：长度不是 24、分隔符不对、有不是数字的地方、日期或时刻不存在
    /// （例如 2 月 30 日、24 点、闰秒 60 秒），都返回 [`FormatError`]。
    pub fn parse(text: &str) -> Result<Timestamp, FormatError> {
        let bad = |why| FormatError::new("时间", text, why);
        let b = text.as_bytes();
        if b.len() != 24 {
            return Err(bad("要 24 个字符，写成 2026-09-25T07:04:05.123Z"));
        }
        let separators = [
            (4, b'-'),
            (7, b'-'),
            (10, b'T'),
            (13, b':'),
            (16, b':'),
            (19, b'.'),
            (23, b'Z'),
        ];
        if separators.iter().any(|&(i, sep)| b[i] != sep) {
            return Err(bad("写成 2026-09-25T07:04:05.123Z"));
        }
        let number = |from: usize, to: usize| {
            let digits = &b[from..to];
            if !digits.iter().all(u8::is_ascii_digit) {
                return Err(bad("日期和时间只能写数字"));
            }
            Ok(digits.iter().fold(0, |n, d| n * 10 + i64::from(d - b'0')))
        };
        let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
        let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
        let milli = number(20, 23)?;
        if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
            return Err(bad("没有这一天"));
        }
        if hour > 23 || minute > 59 || second > 59 {
            return Err(bad("没有这个时刻"));
        }
        let in_day = ((hour * 60 + minute) * 60 + second) * 1000 + milli;
        Ok(Timestamp(
            days_from_civil(year, month, day) * MS_PER_DAY + in_day,
        ))
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (year, month, day) = civil_from_days(self.0.div_euclid(MS_PER_DAY));
        let in_day = self.0.rem_euclid(MS_PER_DAY);
        let (second, milli) = (in_day / 1000, in_day % 1000);
        write!(
            f,
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{milli:03}Z",
            second / 3600,
            second / 60 % 60,
            second % 60
        )
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Timestamp::parse(&String::deserialize(d)?).map_err(D::Error::custom)
    }
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// 公历日期 → 从 1970-01-01 起的天数。
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let month_from_march = (month + 9) % 12;
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// 从 1970-01-01 起的天数 → 公历日期。
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests;
