//! `MIYU_LOG` 的值：不分大小写，前后的空白不算，读不懂的照 `INFO`。

use super::*;

#[test]
fn values_are_read_without_case_and_unknown_ones_fall_back_to_info() {
    let known = |filter| Level {
        filter,
        unknown: None,
    };
    assert_eq!(level(None), known(LevelFilter::INFO));
    assert_eq!(level(Some("  ")), known(LevelFilter::INFO));
    assert_eq!(level(Some("error")), known(LevelFilter::ERROR));
    assert_eq!(level(Some(" warn\n")), known(LevelFilter::WARN));
    assert_eq!(level(Some("info")), known(LevelFilter::INFO));
    assert_eq!(level(Some("DEBUG")), known(LevelFilter::DEBUG));
    assert_eq!(level(Some("Trace")), known(LevelFilter::TRACE));
    assert_eq!(level(Some("off")), known(LevelFilter::OFF));
    assert_eq!(
        level(Some("loud")),
        Level {
            filter: LevelFilter::INFO,
            unknown: Some("loud".to_string()),
        }
    );
}
