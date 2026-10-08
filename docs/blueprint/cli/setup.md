## `miyu setup`

### 是什么

第一次接入模型（施工 8-11，`models.md` 第七条，`15-模型与供应商.md` 第七节）：从常用的几家、本机跑着的服务里选一家，或者自定义地址和接口协议；贴 key（环境变量里找到了的不用贴），试通了，选主对话的模型，写进配置。主程序里最基本的一份：一行行问、敲数字、贴 key 不回显，参数能跳过对应的一步（2026-10-01 项目主人看过、定了；选一家那一步 2026-10-08 改成常用的几家加自定义，施工 8-11 再补）。全屏的引导做在各个头里，照同一组方法（施工方案 M9 那一段第 3 条）。

它只是协议的客户端（`22-命令行.md` O5）：`model.list`、`provider.detect`、`provider.catalog`、`provider.test`、`secret.set`、`config.get`、`config.set`。key 从不写在命令行上，也从不印出来。`miyu ask` 没有模型、又在终端里时，先走它（`cli/ask.md` 第 2 条）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `setup`，换上自己那一页帮助 |
| `crates/miyu-cli/src/setup.rs` | 参数（`Setup`）、连核心、握手、照先后走一遍（`setup_on`，测试照它走）；`miyu ask` 用的「有没有模型、没有就走一遍」（`model_ready_on`） |
| `crates/miyu-cli/src/setup/pick.rs` | 供应商、接口协议、模型的编号表，敲的字认成哪一个，目录里的编号在配置里写成什么 |
| `crates/miyu-cli/src/setup/flow.rs` | 一步步：拿 key、试、取不到模型列表的问模型名、试不通回到上一步、存、写 |
| `crates/miyu-cli/src/setup/choose.rs` | 选一家：常用的几家、本机的服务、`--provider` 写的，`--env` 盖过找到的 |
| `crates/miyu-cli/src/setup/custom.rs` | 自定义的一家：问地址、接口协议，照主机名起配置里的编号（施工 8-11 再补） |
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
3. **选一家**（没写 `--provider`）：`provider.detect`、`provider.catalog {"featured": true}`（施工 8-11 再补，2026-10-08 项目主人定）。
   1. 头的环境里设了（去掉前后空白不是空的）、`looked_for` 里有、`keys` 里没有的变量：先说一段灰字，哪几个变量核心看不到，怎么让它看到（`models.md` 第七条第 2 条）。
   2. 标准错误上一行头「选一家：」，一家一行 `  编号  名字  标记`：先是常用的几家，照 `featured` 的先后（配置里已经有的标「已配好」，`provider.detect` 的 `keys` 里有它、能用的标「已找到 key」；用不了的不编号、灰字、接用不了的原因）；再是本机跑着的服务（`local`，标「本机 <地址>」，已经配好的再接「，已配好」）；最后一行「自定义」。问「选一个编号：」。
   3. 敲的是列出的编号：就是那一家，「自定义」接第 4 条。直接回车、读到头：说「没选」，退出码 1。别的（`0` 也算）：说「<敲的> 不是列出的编号」，再问。
4. **自定义**（施工 8-11 再补）：
   1. 问「Base URL：」，去掉前后空白和末尾的 `/`。不是 `http://`、`https://` 开头的：说「要以 http:// 或 https:// 开头」，再问。直接回车、读到头：「没选」，退出码 1。
   2. 标准错误上一行头「接口协议：」，列三行：`OpenAI 兼容`、`Anthropic`、`OpenAI Responses`（驱动 `openai-chat`、`anthropic`、`openai-responses`，核心现在会说的三种）。问「选一个编号：」，认法同第 3 条。
   3. 名字是地址的主机名。配置里的编号照主机名起：本机的（`localhost`、IP）是 `local`；别的取倒数第二段（`api.example.com` 是 `example`），倒数第二段是通用的二级域名（`co`、`com`、`net`、`org`、`edu`、`gov`、`ac`）的再往前一段（`api.example.co.uk` 是 `example`）；写法照第 10 条改（`302.ai` 是 `p-302`）。试通了以后，配置里已经有这个编号的往后加 `-2`、`-3`。
