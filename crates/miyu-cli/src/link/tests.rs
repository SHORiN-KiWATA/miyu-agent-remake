//! 握手以后说哪种话（施工 8-2）：照回应的 `language`，`ja` 的照英文，老的核心没回的照握手以前的；配置里有几处错误；
//! 握手以前照系统的语言挑。

use serde_json::json;

use super::{config_errors, spoken};
use crate::language::{Language, of_locale};

#[test]
fn the_reply_decides_the_language() {
    assert_eq!(
        spoken(&json!({"language": "zh"}), Language::English),
        Language::Chinese
    );
    assert_eq!(
        spoken(&json!({"language": "en"}), Language::Chinese),
        Language::English
    );
    assert_eq!(
        spoken(&json!({"language": "ja"}), Language::Chinese),
        Language::English,
        "日文照英文"
    );
    assert_eq!(
        spoken(&json!({}), Language::Chinese),
        Language::Chinese,
        "老的核心没回"
    );
    assert_eq!(
        spoken(&json!({"language": "fr"}), Language::Chinese),
        Language::Chinese,
        "认不出的照旧"
    );
}

#[test]
fn config_errors_default_to_none() {
    assert_eq!(config_errors(&json!({"config_errors": 2})), 2);
    assert_eq!(config_errors(&json!({})), 0);
}

#[test]
fn before_the_hello_the_system_language_decides() {
    assert_eq!(
        of_locale(Some("zh-Hans-CN")),
        Language::Chinese,
        "macOS 报的写法"
    );
    assert_eq!(of_locale(Some("zh_CN.UTF-8")), Language::Chinese);
    assert_eq!(of_locale(Some("ja-JP")), Language::English);
    assert_eq!(of_locale(None), Language::English);
}
