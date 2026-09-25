//! 字符串取值的测试：认识的读成对应的一种；不认识的原样留着；只收字符串。

use crate::test_support::round_trip;

text_enum!(
    /// 测试用的一种取值。
    Color {
        Red = "red",
        DarkBlue = "dark_blue",
    }
);

#[test]
fn known_values_read_into_their_variants() {
    let color: Color = serde_json::from_str(r#""dark_blue""#).unwrap();
    assert_eq!(color, Color::DarkBlue);
    assert_eq!(color.as_str(), "dark_blue");
    round_trip::<Color>(r#""red""#);
}

#[test]
fn unknown_values_are_kept_as_they_are() {
    let color: Color = serde_json::from_str(r#""green""#).unwrap();
    assert_eq!(color, Color::Other("green".to_string()));
    round_trip::<Color>(r#""green""#);
}

#[test]
fn only_strings_are_values() {
    assert!(serde_json::from_str::<Color>("1").is_err());
    assert!(serde_json::from_str::<Color>("null").is_err());
}