5. **`--provider`**：`provider.catalog` 带 `query` 是它，取 `id` 一模一样、能用的那一家。没有的：说「目录里没有能用的 <编号>」，退出码 2。`provider.detect` 里有它的 key 的，照找到的那个变量用（第一个）。
6. **key**：
   - 选的是找到的 key：引用那个变量 `{ env = … }`，不复制。写了 `--env` 的照 `--env`。
   - 选的是本机的服务、搜到的地址在本机的一家（`provider.catalog` 的 `local`）：不要 key。
   - 选的是已经配好的那一家：照配置，不问。
   - 自定义的：问「Key（不显示，可以空）：」，读法同下一条；去掉前后空白是空的是不要 key（本机的服务、不要 key 的中转）。
   - 别的：标准输入是终端的，问「粘贴 <编号> 的 key（不显示）：」，关掉回显读一行（和 `miyu login` 同一条：自己管终端的设置，按了 `Ctrl+C`、或者还没贴一个字时按了 `Ctrl+D`，说「没存，取消了」，退出码 130，整个 `miyu setup` 照取消办，配置一个字都不写，施工 8-5 补）；不是终端的，整份读标准输入，没有取消这一条。去掉前后空白是空的：说「没收到 key」，退出码 1。
7. **试**：说「试一下 <名字>……」，`provider.test`：配好了的带 `provider`；自定义的带 `candidate`，`driver`、`base_url` 是填的；别的带 `candidate`，`catalog` 是这一家的编号；`key` 照上一步（贴的照 `{value}` 交，空的不带）；写了 `--model` 的带 `model`。
   - 列不出模型（`stage` 是 `list`）、没写 `--model`、在终端里的（施工 8-11 再补）：说「未获取到模型列表」，问「模型名：」，拿它（去掉前后空白）带 `model` 再试一次。直接回车、读到头：「没选」，退出码 1。
   - 成了：灰字「· 通了：试的 <模型>，<n> 毫秒收到第一个字。」；`listed` 是 `catalog` 的再一行灰字「· 供应商列不出模型，下面照 models.dev 的目录列。」。
   - 没成：说「不通（<哪一步>）：<分类>：<原话>」（没有原话的不写第二个冒号以后；HTTP 状态在原话里，驱动写的 `HTTP 401: …`，和 `miyu ask` 出错那一行一样）。哪一步：`config` 是「配置」，`list` 是「列模型」，`request` 是「发请求」；分类照 `cli/ask.md` 出错那一行的说法。在终端里的回到上一步：贴的 key（自定义的也是）回到贴 key，别的回到第 3 条选一家（写了 `--provider` 的没有上一步，退出码 1）。不在终端里的，退出码 1。
8. **存 key**（贴的 key 试通了）：`secret.set`，`name` 是这一家在配置里的编号（第 10 条）。成了：灰字照 `miyu login` 说「· <编号> 的 key 存好了」或「· 换掉了 <编号> 的 key」。被拒绝的：印核心的原话，退出码 1。
9. **选模型**（没写 `--model`、没在第 7 条填模型名）：`provider.test` 列出的模型，试的那一个排第一、后面接「（推荐）」，别的照列出的先后。标准错误上一行头「选主对话的模型：」，一个一行 `  编号  模型`，最多列 20 个；多的再说一句「还有 <n> 个，敲名字也行。」。问「选一个编号，直接回车用推荐的：」。敲的是编号、列出来的模型名（列了的、没列的都算）：就是它；直接回车：推荐的；读到头：「没选」，退出码 1；别的：说「<敲的> 不在列表里」，再问。不在终端里的不问，用推荐的。
10. **写**：`config.set` 写系统配置：`providers.<编号>.key`（施工 8-25：一家一个；找到的、`--env` 的是 `{ env }`，贴的是 `{ secret = "<编号>" }`；本机的服务不要 key，写 `providers.<编号>.local = true`，这一家因此算配好了）和 `models.chat` 是 `<编号>/<模型>`；自定义的另写 `driver`、`base_url`，贴了 key 的写 `key = { secret = "<编号>" }`、没贴的不写；已经配好的那一家只写 `models.chat`（编号照配置里的）。配置里一个池都没有的，同一次一起写三个预设的池（施工 8-8 补，`models.md` 第七条第 5 条第 7 款）：写之前问一次 `config.get`（不带 `cwd`、`keys`），`items` 里没有 `pools.` 开头的键才写；`pools.lite`、`pools.standard`、`pools.flagship` 各写 `models = []`、`subagent = true`，不带说明。屏幕上照旧只说 `models.chat` 那一行。配置里的编号就是目录里的编号；目录里的编号不合「路径里的名字」写法的（`302ai`、`wafer.ai`），别的字换成 `-`、不是字母开头的前面加 `p-`（`p-302ai`、`wafer-ai`），另写 `catalog = "<目录里的编号>"`。成了：说「写好了：models.chat = <编号>/<模型>」，退出码 0。被拒绝的：印核心的原话，退出码 1。
11. **核心断开**：说「核心断开了」，退出码 1。

