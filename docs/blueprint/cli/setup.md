## `miyu setup`

### 是什么

第一次接入模型（施工 8-11，`models.md` 第七条，`15-模型与供应商.md` 第七节）：找现成的 key 和本机的模型服务，没有的从目录里搜一家、贴 key，试通了，选主对话的模型，写进配置。主程序里最基本的一份：一行行问、敲数字、贴 key 不回显，参数能跳过对应的一步（2026-10-01 项目主人看过、定了）。全屏的引导做在各个头里，照同一组方法（施工方案 M9 那一段第 3 条）。

它只是协议的客户端（`22-命令行.md` O5）：`model.list`、`provider.detect`、`provider.catalog`、`provider.test`、`secret.set`、`config.get`、`config.set`。key 从不写在命令行上，也从不印出来。`miyu ask` 没有模型、又在终端里时，先走它（`cli/ask.md` 第 2 条）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `setup`，换上自己那一页帮助 |
| `crates/miyu-cli/src/setup.rs` | 参数（`Setup`）、连核心、握手、照先后走一遍（`setup_on`，测试照它走）；`miyu ask` 用的「有没有模型、没有就走一遍」（`model_ready_on`） |
| `crates/miyu-cli/src/setup/pick.rs` | 找到的、搜到的、模型的编号表，敲的字认成哪一个，目录里的编号在配置里写成什么 |
| `crates/miyu-cli/src/setup/flow.rs` | 一步步：拿 key、试、试不通回到上一步、存、写 |
| `crates/miyu-cli/src/setup/choose.rs` | 选一家：找到的、搜目录的、`--provider` 写的，`--env` 盖过找到的 |
| `crates/miyu-cli/src/setup/model.rs` | 选主对话的模型 |
| `crates/miyu-cli/tests/support/onboarding.rs` | 测试用：带模型资料的核心、照剧本回的假终端 |
| `crates/miyu-cli/src/config/console.rs` | 人那一头 `Console`（和 `miyu config`、`miyu login` 共用） |
| `crates/miyu-cli/src/language/setup.rs` | 给人看的字 |
| `crates/miyu-cli/src/help/{zh,en}/setup.txt` | 帮助页 |

### 对外的样子

| 参数 | 做什么 |
|---|---|
| `--provider <编号>` | 跳过选一家：用目录、档案里的这一家（`provider.catalog` 的 `id`） |
| `--env <变量>` | 跳过贴 key：key 照核心环境里的这个变量取，写成 `{ env = "<变量>" }` |
| `--model <模型>` | 跳过选模型：试它、用它（这一家的模型名，不带 `<供应商>/`） |

- 用到的环境变量：`MIYU_HOME`、`NO_COLOR`，比核心看不看得到 key 时看自己的环境（只看名字有没有设）；界面语言照 `cli/main.md`。

### 怎么走

1. **连核心以前**：标准输入或标准错误不是终端、又没写 `--provider` 的：说「要在终端里选，或者写 miyu setup --provider <编号>」，退出码 2，不拉起核心。
2. **连核心、握手**：照 `miyu config`（没在跑的拉起来）。之后给人看的字照回应的 `language`。
3. **选一家**（没写 `--provider`）：`provider.detect`。
   1. 头的环境里设了（去掉前后空白不是空的）、`looked_for` 里有、`keys` 里没有的变量：先说一段灰字，哪几个变量核心看不到，怎么让它看到（`models.md` 第七条第 2 条）。
   2. 找到的（`keys`、`local`）都列出来，`keys` 在前、`local` 在后：标准错误上一行头「找到这些现成的：」，一个一行 `  编号  名字  在哪`（`环境变量 <变量>`、`本机 <地址>，<n> 个模型`），已经配好的后面接「，已经配好（<编号>）」。用不了的（`supported` 是假的）不编号、灰字、后面接用不了的原因。最后一行 `  0  都不要，从目录里找一家`。问「选一个编号：」。
   3. 敲的是列出的编号：就是那一家。`0`：搜目录。直接回车、读到头：说「没选」，退出码 1。别的：说「<敲的> 不是列出的编号」，再问。
   4. 一个都没找到（用不了的也没有）：说「没找到现成的 key 和本机的模型服务。」，接着搜目录。
