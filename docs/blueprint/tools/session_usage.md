## `session_usage`

### 是什么

她自己查这个会话用了多少、花了多少钱、上下文还剩多少（施工 8-15，`models.md`「怎么走」第九条第 7 条，`15-模型与供应商.md` 第八节）。只查这个会话，不带子会话，不给账号的汇总。用量、金额和头经 `usage.query` 读的是同一份汇总；上下文是派出去那一刻内核照压缩线的算法估的。只有本机的会话（主会话、子会话）有这件工具，群里的会话没有（「施工时定的」8-15：群里的人不可信，花了多少钱是属主的事）。

一件管用量、金额和上下文，不拆两件（主会话定）：合并前主会话用真模型问六句（还剩多少上下文、这次花了多少钱、快压缩了吗……），挑对、答对就定一件；挑错、答错再开一步试两件。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/session_usage.rs` | 零参数；经端口要用量、金额、上下文，一句一行写出来；金额的写法 |
| `crates/miyu-tool/src/usage.rs` | 查用量的端口 `UsagePort`、上下文 `ContextUse`、用量和金额 `Spent`、那件工具的名字 `SESSION_USAGE`（`tools/interface.md`） |
| `crates/miyu-session/src/usage.rs` | 执行器这一头：派 `session_usage` 那一刻向内核要上下文（`Session::context_used()`、`context_limits()`）、照这一轮的配置取 `usage.currency`（`asked`）；端口先补这个会话、再只查它（`Ledger`） |
| `crates/miyu-session/src/actor.rs`、`tools.rs` | 派一次调用时抄好那一刻的上下文，交给端口（`Dispatch.usage`、`ToolKit.ledger`） |
| `crates/miyu-session/src/agents.rs` | 工具面：只有本机的会话有 `session_usage` |
| `crates/miyu-store/src/usage.rs`、`usage/` | 用量汇总：补一个会话（`catch_up_session`）、查（`query`，`models.md`「在哪」） |
| `resources/software/basesystem/tools/session_usage.json` | 说明和参数格式 |
| `resources/software/basesystem/session_usage/*.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/human/{zh,en,ja}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `read`：只读用量汇总和这个会话的日志，什么都不改。一条路径都不报，不报效果。

样本 `resources/software/basesystem/tools/session_usage.json`：

```json
{
  "description": "Show how many tokens and how much money this session has used so far, and how full your context is.",
  "parameters": {"type":"object","properties":{}}
}
```

- 没有参数；写了什么都不认，也不报错。
- 2026-10-02 主会话照开发端点、`deepseek-v4.1-flash` 量：十三件一起时的边际份量 48；发给模型的 tools 数组不列池的 2147 → 2195（`26-提示词.md` 第十节）。

### 怎么走

1. 没有端口（`Call.usage` 是空的：测试里的假调用、核心没开用量汇总）：照什么都没花答，只写 `usage.txt`（全是 0），说法 `session_usage/shown`。
2. 问端口要用量和金额：执行器在阻塞线程里先补这个会话（日志里比汇总记到的多出来的那一截，`models.md` 第九条第 4 条），再只查这个会话、不带子会话。补的时候读不完的记一行 `WARN usage not indexed`，照读到的算。金额照币种各加各的、不换算，照这一轮配置的 `usage.currency` 排最前，别的照币种代码的字母先后。
3. 查不了（汇总读不了、写坏了）：`failed.txt`，出错，说法 `session_usage/failed`（字段 `error`，英文的一句）。不当成「什么都没花」答。
4. 叫停的旗举起来了：交回「停下了」（`Done::stopped`）。
5. 写，一句一行：
   1. `usage.txt`：请求数、输入（没命中、命中、写进缓存三项加起来）、其中命中缓存的、输出。
   2. 有金额的：`cost.txt`，每种币种一段 `<金额> <币种>`，用 ` + ` 接起来。金额三位有效数字、至少两位小数，多出来的 0 去掉（`0.42`、`1.30`、`0.000292`、`1234.57`）：一次请求花的常常不到一分钱，两位小数会写成 0（「施工时定的」8-15）。
   3. 有用量、没价格的请求：`unpriced.txt`，几次。
   4. 上下文：派出去那一刻内核算的（和压缩线同一个算法，`compaction.md` 第一条）。算不了的（没交过限额、策略里没有压缩）不写。有窗口的写 `context.txt`，有压缩线的下一行再写 `compaction.txt`；没窗口的写 `context-no-window.txt`。
   5. 说法 `session_usage/shown`。

### 样子

```text
Usage so far: 12 requests, 48210 input tokens (40122 from cache), 3120 output tokens.
Cost: 0.0123 USD + 1.30 CNY.
2 requests have no price, so the cost leaves them out.
Context: 23110 of 128000 tokens.
Compaction starts at 95000.
```

给她的字都在 `resources/software/basesystem/session_usage/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节（token 照典型值，2026-10-02 主会话量）：

| 什么时候 | 文件 | 原文 | token |
|---|---|---|---|
| 总有 | `usage.txt` | `Usage so far: {requests} requests, {input} input tokens ({cached} from cache), {output} output tokens.` | 25 |
| 有金额 | `cost.txt` | `Cost: {amounts}.` | 9 |
| 有没价格的 | `unpriced.txt` | `{count} requests have no price, so the cost leaves them out.` | 13 |
| 有窗口 | `context.txt` | `Context: {used} of {window} tokens.` | 11 |
| 有压缩线 | `compaction.txt` | `Compaction starts at {line}.` | 8 |
| 没窗口 | `context-no-window.txt` | `Context: about {used} tokens. This model reports no window.` | 14 |
| 查不了 | `failed.txt` | `Could not read the usage: {error}` | 10 |

- 图纸草稿的 `context.txt` 一句带着压缩线，窗口有、压缩线没有的会话写不对，施工时拆成两份；查不了要有一句，多了 `failed.txt`（照 `sessions/failed.txt`）。

### 出错

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 查不了 | `failed.txt` | `session_usage/failed`，字段 `error` |

### 给人看的字

显示名：查用量（Usage，使用量），不跟参数；符号 `≡`。

| 说法 | 中文 | 英文 | 日文 |
|---|---|---|---|
| `session_usage/shown` | 查了用量 | Checked the usage | 使用量を確認しました |
| `session_usage/failed`（`error`） | 用量读不出来：{error} | Could not read the usage: {error} | 使用量を読めませんでした：{error} |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/session_usage.rs` | 访问类别是读、零参数；输出一字不差：用量、两种币种的金额、没价格的、窗口和压缩线；有窗口没压缩线的、没窗口的、算不了上下文的、没金额的；小金额的写法；参数不认也不报错；没有端口；查不了；叫停；说法中文、英文换得出字、有显示名 |
| `crates/miyu-basesystem/src/session_usage/tests.rs` | 金额三位有效数字、至少两位小数 |
| `crates/miyu-session/tests/session_usage.rs` | 真会话：剧本带价格，第二轮调它看到前两次请求的用量、金额，上下文照窗口、压缩线 |
| `crates/miyu-session/tests/sessions.rs`、`read.rs` | 工具面：本机的会话有它，群里的没有，别的工具一件不少 |
| `crates/miyu-basesystem/tests/budget.rs` | 工具面的预算：十三件，7465 字节 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `models.md`「工具」`session_usage`、「怎么走」第九条第 7 条、「施工时定的」8-15；`15-模型与供应商.md` 第八节；`26-提示词.md` 第十节。
