## 沙盒：Windows 装

### 是什么

Windows 上沙盒要的、要一次管理员权限的那件事（`sandbox.md` 的 Windows 一页，施工 5-8）：

- 建一个专用的低权限沙盒用户，藏在登录界面之外；沙盒里的命令以它的身份跑（怎么跑是 5-9）。
- 记一份「装到哪了」，好让卸载和 `miyu doctor` 找得到，也好让 5-9 拿到沙盒用户的身份。

这件事做成一个能单独调起的动作 `miyu sandbox setup`，卸载是 `miyu sandbox remove`；两条都要管理员权限，装 Miyu 时弹一次（`11-权限与沙盒.md` A3、第六节，2026-09-28 项目主人定）。安装脚本（R12）做出来以后调 `setup`。

这一页只管「装」。每条命令怎么关进沙盒（受限令牌、访问控制、单独桌面、git 的 `safe.directory`）是 5-9，见「还没有的」。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | `sandbox` 的分发臂；给 `sandbox`、`sandbox setup`、`sandbox remove` 换上帮助页 |
| `crates/miyu-cli/src/sandbox.rs` | 子命令的参数（连同只给提升过的自己用的两个）；Windows 上真的那一套「机器」 |
| `crates/miyu-cli/src/sandbox/flow.rs` | 怎么走：要不要装、替谁装、是不是管理员、自提升、提升过的自己没成时写下原因；只经「机器」这个接口干活，测试换替身 |
| `crates/miyu-cli/src/language/sandbox.rs` | 给人看的字：写在代码里，和 `undo` 一样（`cli/main.md`「还没有的」：界面的字以后挪出代码） |
| `crates/miyu-cli/src/help/{zh,en}/sandbox.txt` | `miyu sandbox -h` 的帮助页 |
| `crates/miyu-cli/src/misuse.rs` | 参数写错时：少了子命令、嵌着的子命令写错、成对的选项少了一个，各说一句（`cli/main.md`） |
| `crates/miyu-sandbox/src/install.rs` | 对外的几样：沙盒用户的名字 `USER`、这个平台要不要装、替谁装（`Owner`）、没成的原因（`InstallError`）、起提升过的自己没成（`Elevation`）；Windows 上 `setup()`、`remove()` 照次序干活 |
| `crates/miyu-sandbox/src/install/elevate.rs` | 查是不是管理员；拿 `runas` 起提升过的自己，等它，交回退出码（Windows） |
| `crates/miyu-sandbox/src/install/user.rs` | 建、重置、藏、删沙盒用户；查它是不是管理员（Windows） |
| `crates/miyu-sandbox/src/install/password.rs` | 随机密码怎么拼 |
| `crates/miyu-sandbox/src/install/dpapi.rs` | 密码用机器范围的 DPAPI 加密（Windows） |
| `crates/miyu-sandbox/src/install/record.rs` | 装到哪了那份记录：写法、读回来、写到哪 |
| `crates/miyu-sandbox/src/install/report.rs` | 提升过的自己没成时写下的原因：写、读、清 |
| `crates/miyu-sandbox/src/install/files.rs` | 往本人的数据根里写只有本人和 SYSTEM 读得到的文件：认标记、不经链接、先写临时的再改名 |
| `crates/miyu-sandbox/src/install/quote.rs` | 起提升过的自己时，参数照 Windows 命令行的规矩加引号 |
| `crates/miyu-sandbox/src/install/winsys.rs` | 调 Windows 系统接口共用的：宽字符串、SID、丢掉时释放的句柄、建好就只给本人和 SYSTEM 的文件（和 5-9 共用） |
| `crates/miyu-sandbox/Cargo.toml` | 放开 `unsafe`：整个 crate 照工作区的写法，只把 `unsafe_code` 从 forbid 改成 deny，在调系统接口的几个 Windows 模块里 allow，每处写 `// SAFETY:`；依赖加 `miyu-kernel`（装好的时刻）、`miyu-store`（数据根的标记）；`cfg(windows)` 下加 `windows-sys`、`getrandom`、`base64`，测试里加 `miyu-pipe`（测试里的本人是跑测试的这个用户） |