### 样子

环境变量里有 DeepSeek 的 key，本机跑着 LM Studio，选 DeepSeek（一行一行都在标准错误上）：

```text
$ miyu setup
选一家：
  1   DeepSeek      已找到 key
  2   OpenAI
  3   Anthropic
  4   OpenRouter
  5   Kimi
  6   智谱 GLM
  7   通义千问
  8   opencode Zen
  9   LMStudio      本机 http://127.0.0.1:1234/v1
  10  自定义
选一个编号：1
试一下 DeepSeek……
· 通了：试的 deepseek-flash，812 毫秒收到第一个字。
选主对话的模型：
  1  deepseek-flash（推荐）
  2  deepseek-v4-pro
选一个编号，直接回车用推荐的：
写好了：models.chat = deepseek/deepseek-flash
```

自定义一家中转（2026-10-08 照真的中转跑过一遍，地址换成了例子）：

```text
$ miyu setup
选一家：
  1  DeepSeek
  …
  8  opencode Zen
  9  自定义
选一个编号：9
Base URL：https://api.example.com/v1/
接口协议：
  1  OpenAI 兼容
  2  Anthropic
  3  OpenAI Responses
选一个编号：1
Key（不显示，可以空）：
试一下 api.example.com……
· 通了：试的 deepseek-v4.1-flash，1092 毫秒收到第一个字。
· example 的 key 存好了
选主对话的模型：
  1  deepseek-v4.1-flash（推荐）
  2  glm-5.3-flash
选一个编号，直接回车用推荐的：
写好了：models.chat = example/deepseek-v4.1-flash
```

写进系统配置的：

```toml
[providers.example]
driver = "openai-chat"
base_url = "https://api.example.com/v1"
key = { secret = "example" }

[models]
chat = "example/deepseek-v4.1-flash"
```

列不出模型的：

```text
Key（不显示，可以空）：
试一下 api.example.com……
未获取到模型列表
模型名：my-model
试一下 api.example.com……
· 通了：试的 my-model，640 毫秒收到第一个字。
写好了：models.chat = example/my-model
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
· 核心未读到 OPENAI_API_KEY（核心启动后才设置）。核心空闲退出后重新运行 miyu setup，或选这一家贴 key。
```

样本 `crates/miyu-cli/src/help/zh/setup.txt`（帮助页，中文）：

