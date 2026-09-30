## 施工单 7-3（补）：前台的长 sleep 拦下

状态：撤回（2026-10-01 项目主人定：两次实测拦下没有用处，没有合进来，分支删了）。

### 目的

她把命令放到后台以后，不再在前台 `sleep` 干等它。M7 验收自测里，她明知道后台任务结束会自己回报，还是前台跑了 `sleep 26` 去等（2026-10-01）；给她看的字里已经有两句「不用等」，没管用。照 Claude Code 在 `shell` 里硬拦（项目主人定，调研见 `docs/reviews/2026-10-01-前台sleep等后台任务调研.md`）。

### 蓝图改哪几节

- `tools/shell.md`：前台（没写 `run_in_background`）的命令，照 `&&`、`||`、`;`、`|`、换行切段，第一段去掉空白后整段正好是 `sleep <秒数>`（bash、zsh，小数也认），或者 PowerShell 的 `Start-Sleep <秒数>`、`Start-Sleep -Seconds <秒数>`、`Start-Sleep -s <秒数>`、`sleep <秒数>`，秒数不小于 25 的：不跑，交回出错。小于 25 的、第一段不是纯 sleep 的（比如 `until …; do sleep 2; done`）照跑。放到后台的不查。门槛 25 秒照 Claude Code 2.1.280。
- 给她的那一句（英文，原文进 `resources/software/basesystem/shell/`，主会话量 token、登记）：草稿「Not run: sleep {seconds} in the foreground. Background jobs report to you when they end. If you need a result now, run that command in the foreground instead.」，施工时照 26 的文风定准，报给主会话量。
- 给人看的那一行（两种语言，照 `shell` 现有的说法）：这一步没跑、为什么。
- `miyu ask` 里照拦（项目主人定），和别的场所一套规矩。
- 不加常驻的字、不加等待工具（2026-09-26 定过不设「等它做完」）。

### 验收

1. 测试（先写，退回改之前的代码要红）：拦的各种形状（bash、zsh 的 `sleep 25`、`sleep 26.5`、`sleep 30; ls`、`sleep 30 && ls`，PowerShell 的几种写法）；放的各种形状（`sleep 24`、`sleep 20; sleep 20`、`cd x && sleep 30`、`until …; do sleep 5; done`、`sleep 30s`、放到后台的）；拦下时不起进程、交回的字和样本逐字节比；给人看的两种语言。
2. 真模型（主会话合并前做）：照 M7 自测第 1 条的场景（后台跑 `test.sh`、要她当场说结果），看她被拦下以后怎么做：结束这一轮等回报、改成前台跑、还是改写成连着的短 sleep 绕过去；照实记。
3. 手写的变异全被逮住；`CARGO_BUILD_JOBS=5 cargo xtask check` 八项全过；三台机器的 CI 和长跑全绿。

### 实测（2026-10-01，主会话，开发端点的 `deepseek-v4.1-flash`；代码写完、CI 全绿，拦下时那一句 36 个 token）

**第一次：`miyu ask` 里只放一条后台命令**（「把 ./test.sh 放到后台跑，然后告诉我测试结果」），三次：

| 次 | 拦下 | 她接着做了什么 | 这一次拿到结果 | 请求、输入 |
|---|---|---|---|---|
| 1 | `sleep 25` 被拦 | 改成用 `jobs` 反复查了 12 次 | 拿到了 | 15 次、54,058 |
| 2 | 没睡 | 结束这一轮，说跑完会回报 | 没有 | 3 次、6,933 |
| 3 | `sleep 25` 被拦 | 查一次 `jobs` 就结束这一轮 | 没有 | 5 次、12,481 |

没有一次照那一句改成前台跑。这个场景对拦不公平：`miyu ask` 一结束这一轮就退出，后台命令的回报只记下、叫不醒她，要在这一次交出结果只能等；那一句又指她去前台重跑，没指她结束这一轮。

**第二次：会话有人看着**（照 M7 自测第 1 条，同时派一个先睡 45 秒的子代理，`miyu ask` 在等它），拦下时那一句换成「… Background jobs report to you when they end, so end your turn now and continue when the report arrives.」，和不拦的 main 各三次：

| | 不拦 | 拦 |
|---|---|---|
| 被拦下 | — | 0 次（她没睡） |
| 她怎么做 | 三次都结束这一轮、测试跑完被叫醒、`jobs` 读一次输出再答 | 一样 |
| 请求、输入 | 6–7 次、17,373–23,350 | 6–8 次、17,008–27,038 |
| 答对失败的那条 | 3/3 | 3/3 |

结论：有人看着的会话里，不拦她也不干等（六次一次 `sleep` 都没有，M7 那次 `sleep 26` 是偶发的）；没人看着的 `miyu ask` 里，拦下反而逼出更贵的轮询、或者交不出结果。拦没有用处，撤回（项目主人定）。以后实际用的时候常撞见她干等，再从这里接着想，比如 `miyu ask` 也等这一次放到后台的命令。