- `miyu-sandbox` 是执行器层（`01-架构.md` 第九节第 3 层），已经登记；不新开 crate。调 Windows 系统接口要 `unsafe`，只在这个 crate 里放开（`11-权限与沙盒.md` 第六节）。
- 装的代码在 lib 里，`miyu sandbox setup` 这个头（在 `miyu`/`miyu-cli`）调它；助手那个二进制（`src/bin/miyu-sandbox/`）不掺和，它的 `run`/`probe` 命令行一个字不动。
- 只在 Windows 上用、却不调系统接口的几个模块（`password`、`record`、`report`、`files`、`quote`），编 Windows 和编测试时都编进去：三个平台的测试都跑得到它们。

### 对外的样子

**子命令**（`22-命令行.md` 第五节、`12-进程形态与分发.md` 第三节）：

- `miyu sandbox setup`：把沙盒用户装好，装过了再装也不出错（幂等）。
- `miyu sandbox remove`：把它撤干净，没装过也不出错。

两条都没有给人用的参数。它们是给人敲的管理命令，输出跟界面语言，一个字都不进模型面。成了印在标准输出上，没成印在标准错误上。

只给提升过的自己用的两个参数，帮助里不列：`--owner-home <数据根>`、`--owner-sid <SID>`，两个一起写，只写一个的照「参数写错时」说。数据根要是绝对路径；SID 要是 `S-1-` 开头、后面二到十六段十进制数字（一段标识机构，一到十五段子机构）、用 `-` 隔开（它要拼进访问控制的写法里，别的字一个都不收）。不合的照「装沙盒失败」说，什么都不改。

**沙盒用户**：

| 格 | 值 |
|---|---|
| 登录名 | 固定一个，`miyu-sandbox`（本机用户，不含空格，不超过 20 字符） |
| 说明 | `Miyu sandbox`，在「本地用户和组」里看得到 |
| 属于 | `Users` 组（组名照 SID `S-1-5-32-545` 查，中文系统上也对），不是管理员 |
| 密码 | 每次装现生成：32 个字符，大写、小写、数字、符号（`!#%+-.:=?@^_~`）各至少一个，其余从这四类里随机取；不给人看、不打印 |
| 藏 | 写进 `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\SpecialAccounts\UserList` 的一个 `DWORD=0`：不在登录界面上显示 |
| 标志 | 普通用户、启用、密码不过期、自己改不了密码（5-9 照存下的密码登录，它自己改了就登不上） |

- 已经有一个叫 `miyu-sandbox` 的账号、它在 `Administrators` 组里（连经别的组间接在的）：`setup`、`remove` 都不动它，报「已经有一个叫 miyu-sandbox 的管理员账号」。沙盒用户是管理员，沙盒就形同虚设。

**装到哪了那份记录** `state/sandbox/windows.json`（数据根下；沙盒读不到数据根）：

| 字段 | 是什么 |
|---|---|
| `version` | 这份记录的版本，改写法就加一。现在是 1；读的时候别的版本、认不得的格、少了格，都读不了 |
| `username` | 沙盒用户的登录名 |
| `user_sid` | 它的 SID，写成 `S-1-5-21-…` |
| `password` | DPAPI 加密过的密码，base64；机器范围：提升可能换了一个管理员账号（标准用户在 UAC 里输别人的密码），当前用户范围那时核心解不开。能读到它的只有本人：文件的访问控制只给本人和 SYSTEM |
| `created_at` | 装好的时刻，UTC，精确到毫秒（`kernel/ids.md` 的写法） |

- 一行 JSON，后面一个换行。
- 访问控制是 `D:P(A;;FA;;;<本人的 SID>)(A;;FA;;;SY)`：受保护、不继承上一层，只有本人和 SYSTEM，建的时候就是这样，不是建好了再改。

记录（例子）：

```json
{"version":1,"username":"miyu-sandbox","user_sid":"S-1-5-21-2606943370-4158592556-3158051839-1002","password":"AQAAANC…","created_at":"2026-09-28T12:00:00.000Z"}
```

**提升过的自己没成时写下的原因** `state/sandbox/windows-error.json`：提升过的自己的窗口是藏起来的，它说的话人看不到，所以没成时把原因写在这里，等它的那一个读了、照界面语言说给人听、删掉。写法和记录一样（只给本人和 SYSTEM）。一行 JSON，三种：

```json
{"error":"administrator"}
{"error":"not-data-root","path":"C:\\Users\\me\\somewhere"}
{"error":"failed","step":"create user","detail":"<系统的原话>"}
```

