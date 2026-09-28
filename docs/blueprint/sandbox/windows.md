## 沙盒：Windows 装

### 是什么

Windows 上沙盒要的、要一次管理员权限的那件事（`sandbox.md` 的 Windows 一页，施工 5-8）：

- 建一个专用的低权限沙盒用户，藏在登录界面之外；沙盒里的命令以它的身份跑（怎么跑是 5-9）。
- 照这个用户装好网络封锁：只放行连本机的 Miyu 代理，别的本机地址、所有非本机地址一律拦。
- 记一份「装到哪了」，好让卸载和 `miyu doctor` 找得到，也好让 5-9 拿到沙盒用户的身份。

这件事做成一个能单独调起的动作 `miyu sandbox setup`，卸载是 `miyu sandbox remove`；两条都要管理员权限，装 Miyu 时弹一次（`11-权限与沙盒.md` A3、第六节，2026-09-28 项目主人定）。安装脚本（R12）做出来以后调 `setup`。

这一页只管「装」。每条命令怎么关进沙盒（受限令牌、访问控制、单独桌面、git 的 `safe.directory`）是 5-9，见「还没有的」。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | `sandbox setup`、`sandbox remove` 两个子命令的分发臂 |
| `crates/miyu-cli/src/sandbox.rs` | 子命令的头：调 miyu-sandbox 装/卸，把结果照界面语言印给人看 |
| `crates/miyu-cli/src/language/sandbox.rs` | `setup`/`remove` 给人看的字（跟界面语言，`human/*.json`） |
| `crates/miyu-sandbox/src/install.rs` | `setup()`、`remove()`：查是不是管理员、要就自提升、按平台干活、记录；非 Windows 是空动作 |
| `crates/miyu-sandbox/src/install/elevate.rs` | 查当前是不是管理员；自提升：拿 `runas` 重新起自己、等它、照它的退出码退出（Windows） |
| `crates/miyu-sandbox/src/install/user.rs` | 建/藏/删沙盒用户，随机密码，DPAPI 加密（Windows） |
| `crates/miyu-sandbox/src/install/network.rs` | WFP 过滤器：照沙盒用户装网络封锁、按稳定 GUID 撤（Windows） |
| `crates/miyu-sandbox/src/install/record.rs` | 装到哪了那份记录的读写 |
| `crates/miyu-sandbox/src/install/winsys.rs` | 调 Windows 系统接口共用的：SID、宽字符串、丢掉时释放的句柄（和 5-9 共用） |
| `crates/miyu-sandbox/Cargo.toml` | 放开 `unsafe`（和 5-2/5-6/5-7 同一处，照 `miyu-pipe` 的写法）；`cfg(windows)` 下加 `windows-sys` 的几个 feature |

- `miyu-sandbox` 是执行器层（`01-架构.md` 第九节第 3 层），已经登记；不新开 crate。调 Windows 系统接口要 `unsafe`，只在这个 crate 里放开，每处写 `// SAFETY:`（`11-权限与沙盒.md` 第六节）。
- 装的代码在 lib 里，`miyu sandbox setup` 这个头（在 `miyu`/`miyu-cli`）调它；助手那个二进制（`src/bin/miyu-sandbox/`）不掺和，它的 `run`/`probe` 命令行一个字不动。

### 对外的样子

**子命令**（`22-命令行.md` 第五节、`12-进程形态与分发.md` 第三节）：

- `miyu sandbox setup`：把沙盒用户和网络封锁装好，装过了再装也不出错（幂等）。
- `miyu sandbox remove`：把它们撤干净，没装过也不出错。

两条都没有别的参数。它们是给人敲的管理命令，输出跟界面语言，一个字都不进模型面。

**沙盒用户**：

| 格 | 值 |
|---|---|
| 登录名 | 固定一个，`miyu-sandbox`（本机用户，不含空格，不超过 20 字符） |
| 属于 | 只在 `Users` 组里，不是管理员 |
| 密码 | 每次装现生成的随机串，不给人看、不打印 |
| 藏 | 写进 `HKLM\...\Winlogon\SpecialAccounts\UserList` 的一个 `DWORD=0`：不在登录界面上显示 |
| 标志 | 普通用户、密码不过期 |

