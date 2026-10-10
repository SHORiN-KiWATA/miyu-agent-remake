## 软件包页和软件后台

状态：「清单多的几格」「`package.list` 每一项多的几格」「程序不在就当没装」「开关」「`config.schema` 里包的配置项」施工 F-6 上做了，「`package.file`」「`package.methods`、`package.call`、`method.call`」施工 F-6 中做了（2026-10-10）；「网页软件怎么给后台页」「框和网页之间怎么说」「终端」是网页、终端的会话照着做的图纸。2026-10-10 主会话起草，网页、终端、接入QQ 三个会话同一天对过（改了五处：开关的格叫 `enabled`；界面包的配置项不看程序在不在；通道只交一次；响应头照 `web-ui.md`；`context` 多 `colors`）。设计 `30-插件框架.md` 第十三节：第 1 到 7 条项目主人定，这一页是第 8 条的技术形状。照它施工 F-6 上、F-6 中；网页软件给后台页的文件、框和网页之间的通道由网页的会话做，`miyu web --package` 也是它的。

### 是什么

头的「软件包」页列装了的软件，点进去是核心照清单生成的信息页；「软件后台」页列带自己页面的软件，网页软件把软件自己的页面放在隔离的框里显示。这一页写清单多的几格、协议多的几样、网页软件怎么给页面、框和网页之间怎么说话。

### 清单多的几格

```toml
[package]
icon = "message-circle"   # 可以不写：Lucide 的图标名

[page]                    # 软件后台页，可以不写；只有扩展（process）、内置包（builtin）能写
dir = "page"              # 包目录 packages/<编号>/ 下的子目录，入口是里面的 index.html
```

