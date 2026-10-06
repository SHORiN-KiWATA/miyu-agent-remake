## 施工单 8-3（再补）：`expect` 照数值比

状态：待验收（2026-10-07 写、施工完；网页的会话撞见、转来；只有技术细节，主会话定）。

### 目的

小数的配置项（价格、倍率、温度）文件里存的是整数值时（`input = 1.0`），头带 `expect: {"value": 1}` 改它会被当成冲突拒掉（`config_conflict`，`current` 是 `1.0`）：`expect` 拿 JSON 值直接比，`1.0` 是小数、`1` 是整数，比不上。JS 的头写不出 `1.0`（`JSON.stringify(1.0)` 是 `"1"`），网页只好对这种项不带 `expect`。

### 蓝图改哪几节

- `config.md`「`config.set`」第 5 条：两边都是数字的照数值比；别的照 JSON 一字不差地比。

### 不做什么

- `value` 本身：整数写进小数项本来就收（照清单读成小数），不用改。

### 验收

1. 测试先写，修之前红（`config_set.rs` 的 `expect_on_a_float_compares_the_number`：修之前第二步报 `config_conflict`，`current` 是 `1.0`）：`1` 对得上 `1.0`，写进去的 `3` 对得上 `3.0`，`2` 对不上 `2.5`。
2. `cargo xtask check` 八项、三台机器的 CI 和长跑。
3. 合了告诉网页：去掉对整数值不带 `expect` 的绕法。