4. **搜目录**：问「搜一家供应商（编号或者名字里的一截，直接回车列出全部）：」，`provider.catalog` 带 `query`（空的不带）、`limit` 20。一家一行 `  编号  名字  目录里的编号`，用不了的不编号、灰字、接原因。正好 20 家的，再说一句「只列了前 20 家，搜得细一点能看到别的。」。一家都没有：说「没有对上的。」再问搜什么。有的问「选一个编号，或者再搜一次：」：敲的是列出的编号，就是那一家；直接回车、读到头：「没选」，退出码 1；别的当成新的一截再搜。
5. **`--provider`**：`provider.catalog` 带 `query` 是它，取 `id` 一模一样、能用的那一家。没有的：说「目录里没有能用的 <编号>」，退出码 2。`provider.detect` 里有它的 key 的，照找到的那个变量用（第一个）。
6. **key**：
   - 选的是找到的 key：引用那个变量 `{ env = … }`，不复制。写了 `--env` 的照 `--env`。
   - 选的是本机的服务、搜到的地址在本机的一家（`provider.catalog` 的 `local`）：不要 key。
   - 选的是已经配好的那一家：照配置，不问。
   - 别的：标准输入是终端的，问「粘贴 <编号> 的 key（不显示）：」，关掉回显读一行；不是终端的，整份读标准输入。去掉前后空白是空的：说「没收到 key」，退出码 1。
7. **试**：说「试一下 <名字>……」，`provider.test`：配好了的带 `provider`；别的带 `candidate`，`catalog` 是这一家的编号，`key` 照上一步（贴的照 `{value}` 交）；写了 `--model` 的带 `model`。
   - 成了：灰字「· 通了：试的 <模型>，<n> 毫秒收到第一个字。」；`listed` 是 `catalog` 的再一行灰字「· 供应商列不出模型，下面照 models.dev 的目录列。」。
   - 没成：说「不通（<哪一步>）：<分类>：<原话>」（没有原话的不写第二个冒号以后；HTTP 状态在原话里，驱动写的 `HTTP 401: …`，和 `miyu ask` 出错那一行一样）。哪一步：`config` 是「配置」，`list` 是「列模型」，`request` 是「发请求」；分类照 `cli/ask.md` 出错那一行的说法。在终端里的回到上一步：贴的 key 回到贴 key，别的回到第 3 条选一家（写了 `--provider` 的没有上一步，退出码 1）。不在终端里的，退出码 1。
8. **存 key**（贴的 key 试通了）：`secret.set`，`name` 是这一家在配置里的编号（第 10 条）。成了：灰字照 `miyu login` 说「· <编号> 的 key 存好了」或「· 换掉了 <编号> 的 key」。被拒绝的：印核心的原话，退出码 1。
9. **选模型**（没写 `--model`）：`provider.test` 列出的模型，试的那一个排第一、后面接「（推荐）」，别的照列出的先后。标准错误上一行头「选主对话的模型：」，一个一行 `  编号  模型`，最多列 20 个；多的再说一句「还有 <n> 个，敲名字也行。」。问「选一个编号，直接回车用推荐的：」。敲的是编号、列出来的模型名（列了的、没列的都算）：就是它；直接回车：推荐的；读到头：「没选」，退出码 1；别的：说「<敲的> 不在列表里」，再问。不在终端里的不问，用推荐的。
10. **写**：`config.set` 写系统配置：`providers.<编号>.keys`（找到的、`--env` 的是 `[{ env }]`，贴的是 `[{ secret = "<编号>" }]`，本机的是 `[]`）和 `models.chat` 是 `<编号>/<模型>`；已经配好的那一家只写 `models.chat`（编号照配置里的）。配置里一个池都没有的，同一次一起写三个预设的池（施工 8-8 补，`models.md` 第七条第 5 条第 7 款）：写之前问一次 `config.get`（不带 `cwd`、`keys`），`items` 里没有 `pools.` 开头的键才写；`pools.lite`、`pools.standard`、`pools.flagship` 各写 `models = []`、`subagent = true`，不带说明。屏幕上照旧只说 `models.chat` 那一行。配置里的编号就是目录里的编号；目录里的编号不合「路径里的名字」写法的（`302ai`、`wafer.ai`），别的字换成 `-`、不是字母开头的前面加 `p-`（`p-302ai`、`wafer-ai`），另写 `catalog = "<目录里的编号>"`。成了：说「写好了：models.chat = <编号>/<模型>」，退出码 0。被拒绝的：印核心的原话，退出码 1。
11. **核心断开**：说「核心断开了」，退出码 1。

