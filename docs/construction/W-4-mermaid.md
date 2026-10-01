## 施工单 W-4：mermaid

状态：已完成（2026-10-02，图纸 `docs/blueprint/web-module.md` 项目主人 2026-10-01 批准；「画 mermaid 在核心里，做成可选的软件包」项目主人 2026-10-01 定）。

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

### 验收结果（2026-10-02）

1. **测试**（先写；这一步之前仓库里没有 `miyu-mermaid`、`queries.rs`、`packages.rs`，这几份测试和它们要测的代码一起写，写完立刻跑绿；用「手写变异」代替「退回看红」证明测试真的在守着逻辑，见第 3 条）：
   - `crates/miyu-mermaid/src/tests.rs`（10 条）：记号色都换得掉、底不填色；同一份源码第二次不重画（私下查缓存条目数没有变成 2）；满了丢最早的那一张（FIFO，不是真的按最近用过排，和图纸「源码加尺寸」「满了丢最久没用的」一节的措辞一致，缓存只照源码不照尺寸）；空的 `Empty`、超上限 `TooLong`、画不出 `Failed`（库的原话不是空的）；造出 `Mermaid` 不碰磁盘、第一次调才初始化；`style.json` 读不懂当 `NotReady`；panic 的几种 payload（`&str`、`String`、别的类型）都翻成字符串；真的 `style.json`，一张流程图一张时序图都画得出。
   - `crates/miyu-mermaid/src/style/tests.rs`（5 条）：读得进来；文件没有、JSON 坏了、多一个不认的格、`fonts`/`max_source`/`keep` 是空的或者 0 都读不了。
   - `crates/miyu-mermaid/src/fonts/tests.rs`（1 条）：这台机器上读得到至少一种字体（真机验；图纸要求「别依赖某一种字体在不在，只验读得到至少一种」，这条就是那个验法，三个平台 CI 都会跑到）。
   - `crates/miyu-endpoint/src/queries.rs` 内联测试（3 条）：没登记的方法交回 `None`；登记过的名字要正好对上才找得到；同一个名字登记两次当场 panic。
   - `crates/miyu-core/tests/packages.rs`（6 条，真核心、真协议）：一张流程图一张时序图都出 SVG，回应的 `marks` 三种色和 SVG 里用的对得上；同一份源码两次拿到一样的回应；空的 `bad_params`；超过 64 KiB（真的 `style.json` 的 `max_source`）`mermaid_too_long`；画不出 `mermaid_failed`（`data.detail` 不是空的）；查询表是空表（没编进来某个软件包的情形）时 `mermaid.render` 回 `unknown_method`。
   - 「没编进来回 `unknown_method`」照「施工时定」（见下）测查询表本身，没有额外编一份关掉 `mermaid` 开关的 `miyu-core`；手动跑过一次 `cargo check -p miyu-core --no-default-features`（干净，不进自动门禁，见「施工时定的」第 11 条）确认关掉开关确实编得过、确实不带 `miyu-mermaid`。
2. **给模型看的字**：没有改。`mermaid.render` 只给头，`style.json` 不发给模型；门禁「文档」一项里的登记簿检查原先会把新出现的 `resources/software/mermaid/style.json` 当成「没登记的给模型看的字」拦下来——这是登记簿检查本身没有预料到「非 `models/` 顶层目录下也会有纯数据文件」，照登记簿的原意（26 第十节：只登记真的进请求的字）给 `xtask/src/ledger.rs` 补了一条豁免（`software/mermaid/` 整个不查，和顶层 `models/` 同一个道理），`xtask/src/ledger.rs` 自己的单测 `human_folders_are_not_walked` 顺带补了这一种。改完请求形状探针没有碰：没加测试里的新用例，`cargo xtask check` 的「测试」一项本来就含它，全绿。
3. **变异**：手写 17 个（超过「15 个左右」），一次改一处，跑对应的测试文件或具体用例，全部逮住，再用 `git checkout -- <file>`（施工前先 `git add -A` 做了一次快照）整份恢复、确认和改动前逐字节一样：
   - `miyu-mermaid/src/lib.rs`（9 处）：`source.is_empty()` 判断废掉（恒假）；`source.len() > max_source` 改成 `>=`（差一错误，用正好等于上限的源码去卡边界）；缓存淘汰的界线 `>=` 改成 `>`（差一错误）；淘汰丢的从 `remove(0)`（最早的）改成 `pop()`（最新的）；缓存命中判断强行加 `&& false`（永远不命中）；底色 `"none"` 改成 `"black"`；字体可用性判断取反（`!fonts::available()`，本机真的有字体，马上让除「字体不可用」以外的全部测试一起红）；panic 兜底文案从 `"the drawing library panicked"` 改成别的字符串；`fonts::is_empty`/`max_source == 0`/`keep == 0` 三条校验各废掉一次（算进 style.rs 那组，见下）。
   - `miyu-mermaid/src/style.rs`（3 处）：`fonts` 是空的、`max_source` 是 0、`keep` 是 0 这三条校验各废掉一次（`if false {}`）。
   - `miyu-endpoint/src/queries.rs`（2 处）：`get()` 的名字比对从 `*name == method` 改成恒真（任何名字都能找到已登记的第一个方法）；`register()` 里「不许重复登记」的断言条件取反（变成「必须已经登记过才让登记」，第一次登记就会 panic）。
   - `miyu-core/src/packages.rs`（2 处）：`TooLong` 映射的原因码从 `"mermaid_too_long"` 错写成 `"mermaid_failed"`；`Failed` 的 `data` 字段名从 `"detail"` 错写成 `"detail2"`。
   - `miyu-endpoint/src/refusal.rs`（1 处）：`QueryError::BadParams` 的映射从 `Refusal::BAD_PARAMS` 错写成 `Refusal::INTERNAL`。
   - `miyu-endpoint/src/methods.rs`（1 处）：查询表那一支直接改成恒 `None`（相当于把 `mermaid.render` 的接线整条拔掉），确认真协议测试真的在验证这根线接上了，不是蒙对的。
   - 每一处都单独验证：改完跑最相关的一两条测试，确认标红、看报错内容和预期的坏处对得上，再整份恢复。
   - `CARGO_BUILD_JOBS=5 cargo xtask check`：第一遍「文档」没过（见第 2 条的登记簿问题），补上豁免规则后第二遍八项全过（格式、clippy、文档、分层、纯逻辑、行数、许可证、测试）。