**网络封锁**：WFP（Windows 过滤平台）的持久过滤器，条件是「用户是沙盒用户的 SID」，动作是拦：

| 拦什么 | 为什么 |
|---|---|
| 连所有非本机地址 | 沙盒里的命令不能直接上外网，只能经 Miyu 的代理（`11-权限与沙盒.md` 第四节 A5） |
| 连本机、但不在放行的代理端口段里的 | 拦住本机开着的别的服务（数据库、Docker、旧版 8300），它们能在沙盒外替命令做事（第四节、第五节） |

- 过滤器、子层、提供者都用 Miyu 自己的稳定 GUID，装了又装是「按 GUID 删掉再加」（幂等）；显示名是英文短句，管理工具里看得到。
- **放行的代理端口段**：一段固定的本机 TCP 端口范围（要拍板，见施工单）；代理（5-5）在这段里绑。绑不进的，沙盒网络全关（fail-closed），核心说一句。
- 走 WFP 直接下过滤器、不走高层防火墙接口（COM）：防火墙在设置里被关掉时，COM 下的规则不生效，WFP 过滤器照拦（BFE 服务在跑就行）。

**装到哪了那份记录** `state/sandbox/windows.json`（数据根下，属主是本人；沙盒读不到数据根）：

| 字段 | 是什么 |
|---|---|
| `version` | 这份记录的版本，改写法就加一 |
| `username` | 沙盒用户的登录名 |
| `user_sid` | 它的 SID，写成 `S-1-5-21-…` |
| `password` | DPAPI 加密过的密码，base64；当前用户范围（`setup` 和核心都是本人这个账号，解得开），比 codex 的机器范围窄 |
| `proxy_ports` | 放行的代理端口段 |
| `created_at` | 装好的时刻，RFC 3339 |

记录（例子）：

```json
{"version":1,"username":"miyu-sandbox","user_sid":"S-1-5-21-2606943370-4158592556-3158051839-1002","password":"AQAAANC…","proxy_ports":[3128,3129],"created_at":"2026-09-28T12:00:00Z"}
```

### 怎么走

1. **非 Windows 上**：`setup`、`remove` 什么都不干，说一句「这个平台不用装」，退出 0。（Linux Ubuntu 的 AppArmor 配置是 `miyu sandbox setup` 的另半，5-3 补。）
2. **Windows，不是管理员**：拿 `runas` 重新起一个提升过的自己（`ShellExecuteExW`，弹一次 UAC），等它退出，照它的退出码退出。人取消了提升：说一句「要管理员权限」，退出非 0，什么都不改。
3. **Windows，是管理员——`setup`**，按下面的次序，每一步幂等：
   1. 建沙盒用户：没有就建（随机密码），有就把密码重置成新的随机串；加进 `Users` 组；设成普通用户、密码不过期。
   2. 藏起来：写 `Winlogon\SpecialAccounts\UserList`。
   3. 密码 DPAPI 加密（当前用户范围）。
   4. 装 WFP 的提供者、子层、过滤器：先按 GUID 删掉旧的，再加；条件按沙盒用户的 SID，拦上表两类。
   5. 写记录文件，文件的访问控制只留本人。
   6. 印一句「装好了」。
4. **Windows，是管理员——`remove`**：按 GUID 删掉过滤器、子层、提供者；从 `UserList` 里取消隐藏、删用户（连它的 profile）；删记录文件。哪一样本来就没有，跳过不算错。印一句「撤干净了」。
5. **出错**：哪一步失败就说清是哪一步、系统的原话；装了一半的，说明还剩什么、再跑一次 `setup` 接着装。BFE 服务没在跑：装不了 WFP，说一句怎么开。
6. 命令的输出都是给人看的（一个人在敲这条命令），跟界面语言，一个字不进模型面。

### 样子

给人看的：`setup`/`remove` 印的几句在资源目录 `human/zh.json`、`human/en.json` 里（`store/resources.md`，双槽的人槽，不进登记簿）。

终端上（例子，中文）：

```text
$ miyu sandbox setup
沙盒用户建好了，网络封锁装上了。
```

### 出错

