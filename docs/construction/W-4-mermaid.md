## 施工单 W-4：mermaid

状态：待施工（2026-10-02，图纸 `docs/blueprint/web-module.md` 项目主人 2026-10-01 批准；「画 mermaid 在核心里，做成可选的软件包」项目主人 2026-10-01 定）。

### 目的

mermaid 源码画成 SVG 由核心做：代码只有一份，同一张图终端和网页看只画一次。做成可选的软件包 `mermaid`：crate `miyu-mermaid`，经 `miyu-core` 的 cargo 开关 `mermaid` 编进来，发行版默认打开；没编进来的核心里没有这块代码，`mermaid.render` 回 `unknown_method`，头照代码块显示源码。第一次调才初始化（读字体），之后留着。

这一步顺带立起「可选软件包登记查询」的那张表：端点多一张查询表（方法名到怎么答），核心起来时照编进来的包往里登记。W-7 的 `net` 包照它登记 `link.preview`。

### 蓝图改哪几节

图纸是 `web-module.md`，只读标着 W-4 的这几处：
- 「在哪」：`crates/miyu-endpoint/src/queries.rs`（查询表）、`crates/miyu-mermaid/`（新，第 3 层）、`crates/miyu-core/src/packages.rs`（照编进来的包登记）、`resources/software/mermaid/style.json`。
- 「对外的样子」：「核心多的方法」表 `mermaid.render`；「每个方法的参数和回应」的 `mermaid.render`。
- 「怎么走」第五条（九款）。
- 「出错」：`mermaid_too_long`、`mermaid_failed`，`bad_params` 多的「源码是空的」，`internal_error` 多的「画图的库初始化不了」；运行日志 `WARN mermaid not ready`；「给人看的字」那两句。
- 「守着它的」：`crates/miyu-mermaid/src/tests.rs`、`crates/miyu-core/tests/packages.rs` 那一行。
- 「起草时定的」第 19 到 23 条（第 23 条：施工时另立 `docs/blueprint/mermaid.md`，`web-module.md` 第五条只留指过去的一句）。
- 搬过来的先例（只读，别改那两个分支）：`git show proto/tui-demo:tui-demo/src/figures/mermaid.rs`（和它的 `mermaid/tests.rs`）画 SVG 的那一半、三种记号色；`git show proto/web-demo:web-demo/bridge/src/mermaid.rs`、`web-demo/resources/mermaid.json`（样子、字体）。依赖照终端演示：`mermaid-rs-renderer = { version = "0.3", default-features = false }`，版本和它对上（图纸写 0.3.1）。
- 分层：`01-架构.md` 第九节登记 `miyu-mermaid`（第 3 层），门禁读那张表。
- 跟着改：`protocol.md`（方法表、`mermaid.render` 一段、出错、给人看的字、运行日志）、`core.md`（起来的先后里登记可选软件包，「在哪」加 `packages.rs`）、`store/resources.md`（资源目录多 `software/mermaid/`）、`log.md`（目标 `miyu::mermaid`）、`licenses.md`（新依赖和它带的字体库；许可证门禁要过）、`10-自带软件.md` 第四节（已经写了「画 mermaid」，核对）。新页 `mermaid.md`，`docs/blueprint/README.md` 的页表加一行。

### 不做什么

- `view.detail`（M9 的视图投影）：随 M9，到时候照同一份缓存给。
- 终端栅格化：在头里，不在核心。
- 改两个演示：合了主会话通知它们的会话自己改（终端去掉画 SVG 的那一半和依赖，网页去掉桥的 `mermaid.rs`）。

### 验收

1. 测试（先写，退回改之前的代码要红）：照「守着它的」W-4 那一行：
   - 记号色都换得掉、底和框不填色；回应的 `marks` 三种色和 SVG 里用的对得上；
   - 同一份源码第二次不重画（缓存，最多 64 张，满了丢最久没用的）；
   - 空的 `bad_params`、超过 64 KiB `mermaid_too_long`、画不出 `mermaid_failed`（`data.detail` 是库的原话）、库崩了（panic）当画不出；
   - 第一次调之前不读字体（初始化是懒的）；
   - 没编进来（关掉 cargo 开关的核心）回 `unknown_method`：照仓库已有的办法在测试里编一个不带开关的版本，或者测查询表本身（没登记的方法回 `unknown_method`），施工时定、写明；
   - 真核心走一遍：一张流程图、一张时序图都出 SVG。
2. 给模型看的字：没有。请求形状探针零变化。
3. 手写变异 15 个左右，挑关键的，全被逮住；`CARGO_BUILD_JOBS=5 cargo xtask check` 八项全过；三台机器的 CI 和长跑全绿（字体在三个平台上找得到；macOS、Windows 上系统字体的位置不同，测试别依赖某一种字体在不在，只验「读得到至少一种、读不到时回 `internal_error`」）。
4. 协议多了方法：合了主会话告诉两个头，附 sha 和形状。

### 风险

- 画图的库编译慢、体积大：关掉它的默认功能（照终端演示），只要出 SVG 的那一半。release 二进制的体积前后量一下，写进验收结果。
- 和 8-8 补、8-18 同时在路上：冲突多半在 `methods.rs`（W-4 改成走查询表）、`protocol.md`、`Cargo.lock`，主会话合并时解。
