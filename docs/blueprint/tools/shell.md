## `shell`

### 是什么

在这一轮的工作目录里用这台机器的 shell 执行一条命令，跑完交回输出和退出码。只有前台：到时、叫停时整组杀掉。每次调用起一个新的 shell，`cd`、变量都不带到下一次。命令只拿到白名单上的环境变量，不读用户的启动文件。不报效果：命令改了哪些文件内核看不懂，撤销不了。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/shell.rs` | 参数、超时、结果那一句 |
| `crates/miyu-basesystem/src/shell/program.rs` | 用哪个 shell、怎么起 |
| `crates/miyu-basesystem/src/shell/env.rs` | 命令拿得到哪些环境变量 |
| `crates/miyu-basesystem/src/shell/process.rs` | 起命令、读输出、等它结束、整组杀 |
| `crates/miyu-basesystem/src/shell/output.rs` | 输出：边读边解成字、内存里只留头尾、截成头尾两段 |
| `crates/miyu-basesystem/src/load.rs` | 说明是一段模板，核心起来时换进 shell 的名字 |
| `resources/software/basesystem/tools/shell.json` | 说明和参数格式 |
| `resources/software/basesystem/shell/*.txt`、`common/bad-args.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/human/{zh,en}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `execute`。说明和参数的原文如下。

样本 `resources/software/basesystem/tools/shell.json`：

```json
{
  "description": "Execute a command with {shell} and return its output. Use it for builds, tests, git and other programs, not to read, search or edit files. Every call starts in the working directory, so cd does not carry over to the next call.",
  "parameters": {"type":"object","properties":{"command":{"type":"string"},"timeout":{"type":"integer","description":"Milliseconds before the command is stopped, up to 600000. Default 120000."}},"required":["command"]}
}
```

- 说明是一段模板：核心起来时找一次用哪个 shell，把 `{shell}` 换成它的名字（`bash`、`zsh`、`PowerShell 7`、`Windows PowerShell 5.1`），核心跑着时不变；造会话时连同工具面存进快照。例如 Linux 上：`Execute a command with bash and return its output. Use it for builds, tests, git and other programs, not to read, search or edit files. Every call starts in the working directory, so cd does not carry over to the next call.`
- 别的工具的说明不当模板读，里面的花括号是字面的。

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `command` | 是 | 要执行的命令 |
| `timeout` | 否 | 毫秒。没给、给了 0，是 120000；大过 600000 的照 600000。要是不小于 0 的整数，负数参数不对 |

- 参数格式里不写、写了也认的：`description` 认了不用；`run_in_background` 是真的，不跑（下面第 2 条），它写成字符串的（`"true"`）参数不对：参数格式里没声明，内核不修正。别的参数不认，也不报错。
- 不报要碰的路径。权限策略照访问类别判：完全放开、工作区这一级放行，只读时问人（M5 之前还没有沙盒，`session/guard.md`）。

### 怎么走

1. 读参数，读不懂的（没写 `command`、类型不对）：参数不对。
2. `run_in_background` 是真的：不跑，说还不能放到后台，让她在前台跑、慢的放宽 `timeout`；出错。
3. 定超时（上面的表）。
4. 起命令：
   - 程序和参数照下面「用哪个 shell」。
   - 在这一轮的工作目录里跑：`cwd` 是 `~` 开头的，照 `tilde` 接家目录（`fs.md` 第二节第 1 条，和别的工具一样），别的照原样当目录（施工 4-9 再补二）。
   - 环境变量先清空，只放白名单上的（下面「环境变量」），再设上 `GIT_TERMINAL_PROMPT=0`。
   - 标准输入接空的：要人输入的命令读到结尾就退出。标准输出、标准错误接到同一根管道上，照写出来的先后。
   - Unix 上命令自成一个进程组；Windows 上不弹控制台窗口。
   - 起不来（程序找不到、工作目录不在、管道建不起来）：执行不了，带上系统的原话；出错。
5. 读输出：另一个线程一次读 8192 字节，读到的一段照 UTF-8 解，一个字切在两段中间的留到下一段再解，解不开的字节换成 `�`，推给头（瞬时的 `tool.progress`，不落盘）；读完了剩下没配齐的也换成 `�` 推出去。内存里只留最前 64 KiB 和最后 64 KiB，另外数一共多少字节、多少个字、多少个换行，输出再多内存也不涨。读的线程带着派活时的 span（施工 4-9 再补四上）。
6. 等它结束，最多等超时那么久：
   - 自己结束了：记下退出码（Unix 上被信号杀掉的，记下信号）。
   - 到时了：整组杀掉，再等它最多 5 秒，算超时。5 秒还不结束的（Windows 上 `taskkill` 没杀掉之类）也算超时，记一行 `WARN` `command still running after kill`，不再等它（施工 4-9 再补二）。
   - 等不了（系统报错）：执行不了。
7. 结束以后：Unix 上把组里还在跑的（`&` 放到后台的）也杀掉；Windows 上不杀，退出以后按编号杀可能杀错。再等读输出的线程最多 0.5 秒：还有东西拿着管道的（Windows 上它放出去的孙进程，Unix 上 `setsid` 出了组的），不等它，读到多少算多少，记一行 `DEBUG` `command output still open after the command ended`。这几行运行日志的来源是 `shell`，带会话编号（`log.md` 第 8 条；施工 4-9 再补四上：原来来源写 `basesystem`，也没有会话编号）。
8. 截给她看的（最多 30000 个字）：
   1. 内存里留的头尾接起来照 UTF-8 解（解不开的换成 `�`）。不多过 30000 个字的，整段给。
   2. 多过的，留开头 15000 个字、结尾 15000 个字：开头那一段的最后一个换行落在它后一半里的，截在这个换行后面；结尾那一段的开头已经在行首的（前一个字是换行），照原样；不然第一个换行落在它前一半里的，从这个换行后面起（施工 4-9 再补二）。一半按字节算。
   3. `\r\n` 换成 `\n`，单独的 `\r` 不动。
   4. 每一段不是空的就以换行结尾：头、`[... {count} characters omitted ...]`、尾、`(Showed the start and the end of {total} characters. To see all of it, write the output to a file and read the file.)`。`count` 是一共的字数减去头尾给了的字数，`total` 是一共的字数，内存里丢掉的中间那一段也数在里面。
   - 「字」按 UTF-8 数：不是接续字节的每个字节算一个字。
9. 结果：

   | 怎么结束的 | 给她的字 | 出错 | 说法 |
   |---|---|---|---|
   | 退出码 0，一个字节都没输出 | `No output` | 否 | `shell/quiet` |
   | 退出码 0 | 输出 | 否 | `shell/done`，`count` 是几行 |
   | 退出码不是 0 | 输出，接 `Exit code {code}` | 是 | `shell/exited`，字段 `code` |
   | 被信号杀掉（Unix） | 输出，接 `Killed by signal {signal}` | 是 | `shell/signal`，字段 `signal`（信号的编号） |
   | 到时了 | 输出，接 `Stopped after {timeout} ms because the command took too long. If it needs more time, pass a larger timeout, up to {max}.` | 是 | `shell/timed-out`，字段 `seconds` |

   - 「几行」是换行的个数，最后一段没有换行的也算一行。
   - `{timeout}` 是这一次实际用的毫秒数，`{max}` 是 600000。`seconds` 是它换成秒：整秒的不带小数，不然带一位小数（`300` 毫秒写成 `0.3`）。
10. 不报效果。
11. 叫停：丢掉这次调用时整组杀掉（Unix 上 `kill(-pgid, SIGKILL)`，Windows 上 `taskkill /T /F /PID`）。

**用哪个 shell**（核心起来时找一次，照核心环境里的 `PATH`）：

| 平台 | 程序 | 参数 | 说明里的名字 |
|---|---|---|---|
| macOS | `/bin/zsh` | `-f +o nomatch -c <命令>` | `zsh` |
| Windows，`PATH` 里有 `pwsh.exe` | 找到的那一个 | `-NoLogo -NoProfile -NonInteractive -EncodedCommand <编好的命令>` | `PowerShell 7` |
| Windows，没有 | `%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe`，没设 `SystemRoot` 的是 `powershell.exe` | 同上 | `Windows PowerShell 5.1` |
| 别的（Linux 等） | `PATH` 里的 `bash`，找不到用 `/bin/bash` | `--noprofile --norc -c <命令>` | `bash` |

- 不读用户的启动文件：那里可能 `export` 了密钥，环境变量的白名单就白设了。
- zsh 带 `+o nomatch`：没匹配到的通配符照原样传下去，和 bash 一样（没加引号的网址里的 `?`、`pip install foo[bar]` 里的方括号，施工 4-9 再补二）。
- PowerShell 的命令前面先加一句，和她的命令写在同一行（出错时报的行号还对得上），整条编成 UTF-16LE 再 base64：

  ```text
  [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); $ProgressPreference = 'SilentlyContinue'; <命令>
  ```

  输出编码设成不带 BOM 的 UTF-8（Windows PowerShell 默认用系统的代码页）；关掉进度条（输出接到管道上时，进度条会变成一段 XML 写进标准错误）。

**环境变量**（`11-权限与沙盒.md` 第四节）：只传名单上的，其余一律不传；名字在 Windows 上不分大小写，别处分。

| 哪一类 | 名字 |
|---|---|
| 找程序、家目录、临时目录、用户 | `PATH`、`HOME`、`USER`、`LOGNAME`、`SHELL`、`TMPDIR`、`TMP`、`TEMP` |
| 语言、时区、终端 | `LANG`、`LANGUAGE`、`TZ`、`TERM`，和 `LC_` 开头的（开头照原样比） |
| 工具链 | `CARGO_HOME`、`RUSTUP_HOME`、`GOPATH`、`GOROOT`、`JAVA_HOME`、`NVM_DIR`、`PNPM_HOME`、`VIRTUAL_ENV` |
| Windows 自己要的 | `PATHEXT`、`SystemRoot`、`SystemDrive`、`windir`、`ComSpec`、`USERPROFILE`、`USERNAME`、`HOMEDRIVE`、`HOMEPATH`、`APPDATA`、`LOCALAPPDATA`、`ProgramData`、`ProgramFiles`、`ProgramFiles(x86)`、`ProgramW6432`、`CommonProgramFiles`、`CommonProgramFiles(x86)`、`CommonProgramW6432`、`PUBLIC`、`ALLUSERSPROFILE`、`NUMBER_OF_PROCESSORS`、`PROCESSOR_ARCHITECTURE`、`OS`、`PSModulePath` |
| 另外设上的 | `GIT_TERMINAL_PROMPT=0`：git 要密码时直接失败，不等人输入；核心环境里原来的那一份不传 |

- 有意不传的：`SSH_AUTH_SOCK`（本机的 ssh-agent）、`DISPLAY`、`WAYLAND_DISPLAY`（桌面）。核心环境里从终端带来的密钥（例如 `DEEPSEEK_API_KEY`）也拿不到。

### 样子

`printf 'x\n'; exit 2`：

```text
x
Exit code 2
```

输出太长时（Linux 上 `seq 1 20000`，一共 108894 个字；这一页用 `…` 省掉了中间的行）：

```text
1
2
…
3221
[... 78896 characters omitted ...]
17501
…
20000
(Showed the start and the end of 108894 characters. To see all of it, write the output to a file and read the file.)
```

- 头：前 15000 个字截在 `3222` 的中间，最后一个换行落在后一半里，截在 `3221` 那一行后面，给 14998 个字。
- 尾：后 15000 个字正好从 `17501` 那一行的开头起，已经在行首，照原样给 15000 个字。
- 省了 108894 − 14998 − 15000 = 78896 个字。说法是 `shell/done`，`count` 是 20000。

给她的字都在 `resources/software/basesystem/shell/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节：

| 什么时候 | 文件 | 原文 |
|---|---|---|
| 退出码 0、没有输出 | `empty.txt` | `No output` |
| 退出码不是 0 | `exit.txt` | `Exit code {code}` |
| 被信号杀掉 | `signal.txt` | `Killed by signal {signal}` |
| 到时了 | `timed-out.txt` | `Stopped after {timeout} ms because the command took too long. If it needs more time, pass a larger timeout, up to {max}.` |
| 截在中间 | `omitted.txt` | `[... {count} characters omitted ...]` |
| 截了，末尾 | `truncated.txt` | `(Showed the start and the end of {total} characters. To see all of it, write the output to a file and read the file.)` |
| 执行不了 | `failed.txt` | `Could not run {shell}: {error}.` |
| 要放到后台 | `no-background.txt` | `Running in the background is not available yet. Run the command in the foreground, with a larger timeout if it is slow.` |

输出本身不转义；换进这几句的字段照模板的规矩转义（`tools/read.md` 第 8 条）。

### 出错

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 参数不对 | `The arguments are not right: {error}.` | `common/bad-args`，字段 `error` |
| 要放到后台 | `no-background.txt` | `shell/no-background` |
| 起不来、等不了 | `Could not run {shell}: {error}.` | `shell/failed`，字段 `error` |
| 退出码不是 0、被信号杀掉、到时了 | 见「怎么走」第 9 条 | 同上 |

- `{shell}` 是说明里的那个名字；`{error}` 是系统的原话，例如工作目录不在时的 `No such file or directory (os error 2)`。

### 给人看的字

显示名：执行命令（Run），后面跟 `command` 的值；符号 `$`；`block` 是 `command`：`miyu ask` 里写成 `$ 命令`，不写显示名，下面印她看到的输出（`cli/ask.md`「执行命令那一块」）。

| 说法 | 中文 | 英文 |
|---|---|---|
| `shell/done`（`count`） | `输出 {count} 行` | `{count} lines of output` |
| `shell/quiet` | 没有输出 | no output |
| `shell/exited`（`code`） | `退出码 {code}` | `exit code {code}` |
| `shell/signal`（`signal`） | `被信号 {signal} 停掉了` | `killed by signal {signal}` |
| `shell/timed-out`（`seconds`） | `超过 {seconds} 秒，停掉了` | `stopped after {seconds} s` |
| `shell/failed`（`error`） | `执行不了：{error}` | `can't run: {error}` |
| `shell/no-background` | 还不能放到后台跑 | can't run in the background yet |
| `common/bad-args`（`error`） | `参数不对：{error}` | `bad arguments: {error}` |

### 三个平台

| | Linux | macOS | Windows |
|---|---|---|---|
| 用哪个 | bash | zsh | PowerShell 7，没有就 Windows PowerShell 5.1 |
| 整组杀 | `kill(-pgid, SIGKILL)` | 同 Linux | `taskkill /T /F /PID`，杀整棵进程树 |
| 命令退出以后剩下的 | 杀掉 | 杀掉 | 不杀 |
| 被信号杀掉 | 说是哪个信号 | 同 Linux | 没有信号，总有退出码 |
| 环境变量的名字 | 分大小写 | 分大小写 | 不分 |
| 标准错误和标准输出的先后 | 照写出来的先后 | 同 Linux | PowerShell 两条输出各有缓冲，先后不一定 |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/shell.rs` | 说明里写的是这台机器的 shell、换过字段、不报路径；在工作目录里跑；标准错误合进来、照先后；退出码；没有输出；超时整组杀、说法的秒数；只拿到白名单上的变量、`GIT_TERMINAL_PROMPT` 是 0；中文照原样；太长截成头尾、一共多少个字，尾巴从行首起的不多丢一行；参数不对、要放后台的不跑、写了 `description` 照跑；工作目录不在、`~` 开头的照家目录接；输出边跑边推、最后半个字换成 `�`；Unix：放到后台的在命令退出以后停了、叫停时整组停了、被信号杀掉 |
| `crates/miyu-basesystem/src/shell/tests.rs` | 超时的上下限；秒数怎么写；参数格式里写的上限、默认值和代码一样；每一段以换行结尾 |
| `crates/miyu-basesystem/src/shell/output/tests.rs` | 数行、数字；`\r\n`；内存只留头尾；截在行尾；中间丢过的照样数；一整行很长的照字数截；正好 30000 个字的整段给；切开的字等配齐；离截处太远的换行不用 |
| `crates/miyu-basesystem/src/shell/program/tests.rs` | 各系统用哪个 shell、找不到时用什么；名字带版本；不读启动文件、只带给的变量；zsh 带 `+o nomatch`、没匹配到的通配符原样传下去（macOS）；PowerShell 的编码和前面那一句 |
| `crates/miyu-basesystem/src/shell/process/tests.rs` | 杀不掉的命令，最多再等那么久就交回超时 |
| `crates/miyu-basesystem/tests/log.rs` | 运行日志的来源是 `shell`；输出还没关那一行在阻塞线程里发，带会话编号（Linux） |
| `crates/miyu-basesystem/src/shell/env/tests.rs` | 只传名单上的；`GIT_TERMINAL_PROMPT` 总是 0；Windows 上名字不分大小写；前缀只看开头 |
| `crates/miyu-basesystem/tests/human.rs` | 每一种结果的说法，两种语言都换得出字 |
| `crates/miyu-session/tests/write.rs` | 会话里真的跑：结果进日志，给她的是输出加退出码，说法里有退出码，没有效果 |
| `crates/miyu-session/tests/guard.rs` | 工作区这一级执行命令不问、只读时问（没人能确认就拒绝） |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `10-自带软件.md` 第三节（「`shell` 的细则」）、第五节（`shell` 不报改了哪些文件）、第七节（经过 `shell` 的改动撤销不了）、第八节（B8：各平台的 shell）、第十节（不记 `cd`；先不声明 `description`、`run_in_background`）。
- `11-权限与沙盒.md` 第二节（M5 之前工作区这一级执行命令不问）、第四节（环境变量只传白名单）。
- `26-提示词.md` 第十节：登记簿里的 `tools/shell.json`、`shell/*.txt`。

### 还没有的

- 后台命令、`run_in_background`、`description`、查看和停掉后台任务的 `jobs`：随 M7（`10-自带软件.md` 第三节）。
- 沙盒：命令现在以本人的身份直接跑，碰得到任何地方；只读时每条都问人（`11-权限与沙盒.md` 第二节、第六节，M5）。放行规则照命令开头记，也随 M5（第二节）。
- Windows 上改用 Git Bash 的配置（`10-自带软件.md` 第八节）。
- 环境变量的名单是策略数据，配置那一步能改（`11-权限与沙盒.md` 第四节）：现在写在代码里。
