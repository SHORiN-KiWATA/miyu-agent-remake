//! 工具面的预算（`docs/designs/10-自带软件.md` 第九节，施工 4-10）：`resources/software/basesystem/tools/` 下的几份
//! 说明加起来不超过 [`BUDGET`] 字节。
//!
//! 仓库里没有分词器，预算照实测换成字节：2026-09-28 用 DeepSeek `deepseek-flash` 量，七件的边际份量合计 1076 个
//! token，说明合计 3998 字节，约 3.7 字节一个 token；预算是实测加一成，1184 个 token，合 4400 字节（项目主人定照
//! 字节守）。施工 4-13 重量：`read` 加上图片、`shell` 加 `description`，合计 1119 个 token、4149 字节，预算改成
//! 1231 个 token，合 4600 字节。施工 6-4 加了 `history`：八件合计 1306 个 token、4832 字节，预算改成 1437 个 token，合 5400
//! 字节。施工 7-5 加了 `agent`（边际份量 140）：九件合计 1444 个 token、5369 字节，预算改成 1589 个 token，合 6000 字节。施工 7-3 给
//! `shell` 加 `run_in_background`（边际份量 152 → 183）：九件合计 1476 个 token、5494 字节，预算改成 1624 个 token，合 6100
//! 字节。施工 7-4 加了 `jobs`（边际份量 127）：十件合计 1603 个 token、5941 字节，预算改成 1764 个 token，合 6600 字节。施工
//! 7-7 加了 `message_agent`（边际份量 153，合并时说明去掉分号重量）：十一件合计 1756 个 token、6496 字节，预算改成 1932 个 token，合 7200 字节。加工具、
//! 改说明超了，重新量过再改这里和设计。

use std::path::Path;

/// 预算：字节，回车 `\r` 不算（Windows 上检出的可能多出回车）。
const BUDGET: usize = 7200;

#[test]
fn the_tool_face_stays_within_its_budget() {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/software/basesystem/tools");
    let mut total = 0;
    let mut files = 0;
    for entry in std::fs::read_dir(&dir).expect("读得了工具说明的目录") {
        let path = entry.expect("读得了").path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let bytes = std::fs::read(&path).expect("读得了");
            total += bytes.iter().filter(|byte| **byte != b'\r').count();
            files += 1;
        }
    }
    assert_eq!(files, 11, "基础系统现在是十一件");
    assert!(
        total <= BUDGET,
        "工具面的几份说明一共 {total} 字节，超过预算 {BUDGET}：重新量 token，再改预算（10-自带软件.md 第九节）"
    );
}
