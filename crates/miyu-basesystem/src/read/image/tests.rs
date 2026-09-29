use super::mib;

#[test]
fn sizes_are_written_in_mib_with_one_decimal() {
    assert_eq!(mib(5 * 1024 * 1024), "5.0 MiB");
    assert_eq!(mib(5 * 1024 * 1024 + 1), "5.0 MiB");
    assert_eq!(mib(7_654_321), "7.3 MiB");
}
