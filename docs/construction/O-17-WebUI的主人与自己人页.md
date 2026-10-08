## 施工单 O-17：WebUI 的主人与自己人页

状态：已完成（2026-10-08；页面的样子照 `onebot.md` 第二条，项目主人 2026-10-07 过目；O-16 合了，接在它后面）。

### 目的

`miyu-onebot` 的 WebUI 第二页「主人与自己人」：主人对应表 `external.bindings`（哪些 QQ 号的私聊就是本机账号本人）、自己人 `onebot.trusted`（私聊里能叫她、不限流、睡着时私聊也放行），加一行、删一行、保存。做完以后，接 QQ 从头到尾不用手改配置文件（O-16 做了令牌和端口，这一页做剩下的两张表）。分支接在 O-16 后面，O-8、O-16 先合。

### 蓝图改哪几节

`docs/blueprint/onebot.md` 第二条：

- 「怎么走」第 4 条：
  - **主人**：`config.get` 读 `external.bindings` 下的每一格，一行一个号对一个账号；号只收数字，平台前缀由页面照桥的平台名拼（`qq:<号>`）；账号的下拉照核心现有的账号列（施工时查协议里有没有列账号的方法：有就照它列，没有就只有 `admin`，图纸照实写）。保存时照改动 `config.set`：加的、改的写 `external.bindings."qq:<号>" = "<账号>"`，删的恢复默认（去掉这一格）；只写系统配置。
  - **自己人**：`onebot.trusted`，平台身份的列表，一行一个号；保存时整张写回。原写「O-15 先声明这一项」改成「O-17 声明」。
  - 两张表各有一个「保存」，没改动的时候按钮灰着；核心回问题（`config_invalid`、没有改系统配置的权限）照原话说在那一张表下面，表里的东西不丢。
  - 主人那一张表下面折起来一段「号被盗的代价」（18 第三节的三条），给人看的字。
  - 生效：`external.bindings` 核心当场照新的认（`config.md` 那一行的 `now`）；`onebot.trusted` 现在桥还不读（桥接群、算「发的人是谁」时才读），生效时机写 `now`，到时桥照 O-16 的办法当场重读。页面都不提示重启（O-16 补二以后页面上没有重启的提示了）。
- 「样子」：主人与自己人那一张照现在的线框，施工时照实补细节（空表的样子、号重复、号不是数字）。
- 给人看的字：`web/people/*`（名字施工时定）加进 `human/{zh,en,ja}.json`。
- 「守着它的」「施工时定的」补这一步的。

`docs/blueprint/config.md` 配置项表：加 `onebot.trusted` 一行（列表，元素是文字；没有默认值、空表；只能写系统配置；界面在 onebot 那一组）。

`crates/miyu-core/src/settings.rs` 的 `OnebotSettings`：加 `trusted`（照 O-8 的权宜：由核心代桥声明；以后桥的配置整体挪进 `packages/onebot.toml` 的 `[settings]` 时，写成 `type = "list", element = "text", layers = ["system"]`，同一个提交删掉核心这一份，2026-10-08 和核心的主会话定）。

### 要定的（技术细节，照推荐定）

| # | 定什么 | 推荐 | 为什么 | 没选 |
|---|---|---|---|---|
| 1 | 删一个主人怎么写 | `config.set` 恢复默认（去掉这一格） | 协议现成；不另造「删」的方法 | 写成空字 |
| 2 | 自己人怎么存 | 整张列表写回 | 列表是一项，改一个元素也是改这一项 | 一个号一项 |
| 3 | 同一个号在两张表里都有 | 照写，页面在自己人那一行旁边说一句「已经是主人」 | 主人的权限包含自己人的；不拦，免得改表时卡住 | 不让存 |
| 4 | 号的格式 | 只收数字（1 到 20 位），前后的空白去掉；号重复的标出来、不让存 | QQ 号是数字；页面拼前缀，平台无关的规矩在桥那边 | 收任意字 |

### 不做什么

- 群里的管理员（场所规则的 `managers`）：场所页，等群接通。
- 桥照 `onebot.trusted` 认自己人：桥接群那一步（算「发的人是谁」时读它）。
- 桥的配置整体挪进软件包清单：另起一步。

### 验收

1. 先写测试，退回改之前要红：`onebot.trusted` 读得出、只能写系统配置、写错的报问题；真核心加真桥跑一遍，照页面会发的那几条经 `/ws` 发：加一个主人、删一个主人（恢复默认）、整张写回自己人，核心的 `config.get` 读得回；不是管理员的账号改系统配置被拒。页面里的逻辑（号的格式、重复、改动才能存）照 O-16 的办法写薄，没有浏览器里的自动测试，截图和项目主人试一次兜底。
2. 手写变异 10 个左右，全被逮住。
3. 截图看一眼（亮色、暗色、窄屏）。
4. `CARGO_BUILD_JOBS=5 cargo xtask check` 全过；三台机器的 CI 和长跑全绿。
5. 项目主人在浏览器里试：加自己的号做主人，保存；不用手写 `[external.bindings]`，私聊她照样认出是主人。

