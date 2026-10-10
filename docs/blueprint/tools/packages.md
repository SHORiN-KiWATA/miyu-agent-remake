## `packages`

### 是什么

她看软件包（施工 F-10 上，设计 `31-软件包.md` 第六节第 1 条「查」）：列出装了的、看一个、装之前看一个包文件夹装上会是什么样。只看，哪一级都不问人。包目录在数据根里，文件工具碰不到（`11-权限与沙盒.md`）：不给这件，她不知道装了什么、自己照包的样子写好的文件夹写对没有。装、卸、打开她装的扩展是 F-10 下的另一件（`package`，访问类别要照级别问人），不在这一件里：访问类别是一件工具定死的（内核只读时照它拦、派的时候照它排），看和改分两件，名字一看就分得开（主会话定）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/packages.rs` | 参数、经端口看、写给她看的几行；看包文件夹的报那个路径是读（`targets`） |
| `crates/miyu-tool/src/packages.rs` | 看软件包的端口 `PackagesPort`、拒了的 `PackageRefusal`、那件工具的名字 `PACKAGES`（`tools/interface.md`） |
| `crates/miyu-endpoint/src/packages/port.rs` | 协议端点这一头：照 `package.list`、`package.info`（多列表那一项的 `name`、`summary`、`kind`）、`package.install` 带 `preview` 的算，名字、说明、写错的那一句照英文；拿核心的弱引用 |
| `crates/miyu-session/src/open/setup.rs`、`open.rs`、`open/load.rs`、`tools/kit.rs`、`tools.rs` | 造会话、载入时交进来的端口，原样带进每一次调用（`Call::packages`） |
| `resources/software/basesystem/tools/packages.json` | 说明和参数格式 |
| `resources/software/basesystem/packages/*.txt` | 输出里给她看的几句 |
| `resources/packages/basesystem/package.toml` | 功能「软件包」（`packages`），预设能关 |
| `resources/software/basesystem/human/{zh,en,ja}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `read`。看包文件夹的报那个路径是读（`Target { write: false }`），权限策略照路径判：只碰得到自己工作区的会话（群会话这些，`session/guard.md` 第四条末）看不了工作区外的文件夹，数据根哪一级都不许；列出、看一个不报路径。不报效果。

样本 `resources/software/basesystem/tools/packages.json`：

```json
{
  "description": "List installed Miyu packages, show one by id, or check a package folder before installing it.",
  "parameters": {"type":"object","properties":{"package":{"type":"string","description":"Package id to show."},"path":{"type":"string","description":"Absolute path of a package folder to check."}}}
}
```

- 两个参数都不写：列出。`package`：看一个。`path`：看一个包文件夹，要绝对路径。别的参数不认，也不报错。
- token 等实测（`26-提示词.md` 第十节）：基础系统的工具说明加上它一共 9448 字节，超了 9200 字节的预算（`10-自带软件.md` 第九节），量过照同一个办法改预算。

### 怎么走

1. 没有端口（`Call.packages` 是空的：测试里的假调用、核心没交的）：`unavailable.txt`，出错，说法 `packages/failed`。
2. `package`、`path` 都写了：`both.txt`，出错，不问端口。`path` 不是绝对路径：`relative.txt`，出错，不问端口。
3. 列出：照 `package.list` 一个一行（`listed.txt`：编号、名字、一句说明）；关着的（`enabled` 是假）`listed-off.txt`，卸掉了的出厂包 `listed-removed.txt`，清单读不了的 `listed-broken.txt`（哪里不对）。没写说明的那一格空着，不留两个空格。说法 `packages/listed`。
4. 看一个：照原样交回核心那一份 JSON（紧凑，`package.info` 加上列表那一项的 `name`、`summary`、`kind`），一行；没有这个包 `unknown.txt`。说法 `packages/shown`（字段 `package`）。
5. 看包文件夹：装得上的 `inspected.txt` 一行，下一行接核心看一眼的那一份 JSON（`package.install` 带 `preview` 的回应，`protocol.md`）：JSON 另起一行，模板里换进去的字会转义引号。清单写错的照真装那样拒：`invalid-line.txt`（知道第几行）、`invalid.txt`，哪里不对照英文。说法 `packages/inspected`（字段 `path`）。
6. 别的拒绝（读不了 `path_unreadable`、和出厂的撞名 `package_exists`、核心正在停 `shutting_down`）：`refused.txt`，看的是什么、原因代码照协议原样。说法 `packages/failed`。
7. 叫停的旗举起来了：交回「停下了」。

### 样子

```text
basesystem: Base system. Read and edit files, run commands, look up earlier talk
echo: Echo. Says it back Turned off.
net: Net. Removed.
```

```text
/home/me/work/pudding is a valid package. Installing it gives:
{"files":3,"package":"pudding","program":"process","size":1536,"version":"1.2.0"}
```

给她的字都在 `resources/software/basesystem/packages/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节（token 等实测）。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-basesystem/tests/packages.rs` | 只读、看文件夹报那个路径；列出一个一行，关着、卸掉、读不了的各一种说法；看一个照原样；看文件夹装得上的、写错带行号的、读不了的；参数写错不问端口；没有端口；说法中文、英文都换得出字 |
| `crates/miyu-endpoint/tests/packages_tool.rs` | 真核心的会话照剧本调三次：列出、看 `basesystem`、看一个包文件夹，经端口看到的和协议同一份、名字照英文，看文件夹什么都不装；核心重启以后载入的会话照样看得到 |
| `crates/miyu-basesystem/tests/budget.rs` | 工具面的预算：加了这件超了，等实测改 |

### 还没有的

- 装、卸、打开她装的扩展、批它要的能力：F-10 下（`package`，新的访问类别照级别问人，问人的卡片照看一眼的那一份）。
- 出厂带一个教她写包的技能：技能那一段（扩展性）做了再加。