### 怎么走

1. **非 Windows 上**：`setup`、`remove` 什么都不干，说一句「这个平台不用装」，退出 0。数据根也不找、不建。（Linux 上要不要经它装 AppArmor 配置，5-3 定。）
2. **替谁装**：带了那两个参数的，照参数（先核对写法）；没带的，是自己：数据根照 `miyu ask` 的找法（`store.md`），找到了先建骨架，SID 是当前用户的。
3. **Windows，不是管理员**（查的是当前的令牌里 `Administrators` 组启用了没有：没提升的管理员也算不是）：
   1. 删掉数据根里上次留下的 `windows-error.json`。
   2. 拿 `runas` 起一个提升过的自己（`ShellExecuteExW`，弹一次 UAC）：程序是自己的真实位置，参数是 `sandbox <setup 或 remove> --owner-home <数据根> --owner-sid <SID>`，照 Windows 命令行的规矩加引号（有空格、引号的包起来，引号前的反斜杠加倍）；窗口藏起来，出错不弹框；等它退出。提升后的进程可能是另一个管理员账号（标准用户在 UAC 里输别人的密码）：它自己的家目录、数据根、环境变量都不是本人的，所以记录写到参数给的数据根里，访问控制给参数给的 SID。
   3. 它退出 0：印「装好了」（「撤干净了」），退出 0。
   4. 人取消了提升：说一句「要管理员权限」，退出 1，什么都没改。
   5. 它退出别的：读 `windows-error.json`，照它说，删掉它；读不到的，说「装沙盒失败：elevate：exit code <几>」。退出 1。
4. **Windows，是管理员——`setup`**，照下面的次序，每一步幂等：
   1. 认数据根：里面要有 `.miyu-root` 标记（`store.md`），没有的报「不是 Miyu 的数据根」，用户、密码一样都不碰：免得密码换了却没处记。
   2. 查已有的 `miyu-sandbox`：是管理员的，不动，报错。
   3. 建沙盒用户：没有就建；有就把密码重置成新的，再把标志设回上表的样子（被停用了的也启用）。
   4. 加进 `Users` 组；已经在里面不算错。
   5. 藏起来：写 `UserList`。
   6. 查出它的 SID；这个名字是个组、不是用户的，算错。
   7. 密码 DPAPI 加密（机器范围），base64。
   8. 写记录：
      1. `state`、`state/sandbox` 一层一层来：没有就只建这一层，再查它不是链接（符号链接、目录联接都算；OneDrive 按需下载这类别的重解析点不算，数据根放在同步的目录里也装得上），查过了才往下一层走；是链接的不经它写，报错。不能先一口气建好再查：`state` 是链接的话，一口气建就顺着它建到别处去了。数据根本身是链接不要紧。
      2. 先删掉上次留下的 `windows.json.tmp`，再只许新建地建它，不跟链接，建的时候访问控制就是上面那样；写进去、同步，再改名成 `windows.json`，盖掉旧的。
      3. 挡得住提前放好的链接，挡不住检查完、写之前被换掉：和 `fs.md` 那一条一样，能换的人本来就能改本人的数据根。
   9. 印「装好了」。
5. **Windows，是管理员——`remove`**：认数据根（同上）；查已有的 `miyu-sandbox`，是管理员的不动、报错；删它的 profile；删用户；从 `UserList` 里取消隐藏；删记录文件。先删用户、后取消隐藏：删到一半没成的，账号照样藏着，不会出现在登录界面上。哪一样本来就没有，跳过不算错。印「撤干净了」。
6. **提升过的自己**（带了那两个参数、又是管理员）没成：先把原因写进 `windows-error.json`（写不成就算了，照样说），再照常说、退出 1。
7. **出错**：哪一步失败就说清是哪一步、系统的原话；装了一半的，再跑一次 `setup` 接着装。步骤的名字是英文：`check owner`（参数的写法、数据根的标记看不了）、`find owner`（找自己的数据根、SID）、`check account`、`create user`、`reset password`、`set flags`、`join Users`、`hide user`、`look up user`、`protect password`、`write record`、`delete profile`、`delete user`、`unhide user`、`delete record`、`elevate`、`write report`、`read report`、`clear report`。

### 样子

给人看的字写在代码里（`crates/miyu-cli/src/language/sandbox.rs`），和 `undo` 的一样；不进登记簿，不给模型看。

