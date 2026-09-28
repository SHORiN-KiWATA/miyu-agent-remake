//! 随机密码（`docs/blueprint/sandbox/windows.md`「沙盒用户」）：32 个字符，四类都有，只用表里的字。

use super::*;

/// 几份不同的随机字节：全零、全满、递增、乱的。
fn samples() -> Vec<[u8; LENGTH]> {
    let mut rising = [0u8; LENGTH];
    for (i, byte) in rising.iter_mut().enumerate() {
        *byte = u8::try_from(i * 7).unwrap_or(u8::MAX);
    }
    let mut mixed = [0u8; LENGTH];
    let mut state: u32 = 0x9E37_79B9;
    for byte in &mut mixed {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        *byte = state.to_le_bytes()[0];
    }
    vec![[0; LENGTH], [u8::MAX; LENGTH], rising, mixed]
}

#[test]
fn a_password_is_thirty_two_characters_of_every_kind() {
    for random in samples() {
        let password = generate(&random);
        assert_eq!(password.chars().count(), LENGTH, "{password}");
        for (kind, set) in [
            ("upper", UPPER),
            ("lower", LOWER),
            ("digit", DIGITS),
            ("symbol", SYMBOLS),
        ] {
            assert!(
                password.bytes().any(|b| set.contains(&b)),
                "{password} 没有 {kind}"
            );
        }
    }
}

#[test]
fn only_the_listed_characters_are_used() {
    for random in samples() {
        let password = generate(&random);
        for b in password.bytes() {
            let listed = [UPPER, LOWER, DIGITS, SYMBOLS]
                .iter()
                .any(|set| set.contains(&b));
            assert!(listed, "{password}: {}", char::from(b));
        }
    }
}

#[test]
fn the_symbols_are_the_ones_in_the_blueprint() {
    assert_eq!(SYMBOLS, b"!#%+-.:=?@^_~");
}

#[test]
fn different_random_bytes_give_different_passwords() {
    let passwords: Vec<String> = samples().iter().map(generate).collect();
    for (i, one) in passwords.iter().enumerate() {
        for other in &passwords[i + 1..] {
            assert_ne!(one, other);
        }
    }
    // 只差最后一个字节，也不一样：每个字节都用上了。
    let mut a = [0u8; LENGTH];
    let mut b = [0u8; LENGTH];
    a[LENGTH - 1] = 1;
    b[LENGTH - 1] = 2;
    assert_ne!(generate(&a), generate(&b));
}
