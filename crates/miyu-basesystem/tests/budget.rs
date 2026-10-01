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
//! 7-7 加了 `message_agent`（边际份量 153，合并时说明去掉分号重量）：十一件合计 1756 个 token、6496 字节，预算改成 1932 个 token，合 7200 字节。施工 7-5 再补
//! 把 `agent` 改名 `subagent`：字节不变，边际份量 140 → 141，十一件合计 1757 个 token，预算不改。施工 C-3 加了 `sessions`（边际
//! 份量 95）：十二件合计 1852 个 token、6858 字节，预算改成 2037 个 token，合 7600 字节。施工 C-4 给 `history` 加了 `session`
//! （边际份量 197 → 224）：十二件合计 1879 个 token、6966 字节，预算改成 2067 个 token，合 7700 字节。加工具、改说明超了，
//! 重新量过再改这里和设计。施工 C-5 把 `message_agent` 改名 `send_message`，说明第一句多了「or to another of
//! your sessions by its id」、`sessions` 第二句点名它和 `history`：字节 6966 → 7048，边际份量 153 → 165、95 → 101，十二件
//! 合计 1897 个 token，还在预算里，预算不改（`26-提示词.md` 第十节）。施工 C-6 给 `send_message` 加 `notify_when_idle`、`message`
//! 改成可以不写：字节 7048 → 7200，边际份量 165 → 204，十二件合计 1936 个 token，还在预算里，预算不改。
//! 施工 8-8 给 `subagent` 加 `tier`：字节 7200 → 7360，边际份量 141 → 189，十二件合计 1984 个 token，还在预算里，预算不改。
//! 施工 8-8 补把 `tier` 换成 `pool`（会话开局时照配置拼，资源里没有 `enum`）：字节 7360 → 7291；不列池时边际份量 189 → 141，
//! 十二件合计 1936 个 token，列一个带说明的池是 182（2026-10-02 主会话量），还在预算里，预算不改。

use std::path::Path;

/// 预算：字节，回车 `\r` 不算（Windows 上检出的可能多出回车）。
const BUDGET: usize = 7700;

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
    assert_eq!(files, 12, "基础系统现在是十二件");
    assert!(
        total <= BUDGET,
        "工具面的几份说明一共 {total} 字节，超过预算 {BUDGET}：重新量 token，再改预算（10-自带软件.md 第九节）"
    );
}