1. `icon`：小写字母开头，只有小写字母、数字、`-`，最多 64 个。核心只查写法，不查 Lucide 里有没有；头认不出的照没写画。不合写法的 `bad_icon`。
2. `[page] dir`：相对包目录的路径，写法同 `pages_dir` 的规矩（不能是绝对路径，不带 `..`、`\`、`:`），不合的 `bad_page_dir`。别的种类写了 `wrong_kind`。目录里没有 `index.html` 的不报错，`package.list` 不带 `page`，`miyu check` 报一条警告 `page_missing`。
3. 包目录照 `miyu_store::packages::Found::files_dir` 算：出厂的是资源目录的 `packages/<编号>/`，管理员装的是家目录的 `packages/<编号>/`（装包时清单旁边的同名目录一起拷，F-5 上）。

### `package.list` 每一项多的几格

| 格 | 有的时候 | 是什么 |
|---|---|---|
| `icon` | 清单写了 | 照原样 |
| `page` | 有后台页 | `true`：清单写了 `[page]`，`index.html` 在 |
| `status` | 读成了的 | `running`、`starting`、`stopped`、`off`、`ready`、`program_missing` 之一，见下 |
| `enabled` | 有开关的 | 开关现在是开是关，见「开关」 |

`status`：

1. `program_missing`：要程序的包（扩展、界面、小程序），程序不在 `miyu` 旁边（找法同 `packages.md`「转交」）。
2. `off`：关着。扩展的开关关着；出厂的包卸掉了（`removed: true`）。
3. `running`、`starting`、`stopped`：扩展照 `extension.status` 的 `state`，`waiting` 算 `starting`。停下的原因照旧在 `extension.status`。
4. `ready`：别的。内置包装着，小程序、界面的程序在。

头照它写「运行中」「启动中」「已停止」「已停用」「已启用」「程序未安装」。

### 程序不在就当没装

`status` 是 `program_missing` 的扩展、小程序（核心拉起的那两种）：

1. 它的配置项不进 `config.schema`、参考文件，和没装一样；写在配置文件里的不报不认识（同没装的包，`config.md`）。
2. `package.enable`、`extension.enable` 拒绝，`program_missing`。
3. 它带的功能不进预设（`preset.get` 照「写了没装」列，`installed: false`）。内置包没有程序，不会是这一种。
4. 程序放回去以后：核心下一次重读清单时照新的算。装卸以后、核心起来时都重读；人手放回程序的要重启核心，或者再开一次开关（`package.enable` 先重查一遍程序在不在）。
5. 界面包（`ui`）的程序不在，`status` 照样写 `program_missing`，配置项照旧在：界面不是核心拉起的，它的配置项就是给那个界面自己读写的，连得上核心的界面自己就是程序在的证明（终端的会话 2026-10-10 提：开发时界面的程序不在 `miyu` 旁边，引导写 `tui.icons` 会被拒）。

### 开关

列表上的开关，头只管开、关，怎么做由核心照种类定：

| 种类 | `enabled` | `package.enable` | `package.disable` |
|---|---|---|---|
| 扩展 | 开关开着 | 同 `extension.enable`（要批的能力照它的 `approve`、`needs_approval`） | 同 `extension.disable` |
| 出厂的内置包（不是必需的） | 没有卸掉 | 同 `package.install {package}`：装回来 | 同 `package.remove {package}`：卸掉（在家目录记一笔） |
| 必需的、界面、小程序 | 没有这一格 | `not_switchable` | `not_switchable` |

1. 参数 `{package}`，`package.enable` 另可带 `approve`（同 `extension.enable`）。回应同 `package.list` 的一项。
2. 没有的包 `unknown_package`；程序不在的 `program_missing`；必需的 `package_required`。
3. 信息页的「卸载」照旧是 `package.remove`：出厂的包卸掉和关掉是一回事，头只给一处（列表的开关）；管理员装的包卸掉是删文件，只在信息页给。

### `config.schema` 里包的配置项

1. 包的配置项（清单的 `[settings]`、核心替内置包声明的）多一格 `package`：包的编号。页一律是 `packages`，组是包的编号，组名照包的名字。头照 `package` 把它们画在那个软件的信息页上。
2. `connections` 页去掉：F-4 起平台接入的配置项放进 `connections`，现在和别的包一样。
3. 页和组只由核心定，清单写不了；键是 `<包的编号>.<名字>`，撞不到核心的、别的包的。
4. 核心自己的页：`general`、`models`、`advanced`（`permissions` 施工 F-4 再补并进 `general`）。

### `package.file`

读一个包的后台页里的一份文件。网页软件给框里的页面时用，只给出示本机令牌、登录令牌的连接（扩展调回 `local_only`）。

| 参数 | 类型 | 说明 |
|---|---|---|
| `package` | 字符串，必写 | 包的编号 |
| `path` | 字符串，必写 | 后台页目录里的相对路径，`/` 隔开；空的是 `index.html` |
| `offset` | 非负整数，可以不写 | 从第几个字节读起，不写是 0 |

回应 `{"data": <base64>, "size", "eof"}`：一次最多 512 KiB（同 `fs.read`），`size` 是整份多大，`eof` 读到头了没有。媒体类型不给：网页软件照扩展名查它自己给页面用的那张表（`web.json` 的 `types`），核心不另放一份（施工 F-6 中定）。

1. 路径：`/` 隔开的一段段，有空段、`.`、`..`、`\`、`:`、控制字符的 `bad_params`（以 `/` 开头的就有空段）；换成真实的位置以后跑出后台页目录的（链接指出去的）、不是普通文件的、没有的 `not_found`。
2. 没有这个包、没装的 `unknown_package`；没有后台页的 `no_page`。

### `package.methods`、`package.call`、`method.call`

后台页调它自己的程序：程序先登记方法，头经核心转过去。

1. **登记**：扩展连上以后发 `package.methods {"methods": [{"name", "timeout_ms"?}]}`。
   - `name`：小写字母开头，小写字母、数字、`_`、`.`、`-`，1 到 64 个；同一次里不重复；`timeout_ms` 同工具的，1000 到 600000，不写是 30000。
   - 换掉这个包上一次登记的；不用另批能力：只有管理员的头在这个软件自己的页上调得到。
   - 不缓存：连接断了、扩展停了就没有了，调到的回 `program_not_running`。
   - 不是核心拉起的扩展的连接调它，`not_an_extension`。名字写法不对、重复、时限不在范围里的 `bad_params`，什么都不换。回应 `{"methods": 个数}`。
2. **调**：头发 `package.call {"package", "method", "params"?}`。只给出示本机令牌、登录令牌的连接。
   - 没有这个包 `unknown_package`；它的程序没连着、还没登记方法 `program_not_running`；没登记这个方法 `unregistered`，`data.method` 是哪一个。
   - 核心发反向请求 `{"jsonrpc":"2.0","id":"core-<n>","method":"method.call","params":{"method","params"}}` 给扩展，等它回，回什么交回什么（`result` 原样）。
   - 扩展回了错：`method_failed`，`data.message` 是它说的那一句，`data.code` 是它的错误码（没有的不写）。到了 `timeout_ms` 没回：`method_timeout`，核心不再等，扩展晚回的扔掉。等着时连接断了：`program_not_running`。
   - 在后台答：这个连接后面的请求不等它（同 `link.preview`，`methods::answered_later`）。
3. 扩展这边：收到 `method.call` 照自己的方法表办；不认识的回 JSON-RPC 的 `-32601`（核心交回 `method_failed`）。

### 网页软件怎么给后台页（网页的会话做，`web-ui.md` 跟着写）

1. `POST /page`：`Authorization: Bearer <登录令牌>`，正文 `{"package"}`。网页软件照这个令牌连核心，`package.list` 查它有没有后台页，没有的 404；有的造一张票据（同媒体地址的票据，第三条第 4 款），回 `{"url": "/p/<票据>/<包的编号>/"}`。
2. `GET /p/<票据>/<包的编号>/<路径>`：票据不认识、包对不上的 404；照 `package.file` 一块块读、一块块写。相对路径照目录解析，页面里的相对地址自然带着票据。
3. 响应头照 `web-ui.md`（网页的会话施工时在几种浏览器上实测后写定）。要守的几条：沙箱，只许脚本、表单；不许联网（`connect-src 'none'`）；只许嵌在网页软件自己的页面里；`nosniff`、`no-referrer`、`Cache-Control: no-cache`。框的来源是空的（`null`），ES 模块脚本、`@font-face` 字体照跨源取，`/p/` 的回应要带 `Access-Control-Allow-Origin`：票据本身就是凭据，不多开口子。
4. 框：`<iframe sandbox="allow-scripts allow-forms" allow="clipboard-write" src="<url>">`，不给 `allow-same-origin`；`clipboard-write` 让页面能把令牌这类字写进剪贴板，不给读（2026-10-10 网页的会话实测：不给时 `navigator.clipboard.writeText` 报 `NotAllowedError`）：页面在一个空的来源里跑，读不到网页的存储、口令，`connect-src 'none'` 让它连不了网、连不了 `/ws`。
5. 有后台页在给，网页软件不算空闲（同 `/media`）。

### 框和网页之间怎么说（网页的会话做）

1. 框第一次载入完，网页造一个 `MessageChannel`，把一头经 `postMessage({"miyu": "port"}, "*", [port])` 交给框；以后两边只在这条通道上说。只交这一次：框后来再载入，不管是跳到自己目录里别的页还是跳出去，都不再交。沙箱里的框能把自己导航到外面的网址，网页分不出载入的是谁。所以后台页要做成单页。
2. 照 JSON-RPC 2.0 的写法：框发请求，网页回；网页另发推送（不带 `id`）。
3. 方法表，只有这几样：

| 方法 | 参数 | 网页怎么办 | 回 |
|---|---|---|---|
| `context` | 无 | 照网页这时的样子答 | `{"package", "language", "theme": "light" \| "dark", "colors"?}`：`colors` 是网页这时的几个主色（`accent`、`surface`、`surface_2`、`text`、`text_soft`、`line`、`danger`），可以没有 |
| `settings.get` | 无 | `config.schema`、`config.get`，只留 `package` 是它的项 | `{"items": [...], "values": {键: 最终值}}`；密钥照 `config.get` 的写法，不给值 |
| `settings.set` | `{"changes": [...]}`，写法同 `config.set` | 每一项的键要是 `<它的编号>.` 开头，不是的整个不办，回 `forbidden`；照 `config.set` 写进系统配置。密钥类型的项给的是字的，网页照设置页画密钥框的办法先 `secret.set` 存成密钥、再写成 `{ secret = … }` 的引用（2026-10-10 主会话定：接入QQ 的令牌要在后台页上生成、换） | `config.set` 的回应 |
| `call` | `{"method", "params"?}` | `package.call`，`package` 照框是谁的填，框写不了 | `package.call` 的回应 |

4. 推送：`settings.changed {"keys"}`（`config.changed` 里有它的项时）、`theme.changed {"theme", "colors"?}`。
5. 别的方法回 `-32601`；写法不对的回 `-32600`。网页认的一律照它给框的那个包，框说自己是谁不算数。

### 终端

1. 「软件包」页、信息页照上面的协议画；`page: true` 的写一行「这个软件有自己的页面，在网页里打开」，接一条命令 `miyu web --package <编号>`。
2. `miyu web --package <编号>`：同 `miyu web`，打开的地址直接到「软件后台」里这个软件（网页软件的路由，`cli/web.md`）。

### 接入QQ 怎么挪过来（接入QQ 的会话做）

现在接入QQ 自己起一个网页（`web` 端口、`miyu-onebot web`）。挪成后台页：页面放进 `packages/onebot/page/`，页面要的操作做成桥登记的方法，经 `package.call` 调；桥自己的网页端口、一次性码去掉。先后和细节和接入QQ 的会话对。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-config/src/package/look/tests.rs`（施工 F-6 上） | `icon`、`[page] dir` 读得出；写法不对的 `bad_icon`、`bad_page_dir`，报在那一行；`[page]` 只给扩展、内置包 |
| `crates/miyu-endpoint/tests/package_switch.rs`（施工 F-6 上） | `package.list` 的 `status`、`enabled`、`icon`、`page`（目录里没有 `index.html` 的不带）；开关开、关扩展，出厂内置包卸掉、装回来；必需的、界面、程序不在、没有的拒绝；程序不在的配置项不进 schema、功能不进预设，界面的照旧；`miyu check` 报 `page_missing` |
| `crates/miyu-endpoint/tests/package_settings.rs`（施工 F-6 上改） | 包的配置项带 `package`，核心自己的不带；平台接入的在「软件包」页，没有 `connections` 页 |
| `crates/miyu-endpoint/tests/extensions.rs`（施工 F-6 上改） | 程序不在的 `extension.enable` 回 `program_missing` |
| `crates/miyu-endpoint/src/backstage/file/tests.rs`、`calls/tests.rs`（施工 F-6 中） | 后台页里的路径怎么拆、哪些不收；方法名的写法 |
| `crates/miyu-endpoint/tests/backstage.rs`（施工 F-6 中） | `package.file` 读入口、子目录、从中间读、超过 512 KiB 分两块；出目录的、目录、没有的、没有后台页的、没有的包；链接指出去的（Unix）；测试用的扩展登记三个方法，调得到、回错、超时、没登记、包没有、关掉以后没连着；扩展登记不了写法不对的名字、调不了 `package.call`、`package.file`，人登记不了方法 |

### 还没有的

- 第三方包的后台页能不能直接用核心的别的能力（读会话这些）：现在只能经它自己的程序。
- 后台页要看密钥：`settings.get` 不给值，要给看的由它的程序经方法交回（接入QQ 的令牌就这样，项目主人 2026-10-08 定了令牌能看能复制）。核心不为后台页开交出密钥的口子。
- 后台页的文件改了要不要推给开着的框：现在不推，重开就是新的。