### 样子

找到了一个 key 和一家本机的服务，选了 key（一行一行都在标准错误上）：

```text
$ miyu setup
找到这些现成的：
  1  DeepSeek   环境变量 DEEPSEEK_API_KEY
  2  LMStudio   本机 http://127.0.0.1:1234/v1，1 个模型
     Anthropic  环境变量 ANTHROPIC_API_KEY，用不了：还没有 anthropic 驱动
  0  都不要，从目录里找一家
选一个编号：1
试一下 DeepSeek……
· 通了：试的 deepseek-flash，812 毫秒收到第一个字。
选主对话的模型：
  1  deepseek-flash（推荐）
  2  deepseek-v4-pro
选一个编号，直接回车用推荐的：
写好了：models.chat = deepseek/deepseek-flash
```

什么都没找到，搜目录、贴 key，第一次贴错了：

```text
$ miyu setup
没找到现成的 key 和本机的模型服务。
搜一家供应商（编号或者名字里的一截，直接回车列出全部）：deep
  1  DeepSeek    deepseek
     Deep Infra  deepinfra  用不了：认不出它的接口
选一个编号，或者再搜一次：1
粘贴 deepseek 的 key（不显示）：
试一下 DeepSeek……
不通（发请求）：认证失败：HTTP 401: Authentication Fails (no such user)
粘贴 deepseek 的 key（不显示）：
试一下 DeepSeek……
· 通了：试的 deepseek-flash，790 毫秒收到第一个字。
· deepseek 的 key 存好了
选主对话的模型：
  1  deepseek-flash（推荐）
  2  deepseek-v4-pro
选一个编号，直接回车用推荐的：2
写好了：models.chat = deepseek/deepseek-v4-pro
```

全写在参数上，从管道贴 key（脚本用）：

```text
$ echo "$KEY" | miyu setup --provider deepseek --model deepseek-flash
试一下 DeepSeek……
· 通了：试的 deepseek-flash，812 毫秒收到第一个字。
· deepseek 的 key 存好了
写好了：models.chat = deepseek/deepseek-flash
```

核心是别的终端拉起的，看不到这个终端里后来设的 key（灰字）：

```text
· 这个终端里设了 OPENAI_API_KEY，核心看不到：核心是别处拉起的，看不到后来设的环境变量。等核心空闲了自己退出（没有界面连着、没有在跑的活），再在这个终端里运行 miyu setup；或者从目录里选这一家、把 key 贴进来。
```

样本 `crates/miyu-cli/src/help/zh/setup.txt`（帮助页，中文）：

```text
用法：miyu setup [选项]

接上第一个模型：找环境变量里的 key 和本机的模型服务，没有的从目录里
搜一家、贴 key；试通了，选主对话的模型，写进系统配置。

选项：
      --provider <编号>  不选了，用目录里的这一家
      --env <变量>       不贴了，key 照这个环境变量取
      --model <模型>     不选了，用这个模型
  -h, --help             印帮助

不在终端里的要写 --provider，key 从管道进来：
  echo "$KEY" | miyu setup --provider deepseek
```

样本 `crates/miyu-cli/src/help/en/setup.txt`（帮助页，英文）：

```text
Usage: miyu setup [options]

Connect the first model: look for keys in environment variables and model
services on this machine, or find a provider in the catalog and paste its
key; try it, pick the model for chat, and write it to the system config.

Options:
      --provider <id>   Skip picking: use this provider from the catalog
      --env <var>       Skip pasting: take the key from this variable
      --model <model>   Skip picking: use this model
  -h, --help            Print help

Outside a terminal, give --provider and pipe the key in:
  echo "$KEY" | miyu setup --provider deepseek
```

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 写好了 |
| 1 | 没选；没收到 key；不在终端里、试不通；写了 `--provider`、试不通；核心拒绝了；连不上核心、数据根的错；核心断开 |
| 2 | 参数不对：不在终端里又没写 `--provider`；`--provider` 不是目录里能用的一家；clap 认不出的 |

### 给人看的字

