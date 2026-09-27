use super::*;

#[test]
fn the_timeout_is_kept_within_bounds() {
    assert_eq!(limit(None), DEFAULT);
    assert_eq!(limit(Some(0)), DEFAULT);
    assert_eq!(limit(Some(1)), 1);
    assert_eq!(limit(Some(MAX)), MAX);
    assert_eq!(limit(Some(MAX + 1)), MAX);
}

#[test]
fn seconds_drop_a_zero_fraction() {
    assert_eq!(seconds(120_000), "120");
    assert_eq!(seconds(300), "0.3");
    assert_eq!(seconds(1_500), "1.5");
}

#[test]
fn the_schema_states_the_same_limits() {
    let schema = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources/software/basesystem/tools/shell.json"),
    )
    .expect("读得出");
    assert!(
        schema.contains(&format!("up to {MAX}. Default {DEFAULT}.")),
        "{schema}"
    );
}

#[test]
fn a_body_ends_each_piece_with_a_line_break() {
    assert_eq!(line(String::new()), "");
    assert_eq!(line("a".into()), "a\n");
    assert_eq!(line("a\n".into()), "a\n");
}
