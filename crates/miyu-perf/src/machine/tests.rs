//! 读 `/proc` 的字。

use super::*;

#[test]
fn the_cpu_model_is_the_first_one() {
    let text = "processor\t: 0\nvendor_id\t: AuthenticAMD\nmodel name\t: AMD Ryzen 7 7840HS w/ Radeon 780M Graphics\n\nprocessor\t: 1\nmodel name\t: other\n";
    assert_eq!(
        parse_cpuinfo(text).as_deref(),
        Some("AMD Ryzen 7 7840HS w/ Radeon 780M Graphics")
    );
    assert_eq!(parse_cpuinfo("processor\t: 0\n"), None);
}

#[test]
fn total_memory_is_in_bytes() {
    let text = "MemTotal:       32012345 kB\nMemFree:         1234567 kB\n";
    assert_eq!(parse_meminfo(text), Some(32_012_345 * 1024));
    assert_eq!(parse_meminfo("MemFree: 1 kB\n"), None);
}
