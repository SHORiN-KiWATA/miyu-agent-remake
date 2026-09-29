//! 用量那一行要的小工具：token 数写短、算百分比。

/// 把 token 数写短：`950`、`12.3k`、`1.4M`。整数不带 `.0`。
pub fn short(n: u64) -> String {
    let (value, unit) = match n {
        0..1_000 => return n.to_string(),
        1_000..1_000_000 => (n as f64 / 1e3, "k"),
        _ => (n as f64 / 1e6, "M"),
    };
    let text = format!("{value:.1}");
    format!("{}{unit}", text.strip_suffix(".0").unwrap_or(&text))
}

/// 百分比，保留一位小数，整数不带 `.0`：`0.2`、`4.1`、`12`。分母是 0 的是 `0`。
pub fn percent_tenths(part: u64, whole: u64) -> String {
    if whole == 0 {
        return "0".to_string();
    }
    let text = format!("{:.1}", part as f64 * 100.0 / whole as f64);
    text.strip_suffix(".0").unwrap_or(&text).to_string()
}

/// 读秒：`12s`、`1m 05s`、`1h 02m 05s`（运行状态行的用时、收起那一行末尾的用时，照旧版的底栏）。
pub fn clock(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    match (h, m) {
        (0, 0) => format!("{s}s"),
        (0, _) => format!("{m}m {s:02}s"),
        _ => format!("{h}h {m:02}m {s:02}s"),
    }
}

/// 命中率（蓝图 `tui.md`「命中率的写法」）：99 以下写整数；99 以上带一位小数，正好 99.0 的写 `99`；
/// 四舍五入到 100.0 的写 `100`。分母是 0 的是 `0`。
pub fn hit_rate(part: u64, whole: u64) -> String {
    if whole == 0 {
        return "0".to_string();
    }
    let exact = part as f64 * 100.0 / whole as f64;
    let tenths = (exact * 10.0).round() / 10.0;
    if tenths < 99.0 {
        return format!("{}", exact.round() as u64);
    }
    let text = format!("{tenths:.1}");
    text.strip_suffix(".0").unwrap_or(&text).to_string()
}

/// 一步、一轮用了多久：一分钟以内带一位小数 `11.8s`，再长 `1m 05s`、`1h 02m 05s`（照旧版的时间线，
/// 和运行状态行的读秒一个写法）。
pub fn seconds(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        return format!("{:.1}s", d.as_secs_f64());
    }
    clock(secs)
}

#[cfg(test)]
mod tests {
    use super::{percent_tenths, seconds, short};

    #[test]
    fn short_numbers() {
        assert_eq!(short(950), "950");
        assert_eq!(short(12_345), "12.3k");
        assert_eq!(short(1_000_000), "1M");
        assert_eq!(short(1_400_000), "1.4M");
    }

    #[test]
    fn tenths_keep_one_decimal() {
        assert_eq!(percent_tenths(1_800, 1_000_000), "0.2");
        assert_eq!(percent_tenths(41_100, 1_000_000), "4.1");
        assert_eq!(percent_tenths(120_000, 1_000_000), "12");
        assert_eq!(percent_tenths(5, 0), "0");
    }

    #[test]
    fn clock_matches_the_old_footer() {
        use super::clock;
        assert_eq!(clock(12), "12s");
        assert_eq!(clock(65), "1m 05s");
        assert_eq!(clock(3725), "1h 02m 05s");
    }

    #[test]
    fn hit_rate_shows_a_decimal_only_above_99() {
        use super::hit_rate;
        assert_eq!(hit_rate(94, 100), "94");
        assert_eq!(hit_rate(9_860, 10_000), "99", "98.6 还不到 99，照整数写");
        assert_eq!(hit_rate(9_900, 10_000), "99", "99.0 不写 .0");
        assert_eq!(hit_rate(9_963, 10_000), "99.6");
        assert_eq!(hit_rate(9_990, 10_000), "99.9");
        assert_eq!(hit_rate(9_996, 10_000), "100", "四舍五入到 100.0 的写 100");
        assert_eq!(hit_rate(0, 0), "0");
    }

    #[test]
    fn step_seconds() {
        use std::time::Duration;
        assert_eq!(seconds(Duration::from_millis(11_840)), "11.8s");
        assert_eq!(seconds(Duration::from_secs(65)), "1m 05s");
        assert_eq!(seconds(Duration::from_secs(3725)), "1h 02m 05s");
    }
}