```text
用法：miyu setup [选项]

接上第一个模型：从常用的几家、本机的模型服务里选一家，或者自定义地址
和接口协议；贴 key，试通了，选主对话的模型，写进系统配置。

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

Connect the first model: pick a common provider, a model service on this
machine, or a custom address and API protocol; paste the key, try it, pick
the model for chat, and write it to the system config.

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
| 130 | 贴 key 时取消了：按了 `Ctrl+C`，或者空行按了 `Ctrl+D`（施工 8-5 补） |

### 给人看的字

| 什么时候 | 中文 | 英文 |
|---|---|---|
| 选一家的头一行 | 选一家： | Pick a provider: |
| 找到了 key | 已找到 key | key found |
| 已经配好 | 已配好（本机的接在地址后面：，已配好） | set up |
| 本机的服务 | 本机 {base_url} | local {base_url} |
| 自定义 | 自定义 | Custom |
| 用不了 | 用不了：{原因} | cannot use: {why} |
| 用不了的原因 | 认不出它的接口；目录里没有它的地址；还没有 {driver} 驱动 | its API is not known; the catalog has no address for it; no {driver} driver yet |
| 问编号 | 选一个编号： | Pick a number: |
| 编号不对 | {敲的} 不是列出的编号 | {typed} is not a listed number |
| 核心看不到 | 见「样子」（2026-10-08 项目主人定改短：照软件的说法） | · The core has not read {vars} (set after the core started). Run miyu setup again once the core exits when idle, or pick that provider and paste the key. |
| 问地址 | Base URL： | Base URL: |
| 地址不对 | 要以 http:// 或 https:// 开头 | Must start with http:// or https:// |
| 接口协议的头一行 | 接口协议： | API protocol: |
| 三种接口协议 | OpenAI 兼容；Anthropic；OpenAI Responses | OpenAI compatible; Anthropic; OpenAI Responses |
| 自定义的 key | Key（不显示，可以空）： | Key (hidden, may be empty): |
| 取不到模型列表 | 未获取到模型列表 | Could not get the model list |
| 问模型名 | 模型名： | Model name: |
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

「没选」「没收到 key」「没存，取消了」「· {名字} 的 key 存好了」「· 换掉了 {名字} 的 key」「粘贴 {名字} 的 key（不显示）：」照 `miyu login` 的（`config.md`「给人看的字」；「没存，取消了」施工 8-5 补）。灰字的几行开头是「· 」，和 `miyu ask`、`miyu login` 的灰字一样。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/src/setup/pick/tests.rs` | 编号表照样子（对齐、用不了的不编号且是灰的、最后一行 `0` 只和编号对齐）；敲的编号、`0`、别的字、超出的、直接回车、读到头；目录里的编号在配置里写成什么（`p-302ai`、`wafer-ai`） |
| `crates/miyu-cli/tests/setup.rs` | 空的配置里写出三个预设的池（空成员、开关开着），已经有池的不写（施工 8-8 补）；在进程里起核心（真目录裁出来的一份，本机的服务和供应商是假服务器）、假终端照剧本回，走一遍（Unix 和 Windows 一样跑）：选一家那张表（常用的几家照先后、已找到 key、已配好、用不了的不编号、本机、最后一行自定义，施工 8-11 再补）；环境变量里的 key 只引用不复制（配置里是 `{ env }`，密钥文件里没有）；贴的 key 先试、通了存成密钥；试不通回到上一步（贴错一次再贴对）；本机的服务写 `local = true`（施工 8-25）；已经配好的只写 `models.chat`；核心看不到的变量说清是哪个、不复制；写出的配置对；推荐的排第一、敲编号换一个；没选、没收到 key；贴 key 那一步取消了（照剧本回的假终端报 `Ctrl+C`），退出码 130，配置文件、密钥文件都没动（施工 8-5 补）；屏幕上、日志里从头到尾没有 key；`miyu ask` 没模型：终端里先走 setup 再说，不是终端的退出码 5、不造会话 |
| `crates/miyu-cli/tests/setup_custom.rs`、`crates/miyu-cli/src/setup/custom/tests.rs` | 自定义（施工 8-11 再补）：问地址（不是 http、https 开头的再问）、接口协议、key，写 `driver`、`base_url`、`key`（施工 8-25），贴的 key 存成密钥；编号照主机名起（本机、IP 是 `local`，通用的二级域名往前一段，不合写法的加 `p-`），配置里有了的往后加 `-2`；空 key 不存、不写 `key`、不带认证头；列不出模型的印「未获取到模型列表」、问模型名、拿它再试、通了不再列不再问；模型名、地址直接回车是没选、什么都不写 |
| `crates/miyu-cli/tests/setup_skip.rs` | 每一步都能用参数跳过：`--provider`、`--env`、`--model` 全写的不问一句；不在终端里的从管道读 key、用推荐的模型；不在终端里又没写 `--provider` 的退出码 2、不连核心；`--provider` 不是目录里能用的退出码 2；不在终端里试不通退出码 1 |
| `crates/miyu-cli/src/help/tests.rs` | 两页列的选项和程序真有的对得上，最宽 80 列 |
| `crates/miyu/tests/setup.rs` | 真跑主程序：帮助页跟着界面语言；不在终端里又没写 `--provider` 退出码 2、不拉起核心 |
| `crates/miyu/tests/ask.rs` | 真跑 `miyu ask`：没配模型、不在终端里的退出码 5、不造会话 |