| 什么时候 | 中文 | 英文 |
|---|---|---|
| 找到的头一行 | 找到这些现成的： | Found these ready to use: |
| 找到的 key | 环境变量 {env} | environment variable {env} |
| 找到的本机服务 | 本机 {base_url}，{n} 个模型 | this machine {base_url}, {n} models |
| 已经配好 | ，已经配好（{id}） | , already set up ({id}) |
| 用不了 | ，用不了：{原因}（目录的表里不带开头的逗号） | , cannot use: {why} |
| 用不了的原因 | 认不出它的接口；目录里没有它的地址；还没有 {driver} 驱动 | its API is not known; the catalog has no address for it; no {driver} driver yet |
| 都不要 | 都不要，从目录里找一家 | None of these: find one in the catalog |
| 问编号 | 选一个编号： | Pick a number: |
| 编号不对 | {敲的} 不是列出的编号 | {typed} is not a listed number |
| 什么都没找到 | 没找到现成的 key 和本机的模型服务。 | No key or local model service found. |
| 核心看不到 | 见「样子」 | · {vars} is set in this terminal, but the core cannot see it: the core was started elsewhere and does not see variables set later. Wait until the core is idle and exits by itself (no interface connected, nothing running), then run miyu setup in this terminal again; or pick that provider from the catalog and paste the key. |
| 搜 | 搜一家供应商（编号或者名字里的一截，直接回车列出全部）： | Search for a provider (part of its id or name; Enter lists all): |
| 没对上 | 没有对上的。 | Nothing matches. |
| 列满了 | 只列了前 {n} 家，搜得细一点能看到别的。 | Only the first {n} are listed; search more narrowly to see others. |
| 选或者再搜 | 选一个编号，或者再搜一次： | Pick a number, or search again: |
| 目录里没有 | 目录里没有能用的 {id} | No usable provider {id} in the catalog |
| 要终端 | 要在终端里选，或者写 miyu setup --provider <编号> | Pick in a terminal, or run miyu setup --provider <id> |
| 试 | 试一下 {name}…… | Trying {name}… |
| 通了 | · 通了：试的 {model}，{ms} 毫秒收到第一个字。 | · It works: tried {model}, first token in {ms} ms. |
| 照目录列 | · 供应商列不出模型，下面照 models.dev 的目录列。 | · The provider listed no models; the list below is from the models.dev catalog. |
| 不通 | 不通（{哪一步}）：{分类}：{原话} | Did not work ({stage}): {class}: {message} |
| 哪一步 | 配置；列模型；发请求 | config; listing models; request |
| 选模型的头一行 | 选主对话的模型： | Pick the model for chat: |
| 推荐 | （推荐） | (recommended) |
| 还有 | 还有 {n} 个，敲名字也行。 | {n} more; you can type a name. |
| 问模型 | 选一个编号，直接回车用推荐的： | Pick a number, or press Enter for the recommended one: |
| 不在列表里 | {敲的} 不在列表里 | {typed} is not in the list |
| 写好了 | 写好了：models.chat = {ref} | Done: models.chat = {ref} |
| `miyu ask` 先走 setup | 还没有模型，先接上一个。 | No model is set up yet. Let's connect one first. |