终端上（例子，中文）：

```text
$ miyu sandbox setup
沙盒用户建好了。
```

`miyu sandbox -h`、`miyu sandbox setup -h`、`miyu sandbox remove -h`、`miyu help sandbox` 印的是同一页。

样本 `crates/miyu-cli/src/help/zh/sandbox.txt`（帮助页，中文）：

```text
用法：miyu sandbox <命令>

装好、撤掉沙盒要管理员权限的那部分：一个专用的沙盒用户（Windows）。

命令：
  setup   建好沙盒用户
  remove  撤掉沙盒用户

不是管理员时，弹一次 Windows 的权限确认。别的平台上不用装。

  -h, --help  印帮助
```

样本 `crates/miyu-cli/src/help/en/sandbox.txt`（帮助页，英文）：

```text
Usage: miyu sandbox <command>

Set up or remove what the sandbox needs an administrator for: a dedicated
sandbox user (Windows).

Commands:
  setup   Create the sandbox user
  remove  Remove the sandbox user

Without administrator rights, Windows asks for permission once. Other platforms
need nothing.

  -h, --help  Print help
```

### 出错

| 什么时候 | 说的话 | 退出码 |
|---|---|---|
| 成功 | 装好了 / 撤干净了 / 这个平台不用装 | 0 |
| 用法不对（多了参数、少了子命令等） | 照 `cli/main.md` 的「参数写错时」 | 2 |
| 要管理员、人取消了提升 | 要管理员权限 | 1 |
| 已经有这个名字的管理员账号 | 已经有一个叫 miyu-sandbox 的管理员账号 | 1 |
| 给的数据根没有标记 | 不是 Miyu 的数据根 | 1 |
| 哪一步失败 | 装沙盒失败（卸沙盒失败）：<哪一步>：<系统的原话> | 1 |

退出码照命令行共用的那张表（`cli/main.md`、`22-命令行.md` 第二节）。

### 给人看的字

| 什么时候 | 中文 | 英文 |
|---|---|---|
| 装好了 | 沙盒用户建好了。 | The sandbox user is set up. |
| 撤干净了 | 沙盒用户撤掉了。 | The sandbox user is removed. |
| 这个平台不用装 | 这个平台不用装沙盒。 | Nothing to set up on this platform. |
| 要管理员 | 要管理员权限：用管理员身份跑 miyu sandbox <setup 或 remove>。 | Administrator rights are needed: run miyu sandbox <setup or remove> as administrator. |
| 管理员账号 | 已经有一个叫 miyu-sandbox 的管理员账号，不动它。 | An administrator account named miyu-sandbox already exists; leaving it alone. |
| 不是数据根 | <路径> 不是 Miyu 的数据根。 | <path> is not a Miyu data root. |
| 装的哪一步失败 | 装沙盒失败：<哪一步>：<原话> | Sandbox setup failed: <step>: <原话> |
| 卸的哪一步失败 | 卸沙盒失败：<哪一步>：<原话> | Sandbox removal failed: <step>: <原话> |