### 出处

- `models.md` 第七条（第一次接入）、「协议」的 `provider.*`、「定的」第 7 条。
- `15-模型与供应商.md` 第七节。`22-命令行.md` 第三节「还没配模型时」、第五节。
- `cli/login.md`（贴 key、`Console`）、`cli/config.md`（连核心、握手）。

### 还没有的

- 让核心空闲时重启：要一个新的协议方法。
- 借已经登录的 agent CLI 的订阅：以后再说。
- 选看图的模型、给池填成员：现在照旧写配置（`miyu config edit`）。

### 施工时定的

图纸没画到的几处（2026-10-01 施工时定，照推荐）：

| 定了什么 | 为什么 | 别的选法 |
|---|---|---|
| 参数三个：`--provider`、`--env`、`--model`；key 不上命令行（会进 shell 的历史），不在终端里的照 `miyu login` 从管道读 | 图纸「参数能跳过对应的一步」；`miyu login` 的规矩 | `--key <值>`：进 shell 的历史 |
| 不在终端里不问：没写 `--provider` 的退出码 2；模型用推荐的（等于直接回车）；试不通退出码 1 | 脚本能一行写完；能默认的不报错 | 不在终端里一律退出码 2：`--model` 不写也得写 |
| 试不通回到上一步：贴的 key 回到贴 key，别的回到选一家；写了 `--provider` 的、不在终端里的没有上一步，退出码 1。直接回车、读到头是出口（「没选」「没收到 key」） | 图纸「回到上一步」；不能卡在一个问题上出不来 | 一律退出：贴错一次要从头来 |
| 编号表最多列 20 个模型，多的说一句；模型也能敲名字 | 一家几百个模型的（中转站）一屏放不下 | 全列：几百行 |
| 用不了的照样列，不编号、灰字、接原因 | 图纸「别的也列、标出来，人知道为什么选不了」 | 不列：找到了 key 却不知道为什么不见了 |
| 「核心看不到」只说等核心空闲退出、或者选这一家贴 key | 让核心空闲时重启要一个新的协议方法，不在这一步 | 加一个 `core.restart`：多一个方法，不在图纸里 |
| 写系统配置 | 第一次接入的是管理员；模型这一块只能写系统、个人两层，`xtask dev-home` 也写系统配置 | 写个人设置 |
| 目录里的编号不合「路径里的名字」写法的（`302ai`、`wafer.ai`）换一个配置里的编号（`p-302ai`、`wafer-ai`），另写 `catalog`；密钥也用这个名字 | 配置的编号、密钥的名字都只收小写字母开头的那种，照原样写进去 `config.set` 不收 | 这两家不让选：目录里有、选不了，说不清为什么 |
| 不通的那一行不另写 HTTP 状态：驱动的原话里有（`HTTP 401: …`） | 写两遍；和 `miyu ask` 出错那一行一样 | 照 `status` 另写一段：`HTTP 401：HTTP 401: …` |
| 用不了的原因照 `driver`、`base_url` 认：`driver` 是 `null` 的认不出接口，`base_url` 是 `null` 的没有地址，都有的是驱动还没有；`provider.detect` 的 `keys` 没有地址那一格，当有地址 | 核心不另交原因；现在用不了的几家（`anthropic`、`openai`）`driver` 都有 | `provider.catalog` 另交一格原因：这一步只有这一处要它 |
| 自定义的编号由 `miyu setup` 照地址的主机名起，不让人填；撞了的往后加 `-2`、`-3`（施工 8-11 再补，2026-10-08） | 能定的不让人填（`design-for-user-needs`） | 问「起个名字：」：多一问 |
| 搜整份目录那一步去掉（施工 8-11 再补，2026-10-08） | 项目主人要的是几家加自定义；目录里别的几家照旧能经 `--provider`、配置文件用 | 留一行「从目录里找」：项目主人没要 |
| 填了模型名试通的，不再列模型、不再问（施工 8-11 再补，2026-10-08） | 人已经说了用哪个；列不出的再列一遍是空的，「照 models.dev 的目录列」那一句也没东西跟着 | 照常问：问一个刚答过的问题 |
