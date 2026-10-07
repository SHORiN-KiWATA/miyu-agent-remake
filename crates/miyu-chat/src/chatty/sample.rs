//! 抽样的哈希（`docs/blueprint/chat.md` 第三条「怎么走」第 6 条、第七条第 1 条）：种子由「场所编号加序号」算出来，不碰
//! 随机源（`02-内核.md` K1），同一份日志回放出同样的结果。
//!
//! 写法定死了不能随便改：改了旧日志回放出的抽样就变了。O-12 照第七条改成序号的时候，场所日志还没有落地，不影响回放。
//! 测试里钉着两个拿 Python 的 hashlib 另算的数。

use miyu_kernel::id::{Seq, VenueId};
use sha2::{Digest, Sha256};

/// 这条消息抽中没有：`SHA-256(场所编号 + "\n" + 序号的十进制)` 的前 8 个字节大端读成无符号整数 `x`，`x × 1000 ÷ 2⁶⁴`
/// （取整）小于 `probability`（千分比）就中。`0` 永不中，`1000` 及以上必中。
///
/// 换行隔开两段：编号里不会有换行（内核的编号不收控制字符），`"12" + "3"` 和 `"1" + "23"` 不会撞成同一个种子。
pub(super) fn drawn(venue: &VenueId, msg: Seq, probability: u16) -> bool {
    let digest: [u8; 32] = Sha256::new()
        .chain_update(venue.as_str())
        .chain_update("\n")
        .chain_update(msg.get().to_string())
        .finalize()
        .into();
    let [b0, b1, b2, b3, b4, b5, b6, b7, ..] = digest;
    let x = u64::from_be_bytes([b0, b1, b2, b3, b4, b5, b6, b7]);
    // 在 u128 里乘，乘出来不会溢出；右移 64 位就是除以 2⁶⁴ 取整，落在 0 到 999。
    (u128::from(x) * 1000) >> 64 < u128::from(probability)
}