### 风险

- 改主人对应表要管理员权限：登录的是只有「接平台」能力、不是管理员的账号时，核心会拒；页面照原话说，测试里覆盖。
- 页面逻辑没有浏览器里的自动测试：照 O-16 的办法，逻辑尽量薄、能抽的抽成纯函数，项目主人试一次兜底。

**验收结果（2026-10-08）**

1. 测试先写，退回改之前全红（核心的 `onebot_trusted_is_a_system_list_of_identities` 编译不过：`OnebotSettings` 没有 `trusted`；桥的 5 个：`people.rs` 两个回 `unknown_config_key`，`status.rs`、`no_token.rs` 三处 `/status` 没有 `platform`），改完全过（`miyu-onebot` 58 个、`miyu-core` 的 `settings` 6 个）：
   - `crates/miyu-onebot/tests/people.rs`（真的核心加真的桥，浏览器经桥的 `/ws` 照页面的样子发）：还没设好密码的连接（一次性码）改系统配置回 `setup_first`、带原话、什么都没写；设好以后加一个主人（`external.bindings."qq:10002"`，生效 `now`）、删一个主人（`unset`）、整张写回自己人（`onebot.trusted`，生效 `now`），换一条连接（登录令牌）`config.get` 读得回，来源是系统配置；假 NapCat 发一条 10002 的私聊，进了管理员的会话、`via` 是 `qq:10002`、她的回话发回 10002（页面拼的键和桥拼的平台身份对得上）。另一个：自己人写进个人设置 `config_invalid`（`wrong_layer`），写成一个字、元素是空的字、元素是数，`config_invalid`、问题说在 `onebot.trusted` 上、带一句话，一起发的主人也没写（整条不收）。
   - `crates/miyu-core/tests/settings.rs`：登记的先后多 `onebot.trusted`；它的类型（文字的列表，最多 128 个字）、没有默认值、只能写系统配置、`now`、高级页「QQ 桥」组的列表；读得出、不写是没有；空表照收；写进个人设置 `wrong_layer`，写成一个字、元素是数 `wrong_type`，空的字、129 个字 `bad_format`，128 个字照收。样本照 `MIYU_SAMPLE_WRITE=1` 重写，`config.md` 的样本块跟着改。
   - 改的：`status.rs`、`no_token.rs`（`/status` 多 `platform: "qq"`）。
   - 「不是管理员的账号改系统配置被拒」：核心现在只有管理员一个账号，没有这种账号可测；照实测的是还没设好密码的连接，图纸「施工时定的」第 35 条。
2. 手写变异 12 个，全被逮住：`/status` 不带平台名、平台名写错（`status.rs`、`no_token.rs`）；自己人也能写进个人设置（`settings`、样本、`people.rs`）；元素最多 127 个字、129 个字（`settings`、样本）；生效时机写成下次启动（`settings`、样本、`people.rs`）；界面放进运行日志那一组、控件写成一行字（`settings`）；核心的中文说明少一句（样本）；核心的日文没有这一项（`every_language_names_every_item_and_nothing_more`、`japanese_has_every_sentence_the_files_need`）；桥的日文少一句代价、中文少了「加一个」（`miyu-store` 的 `human_languages`）。页面里的逻辑（号的格式、重复、改动才能存、账号的下拉）没有浏览器里的自动测试，照施工单靠截图和项目主人试一次兜底；截图那一趟的脚本里顺手核了：空表两个「保存」灰着、填了对的号亮、有标着的灰、存主人时自己人没存的三行留着、核心回问题时表里三行留着、写进系统配置的就是页面上的那几行。
3. 截图（无头 Chromium，真的核心和桥，临时数据根，用完停掉）：`/tmp/claude-1000/-home-shorin-Documents-github-miyu-agent-remake/1dce34c0-a651-41a2-97ec-7cae11e86ec6/scratchpad/o17/` 里的 `empty-{light,dark}.png`（两张表空着）、`filled-{light,dark}.png`（号以 0 开头、重复、已经是主人，代价点开）、`saved-{light,dark}.png`（两张表都存好了）、`problem-{light,dark}.png`（系统配置文件手改坏了再存，核心的原话在主人表下面）、`narrow-{light,dark}.png`、`narrow-drawer-dark.png`（窄屏），另有 `connection-light.png`（「连接」页照旧）。
4. `CARGO_BUILD_JOBS=5 cargo xtask check`：八项全过（第一趟就过）。
5. 三台机器的 CI 和长跑全绿（run 1232）。
6. 浏览器里试：项目主人 2026-10-08 让主会话自己看（试 O-16 时说的），主会话看过截图（空表、填了几行、核心回问题、窄屏，亮暗两套）和真核心加真桥的那条测试（照页面的办法加主人、存进去，仿 NapCat 的私聊以这个号发进来认成管理员本人），没有再请项目主人试。
