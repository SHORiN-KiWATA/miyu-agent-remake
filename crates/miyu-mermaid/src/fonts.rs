//! 系统的字体库找不找得到字（`mermaid.md`「怎么走」第 2 条，施工 W-4）。
//!
//! 画图的库（`mermaid-rs-renderer`）自己也会去读系统的字体库量字的宽，但它找不到字体时只是退化成按字数估算，
//! 不会报错：核心要的是「读不到字体」这个信号，所以这里独立探一次，不借它的。

/// 这台机器上读得到至少一种字体。
pub(crate) fn available() -> bool {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    db.faces().next().is_some()
}

#[cfg(test)]
mod tests;
