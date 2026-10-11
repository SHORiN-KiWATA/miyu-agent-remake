## 施工单 V-3：量尺进 CI

状态：合了（2026-10-11，dbab86dc，run 1741）。23 F4 定了「回归闸门钉住一个大会话，每次改动都跑」；0.0.1 差距清单核心那一节的最后一项。

### 这一步做什么

1. CI 加一项 `perf`（只在 Linux 上，预算照 Linux 定的）：`cargo xtask perf --idle-wait 0 --gate 1.5`，照出厂参数量一万条事件的大会话，不等闲够了退下（那一项没有预算，等它要多三分多钟）。表贴进这一次跑的摘要，也印进日志（摘要经接口取不出来）。编 release 加量一共七八分钟，和别的几项并着跑。
2. 量尺加 `--gate <倍数>`（`report/gate.rs`）：表写好以后，有预算的哪一行时间超过预算的这么多倍、内存超过预算，一行一句报出来，退出码 1。表和闸门照同一份「有预算的几行」（`rows::checks`）。
3. 倍数照 GitHub 的机器实测定：第一次只出表（V-3 上）量出重开大会话 p95 128.8 ms，查出重开时日志读了三遍，另做 V-2 三补修了；修了以后重开 54.4 ms，别的都远在预算里，定 1.5 倍。
4. 图纸 `perf.md`「闸门」一节、参数表、守着它的；23 F4 写上做了。

### 量到的（GitHub 的机器：4 个逻辑核的 EPYC 7763）

| 项目 | V-3 上（修以前） | V-2 三补以后 | 预算 |
|---|---|---|---|
| 重启后打开大会话 p95 | 128.8 ms | 54.4 ms | 60 ms |
| 一次请求的投影 p95 | 2.6 ms | 2.6 ms | 5 ms |
| 从头投影 p95 | 5.0 ms | 4.8 ms | 60 ms |
| 追加一条并同步 p99 | 0.5 ms | 0.4 ms | 20 ms |
| 核心空闲 PSS | 27.3 MB | 27.1 MB | 30 MB |
| 每多一个活动会话 | 0.96 MB | 1.03 MB | 5 MB |

### 这一天撞见的偶发红（都在等异步的测试上、负载高时，和这一步的改动无关，复跑都过，再红就专门查）

- `preset_background::interrupting_the_parent_stops_its_foreground_subagent`：GitHub 的 Linux 上等子代理报回来超了六十秒（V-2 三补第一趟 CI）。
- `extensions::one_that_never_says_hello_fails_too`：GitHub 的 Windows 上停下的原因是 `config_error`，该是 `failed_repeatedly`（V-2 三补第二趟 CI）。
- `followed::after_a_restart_her_next_words_reach_the_group_without_anyone_speaking`（`miyu-onebot`）：本机整套门禁时十秒内没等到桥调动作；单跑 0.3 秒。

### 还没做

- 23 F4 的前一半：核心自己定时采样内存、写成 JSONL。

### 验收

- `miyu-perf` 的单测：闸门只报超了倍数的那几行、内存照预算拦、没量的不算、没写倍数的不拦；`--gate` 要正数。变异：内存跟着倍数放宽、时间不照倍数、都过了也报错，各有测试报红。
- CI 的 `perf` 这一项绿，摘要里有表。
- `cargo xtask check` 八项；三台 CI。
