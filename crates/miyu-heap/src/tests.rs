//! glibc 上真的改得成、还得回去；别的平台什么都不做。

#[test]
fn the_allocator_can_be_tuned_where_glibc_is() {
    assert_eq!(
        super::tune(),
        cfg!(all(target_os = "linux", target_env = "gnu"))
    );
}

#[test]
fn freed_small_blocks_are_given_back_where_glibc_is() {
    // 一堆小块（不走 mmap，留在堆里），放掉以后堆顶有一大片空着的。
    let blocks: Vec<Vec<u8>> = (0..32 * 1024)
        .map(|n| vec![(n % 251) as u8; 1024])
        .collect();
    assert_eq!(blocks.len(), 32 * 1024);
    drop(blocks);
    let released = super::trim();
    if cfg!(all(target_os = "linux", target_env = "gnu")) {
        assert!(released, "glibc 上空着的还得回去");
    } else {
        assert!(!released);
    }
}