| 什么时候 | 说的话（英文键，跟界面语言取字） | 退出码 |
|---|---|---|
| 成功 | 装好了 / 撤干净了 | 0 |
| 用法不对（多了参数等） | 照 `cli/main.md` 的「参数写错时」 | 2 |
| 要管理员、人取消了提升 | 要管理员权限：用管理员身份跑 miyu sandbox setup | 1 |
| BFE 服务没在跑 | 装不了网络封锁：基础过滤引擎（BFE）没在跑 | 1 |
| 哪一步失败 | 装沙盒失败：<系统的原话> | 1 |

退出码照命令行共用的那张表（`cli/main.md`、`22-命令行.md` 第二节）。

### 给人看的字

| 什么时候 | 中文 | 英文 |
|---|---|---|
| 装好了 | 沙盒用户建好了，网络封锁装上了。 | Sandbox user and network rules are set up. |
| 撤干净了 | 沙盒用户和网络封锁都撤掉了。 | Sandbox user and network rules are removed. |
| 这个平台不用装 | 这个平台不用装沙盒。 | Nothing to set up on this platform. |
| 要管理员 | 要管理员权限：用管理员身份跑 miyu sandbox setup。 | Administrator rights are needed: run miyu sandbox setup as administrator. |
| BFE 没在跑 | 装不了网络封锁：基础过滤引擎（BFE）没在跑。 | Cannot set up network rules: the Base Filtering Engine (BFE) is not running. |
| 哪一步失败 | 装沙盒失败：<原话> | Sandbox setup failed: <原话> |

- 措辞和键名施工时定稿，落进 `human/*.json`；这里是意思。给人看，不给模型看。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-sandbox/src/install/network/tests.rs` | WFP 过滤器拼出来的规格：GUID、层、按用户 SID 的条件、拦哪几类；端口段换成过滤条件对；不真装 |
| `crates/miyu-sandbox/src/install/record/tests.rs` | 记录写进去、读回来一样；版本不认得的读不了；密码那一格不明文落盘 |
| `crates/miyu-sandbox/src/install/tests.rs` | 非 Windows 上 `setup`/`remove` 是空动作、退出 0；不是管理员时先自提升（用替身看它起的命令）；每一步幂等 |
| `crates/miyu/tests/commands.rs` | `miyu sandbox setup`/`remove` 解析得到、分发对；子命令写错的话 |
| 虚拟机实测（CI 测不了） | 真装真卸：用户建出来/藏着/是普通用户；WFP 过滤器装上/删净；沙盒用户连公网被拦、连代理端口通；管理员弹窗只在默认 UAC 的桌面上验（施工单「验收」） |

### 出处

- `11-权限与沙盒.md` 第六节 Windows（A3）：专用沙盒用户、按用户生效的 WFP 规则、安装要一次管理员权限；第四节 A5：只经代理、拦本机内网；第五节 A6：沙盒用户不是本人，连不上只对本人开放的管道。
- `12-进程形态与分发.md` 第三节、R12：`miyu sandbox setup`/`remove` 是主程序的子命令，安装脚本装完调它；第七节：`miyu doctor` 查「Windows 的沙盒安装有没有完成」。
- `22-命令行.md` 第五节：命令全表；第二节：退出码。
- `01-架构.md` 第九节：miyu-sandbox 在第 3 层。

### 还没有的

- **每条命令怎么关进沙盒（5-9）**：给沙盒用户现生成受限令牌（带只属于这个成员的限制 SID）、以它的身份起命令、工作区和工具链目录的访问控制、单独的桌面、给沙盒用户登记 git 的 `safe.directory`。助手的 `windows.rs` 那时才真收紧。
- **放行代理端口段的收口**：本机放行段的确定值等 5-5/5-6/5-7 对齐（施工单「要拍板的」）。
- **`miyu doctor` 读这份记录**报「Windows 的沙盒安装有没有完成」（`12-进程形态与分发.md` 第七节）。
- **安装脚本（R12）调 `setup`**：脚本本身还没有，先做成能单独调起的动作。
- **权限策略接上（5-4）**：沙盒用不了（没装）时改成问人、`miyu ask` 开头说一句。