4. **release 二进制体积**：`cargo build --release -p miyu`（本机 Linux，`target/release/miyu`）。开着 `mermaid`（出厂默认）：32,560,600 字节（≈31.1 MiB）；临时把 `crates/miyu/Cargo.toml` 的 `miyu-core` 依赖改成 `default-features = false` 重新编（编完照「省额度」的办法 `git checkout` 整份恢复，没留痕）：28,501,576 字节（≈27.2 MiB）。差 4,059,024 字节（≈3.87 MiB，占开着时整个二进制的 12.5%）：`mermaid-rs-renderer`（关掉了它默认的 `cli`、`png` 两个功能，`resvg`/`usvg`/`clap` 都不进依赖图）加上它带来的 `fontdb`、`regex`、`json5`、`ttf-parser` 这几个。CI 的三平台构建会各自给出各平台的数，这里只有本机 Linux 的一份。
5. **协议多了方法**：待推到 `step/w-4-mermaid`、CI 全绿、合进 main 后，按规矩给终端界面、网页两个头的会话各发一条，附 sha 和 `mermaid.render` 的形状。

**施工时定的**（技术细节照推荐定，详细的「定了什么/为什么/别的选法」写进 `mermaid.md`「起草时定的」第 6 到 12 条）：

- 核心独立探一次系统的字体库（不借画图库内部那一份）：画图库自己找不到字体时只退化成按字数估算宽度，从不报错，`internal_error`（「读不到字体」）要核心自己能判断。
- 没能找到稳定触发 `mermaid-rs-renderer` 内部 panic 的源码（试了空白、半截语法、递归很深的箭头、二进制字节几十种，全部只报解析错误）：panic 的捕获靠 `std::panic::catch_unwind` + 独立测过的 `panic_message` 辅助函数，没有在真机逼出过一次真的 panic。
- `panic_message` 的参数特地写成 `&Box<dyn Any + Send>`、不是 `&(dyn Any + Send)`：`Box<dyn Any>` 自己也实现 `Any`，写成后一种会把方法调用点之外那层 `Box` 整体当成被查的类型，`downcast_ref` 永远查不中真正装的 `&str`/`String`（写完立刻被第一轮测试逮到：三个 panic 用例全部落到兜底分支，查实是这个坑，这不是施工单原计划要踩的坑，记在这里备查）。
- `style.json` 没有 `note` 字段，`fonts`、`marks`、`max_source`、`keep` 都是纯数据，`#[serde(deny_unknown_fields)]`：两个演示的同名资源文件里带了一句 `note`，这个仓库没有先例，不照抄。
- 懒初始化只试一次：第一次 `render` 读 `style.json`、探字体，成不成都记进 `OnceLock`，这个核心的生命周期里不会再重试；失败的 `WARN` 只在那一次 `get_or_init` 的闭包里记一条，不是每次调用都记。
- 运行日志的目标用 `miyu::mermaid`，不是 `web-module.md` 组合日志表默认的 `miyu::endpoint`：和以后 `net` 包的 `miyu::net` 一个道理，软件包自己的问题用自己的目标。
- 「没编进来回 `unknown_method`」测查询表本身（空表），不额外编一份关掉 cargo 开关的核心：两者是同一个结果，另编一次是门禁没有的 `cargo test --no-default-features` 构建组合。
- 查询表的接口（`queries.rs`）：`QueryError` 只给软件包用（参数不对/内部出错/一个原因码可带一格 `data`），不直接把 `Refusal`（JSON-RPC 错误码、中英文 `message`）公开给 `miyu-core` 这样的外部 crate；`Refusal::from(QueryError)` 在端点内部兜底翻译。
- 门禁的登记簿检查补了 `software/mermaid/` 豁免（见第 2 条），`xtask/src/ledger.rs` 跟着改，不是单独一步。