- `<setup 或 remove>` 照这一次敲的是哪一个；`<原话>` 里的控制字符换成 `�`。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-sandbox/src/install/tests.rs` | 名字不超过 20 字符、没有空格；只有 Windows 要装；SID 的写法：真的 SID 收，拼得进访问控制的怪字、空的、段太多的、全角数字不收；`Owner` 只收绝对路径和合写法的 SID；三种原因各一句英文 |
| `crates/miyu-sandbox/src/install/password/tests.rs` | 32 个字符，四类都有，只用表里的字，符号就是上表那一串；随机字节不同，密码不同，最后一个字节也用上 |
| `crates/miyu-sandbox/src/install/quote/tests.rs` | 没空格的照原样；空格、制表符、空的包起来；引号和它前面的反斜杠；包起来的结尾反斜杠加倍；非 ASCII、落单的代理项原样；带 NUL 的拼不成 |
| `crates/miyu-sandbox/src/install/record/tests.rs` | 记录是一行、读回来一样；版本不认得、多了格、少了格、类型不对的读不了；写进参数给的数据根、没留临时文件；没有标记的一个字节都不写；上次留下的临时文件清掉；盖掉旧的；删、再删，删也要认标记 |
| `crates/miyu-sandbox/src/install/report/tests.rs` | 三种原因写成上面那三行、读回来一样；没有的是空的；清掉、再清；后写的盖掉先写的；读不懂的说 `read report`；没有标记的不写 |
| `crates/miyu-sandbox/src/install/files/tests.rs` | 标记只看在不在；`state/sandbox` 建在 `state` 下，再建也成；只许新建、不覆盖；Unix 上 `state`、`state/sandbox` 是链接的不经它写，放文件的地方是链接的不跟，文件是 0600；Windows 上建出来的访问控制就是本人和 SYSTEM、受保护，目录联接、目录链接都不经它写（建不了目录链接的机器上那一条跳过） |
| `crates/miyu-sandbox/src/install/dpapi/tests.rs` | Windows：加密了解得回来；密文里没有明文（UTF-16 的也没有）；同一个密码两份密文不一样 |
| `crates/miyu-sandbox/src/install/user/tests.rs` | Windows，`#[ignore]`，只在虚拟机上以管理员身份跑（会真建用户，用的是 `miyu-sbx-test`）：建出来是普通用户、启用、密码不过期、改不了密码、说明对、在 `Users` 不在 `Administrators`、藏着、照密码登得上；再建是重置密码、又启用，旧密码不能用了；是管理员的不动；没有的删了不算错，注册表没留下；同名的组不当成用户 |
| `crates/miyu-cli/src/sandbox/tests.rs` | 怎么走，用替身：是管理员的直接干；替谁装照参数给的数据根和 SID，不问自己是谁；提升过的自己没成才写原因；不是管理员的先清旧原因、再起自己，参数带着数据根和 SID，自己不干；取消、退出 0、带原因退出（读了就清）、不带原因退出、起不来、查不了是不是管理员、找不到自己、参数写法不对，各怎么说；参数照主程序读回来是同一个人；每一句两种语言；退出码 |
| `crates/miyu-cli/src/help/tests.rs` | `sandbox` 那一页列的选项和 `sandbox`、`setup`、`remove` 真有的对得上（藏起来的两个不算）；是它自己那份文件；最宽 80 列 |
| `crates/miyu-cli/src/misuse/tests.rs` | 少了子命令、嵌着的子命令写错、成对的少了一个、多了参数，各说一句；最外面那一层写错的照旧 |
| `crates/miyu/tests/commands.rs` | 非 Windows：`miyu sandbox setup`、`remove` 说那一句、退出 0，数据根一样东西都没多；三种写错退出 2、说那一句；`-h`、`--help`、`help sandbox` 印那一页；主程序换了文件名，说的还是 `miyu sandbox`（Windows 上文件叫 `miyu.exe`） |
| 虚拟机实测（CI 测不了） | 真装真卸：用户建出来/藏着/是普通用户，记录在本人的数据根里、只有本人和 SYSTEM 读得到，卸了不留东西；管理员弹窗只在默认 UAC 的桌面上验（施工单「验收」） |

### 出处

- `11-权限与沙盒.md` 第六节 Windows（A3）：专用沙盒用户、安装要一次管理员权限；第五节 A6：沙盒用户不是本人，连不上只对本人开放的管道。
- `12-进程形态与分发.md` 第三节、R12：`miyu sandbox setup`/`remove` 是主程序的子命令，安装脚本装完调它；第七节：`miyu doctor` 查「Windows 的沙盒安装有没有完成」。
- `22-命令行.md` 第五节：命令全表；第二节：退出码。
- `01-架构.md` 第九节：miyu-sandbox 在第 3 层。

### 还没有的

- **每条命令怎么关进沙盒（5-9）**：给沙盒用户现生成受限令牌（带只属于这个成员的限制 SID）、以它的身份起命令、工作区和工具链目录的访问控制、单独的桌面、给沙盒用户登记 git 的 `safe.directory`。助手的 `windows.rs` 那时才真收紧。`remove` 那时还要先停掉以沙盒用户身份在跑的进程，不然 profile 删不掉。
- **`miyu doctor` 读这份记录**报「Windows 的沙盒安装有没有完成」（`12-进程形态与分发.md` 第七节）。
- **安装脚本（R12）调 `setup`**：脚本本身还没有，先做成能单独调起的动作。
- **权限策略接上（5-4）**：沙盒用不了（没装）时改成问人、`miyu ask` 开头说一句。
