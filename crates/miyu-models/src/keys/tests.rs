//! 照会话编号挑 key（施工 8-6）：算式照图纸、同一个编号永远是同一个、先后、分得开。

use super::*;

#[test]
fn the_key_is_the_digest_mod_the_count() {
    let session = "01990000-0000-7000-8000-000000000000";
    let digest = Sha256::digest(session.as_bytes());
    let number = u64::from_be_bytes(digest[..8].try_into().expect("八个字节"));
    for count in 1..=7usize {
        let expected = usize::try_from(number % count as u64).expect("比 count 小");
        assert_eq!(pinned(session, count), Some(expected), "{count}");
    }
    assert_eq!(pinned(session, 0), None);
    // 算式写死：前 8 个字节照大端。照小端、取后 8 个字节，都算出别的数（守着变异）。
    let little = u64::from_le_bytes(digest[..8].try_into().expect("八个字节"));
    let tail = u64::from_be_bytes(digest[24..].try_into().expect("八个字节"));
    assert!(
        (2..=7u64).any(|count| number % count != little % count && number % count != tail % count),
        "挑得出区分的个数"
    );
}

#[test]
fn the_same_session_always_gets_the_same_key() {
    let session = "0199aaaa-bbbb-7ccc-8ddd-eeeeffff0000";
    let first = pinned(session, 3);
    for _ in 0..10 {
        assert_eq!(pinned(session, 3), first);
    }
}

#[test]
fn the_pinned_key_comes_first_and_the_rest_keep_their_order() {
    for n in 0..40 {
        let session = format!("0199{n:04}-0000-7000-8000-000000000000");
        let order = order(&session, 4);
        let first = pinned(&session, 4).expect("有 key");
        assert_eq!(order[0], first);
        let rest: Vec<usize> = (0..4).filter(|at| *at != first).collect();
        assert_eq!(order[1..], rest[..]);
    }
    assert!(order("s", 0).is_empty());
    assert_eq!(order("s", 1), [0]);
}

#[test]
fn sessions_spread_over_the_keys() {
    let mut seen = [0usize; 3];
    for n in 0..300 {
        let session = format!("0199{n:04}-0000-7000-8000-000000000000");
        seen[pinned(&session, 3).expect("有 key")] += 1;
    }
    assert!(seen.iter().all(|count| *count > 60), "{seen:?}");
}