「没选」「没收到 key」「· {名字} 的 key 存好了」「· 换掉了 {名字} 的 key」「粘贴 {名字} 的 key（不显示）：」照 `miyu login` 的（`config.md`「给人看的字」）。灰字的几行开头是「· 」，和 `miyu ask`、`miyu login` 的灰字一样。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/src/setup/pick/tests.rs` | 编号表照样子（对齐、用不了的不编号且是灰的、最后一行 `0` 只和编号对齐）；敲的编号、`0`、别的字、超出的、直接回车、读到头；目录里的编号在配置里写成什么（`p-302ai`、`wafer-ai`） |
| `crates/miyu-cli/tests/setup.rs` | 空的配置里写出三个预设的池（空成员、开关开着），已经有池的不写（施工 8-8 补）；在进程里起核心（真目录裁出来的一份，本机的服务和供应商是假服务器）、假终端照剧本回，走一遍（Unix 和 Windows 一样跑）：环境变量里的 key 只引用不复制（配置里是 `{ env }`，密钥文件里没有）；搜目录、贴的 key 先试、通了存成密钥；试不通回到上一步（贴错一次再贴对）；本机的服务 `keys = []`；已经配好的只写 `models.chat`；核心看不到的变量说清是哪个、不复制；写出的配置对；推荐的排第一、敲编号换一个；没选、没收到 key；屏幕上、日志里从头到尾没有 key；`miyu ask` 没模型：终端里先走 setup 再说，不是终端的退出码 5、不造会话 |
| `crates/miyu-cli/tests/setup_skip.rs` | 每一步都能用参数跳过：`--provider`、`--env`、`--model` 全写的不问一句；不在终端里的从管道读 key、用推荐的模型；不在终端里又没写 `--provider` 的退出码 2、不连核心；`--provider` 不是目录里能用的退出码 2；不在终端里试不通退出码 1 |
| `crates/miyu-cli/src/help/tests.rs` | 两页列的选项和程序真有的对得上，最宽 80 列 |
| `crates/miyu/tests/setup.rs` | 真跑主程序：帮助页跟着界面语言；不在终端里又没写 `--provider` 退出码 2、不拉起核心 |
| `crates/miyu/tests/ask.rs` | 真跑 `miyu ask`：没配模型、不在终端里的退出码 5、不造会话 |

### 出处

- `models.md` 第七条（第一次接入）、「协议」的 `provider.*`、「定的」第 7 条。
- `15-模型与供应商.md` 第七节。`22-命令行.md` 第三节「还没配模型时」、第五节。
- `cli/login.md`（贴 key、`Console`）、`cli/config.md`（连核心、握手）。

### 还没有的

- 接目录里没有的中转站（自己写驱动、地址）：现在照旧写配置（`miyu config edit --system`）。
- 让核心空闲时重启：要一个新的协议方法。
- 借已经登录的 agent CLI 的订阅：以后再说。
- 选看图的模型、给池填成员：现在照旧写配置（`miyu config edit`）。
- 贴 key 时按 Ctrl+C 恢复回显：随 8-5 补，和 `miyu login` 一起。

### 施工时定的

图纸没画到的几处（2026-10-01 施工时定，照推荐）：

| 定了什么 | 为什么 | 别的选法 |
|---|---|---|
| 参数三个：`--provider`、`--env`、`--model`；key 不上命令行（会进 shell 的历史），不在终端里的照 `miyu login` 从管道读 | 图纸「参数能跳过对应的一步」；`miyu login` 的规矩 | `--key <值>`：进 shell 的历史 |
| 不在终端里不问：没写 `--provider` 的退出码 2；模型用推荐的（等于直接回车）；试不通退出码 1 | 脚本能一行写完；能默认的不报错 | 不在终端里一律退出码 2：`--model` 不写也得写 |
| 试不通回到上一步：贴的 key 回到贴 key，别的回到选一家；写了 `--provider` 的、不在终端里的没有上一步，退出码 1。直接回车、读到头是出口（「没选」「没收到 key」） | 图纸「回到上一步」；不能卡在一个问题上出不来 | 一律退出：贴错一次要从头来 |
| 编号表最多列 20 个模型、搜目录每次 20 家，多的说一句；模型也能敲名字 | 一家几百个模型的（中转站）一屏放不下 | 全列：几百行 |
| 用不了的照样列，不编号、灰字、接原因 | 图纸「别的也列、标出来，人知道为什么选不了」 | 不列：找到了 key 却不知道为什么不见了 |
| 「核心看不到」只说等核心空闲退出、或者从目录里贴 key | 让核心空闲时重启要一个新的协议方法，不在这一步 | 加一个 `core.restart`：多一个方法，不在图纸里 |
| 写系统配置 | 第一次接入的是管理员；模型这一块只能写系统、个人两层，`xtask dev-home` 也写系统配置 | 写个人设置 |
| 目录里的编号不合「路径里的名字」写法的（`302ai`、`wafer.ai`）换一个配置里的编号（`p-302ai`、`wafer-ai`），另写 `catalog`；密钥也用这个名字 | 配置的编号、密钥的名字都只收小写字母开头的那种，照原样写进去 `config.set` 不收 | 这两家不让选：目录里有、选不了，说不清为什么 |
| 不通的那一行不另写 HTTP 状态：驱动的原话里有（`HTTP 401: …`） | 写两遍；和 `miyu ask` 出错那一行一样 | 照 `status` 另写一段：`HTTP 401：HTTP 401: …` |
| 用不了的原因照 `driver`、`base_url` 认：`driver` 是 `null` 的认不出接口，`base_url` 是 `null` 的没有地址，都有的是驱动还没有；`provider.detect` 的 `keys` 没有地址那一格，当有地址 | 核心不另交原因；现在用不了的几家（`anthropic`、`openai`）`driver` 都有 | `provider.catalog` 另交一格原因：这一步只有这一处要它 |
