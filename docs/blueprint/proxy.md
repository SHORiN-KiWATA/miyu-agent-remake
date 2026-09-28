## 代理：`miyu-proxy`

### 是什么

核心里的 HTTP 代理：沙盒里的命令联网都经它。公开网站直接通，不问人；本机、内网、云上的元数据地址拦下，这台机器自己网卡上的地址也算本机。它自己解析目标的名字，解析出来的每一个地址都核对，只连核对过的，中间不再解析：DNS 重绑定骗不过去。

认两种请求：`CONNECT`（HTTPS 走它，不解密，两头原样对拷）和普通 HTTP 的转发。核心起来时开它，只在 `127.0.0.1` 上听。

逼着沙盒里的命令只能走它，是各平台的事：Linux 5-6、macOS 5-7、Windows 5-8、5-9。规格里网络那一格写它的地址（`sandbox.md`），由 5-4 填。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-proxy/src/lib.rs` | 交出去的几样：`start`、`Proxy`、`PORTS` |
| `crates/miyu-proxy/src/listen.rs` | 在一段端口里找一个绑上；一个个接连接，同时在转的有上限；丢掉 `Proxy` 就停 |
| `crates/miyu-proxy/src/head.rs` | 读请求头：多长、等多久；读成 CONNECT 或者转发；转发时请求头怎么改 |
| `crates/miyu-proxy/src/target.rs` | 目标的主机、端口怎么读，哪些写法不认 |
| `crates/miyu-proxy/src/address.rs` | 一个地址是不是公网的，纯函数 |
| `crates/miyu-proxy/src/net.rs` | 碰系统网络的三样 `Net`：解析、判是不是本机、连接；真的是 `System` |
| `crates/miyu-proxy/src/dial.rs` | 解析一次、逐个核对、错开着连核对过的，有时限 |
| `crates/miyu-proxy/src/serve.rs` | 一个连接从头到尾：读头、找目标、连上、回话、对拷、记日志 |
| `crates/miyu-proxy/src/reply.rs` | 出错时回的几种 |
| `crates/miyu-proxy/src/testkit.rs` | 测试用：假的 `Net`、改小的上限和时限，`testkit` 开关打开才编进去 |
| `crates/miyu-core/src/proxy.rs` | 核心起来时开代理，记一行（`core.md`） |

### 对外的样子

**给核心的**

| 名字 | 是什么 |
|---|---|
| `start(ports)` | 在 `127.0.0.1` 上、`ports` 这一段里挑第一个绑得上的端口听（核心传 `miyu_sandbox::PROXY_PORTS`，`sandbox.md`「代理的端口段」：28480 到 28511），交回 `Proxy`；整段都绑不上交回出错。给 `0..=0` 的由系统随便给一个，测试用。要在 tokio 的运行时里调：接连接的任务起在它上面 |
| `Proxy` | 开着的代理。`addr()` 是它听的地址，例如 `127.0.0.1:28480`，规格的 `{"proxy": "<addr>"}` 写的就是它。丢掉它就停：不再接连接，在转的连接一起断开 |
| `testkit::start(ports, net, limits)` | 只在 `testkit` 开关打开时有：碰网络的三样换成 `net`（`Net`），上限和时限换成 `limits`。只多出这个入口，`start` 一点不变 |

**认的请求**：HTTP/1.0、1.1。

| 请求行 | 怎么办 |
|---|---|
| `CONNECT <主机>:<端口> HTTP/1.1` | 连上目标以后回 `HTTP/1.1 200 Connection established`，之后两头原样对拷，不解密 |
| `<方法> http://<主机>[:<端口>][<路径和参数>] HTTP/1.1` | 连上目标以后照「转发普通 HTTP」改好请求头发过去，之后两头原样对拷。端口不写是 80 |
| 别的 | 400 |

**目标的写法**

| 格 | 认的 | 不认的（400） |
|---|---|---|
| 主机 | 方括号里的 IPv6，照标准库读，不带 `%` 的区域；字母、数字、`.`、`-`、`_` 组成的 1 到 253 个字：标准库读得成 IPv4 的就是地址本身，别的是名字 | 空的、别的字、不带方括号的 IPv6、带 `@` 的（用户名密码） |
| 端口 | 十进制的 1 到 65535 | `0`、带正负号的、不是数字的、太大的；CONNECT 没写端口的 |
| scheme | 转发只认 `http://`，不分大小写 | 别的，`https://` 请用 CONNECT |

**只通公网**：地址先换回 IPv4（IPv4 映射的 IPv6，`::ffff:127.0.0.1` 就是 `127.0.0.1`），再照下面两张表判，表里的一律拦。

IPv4：

| 段 | 是什么 |
|---|---|
| `0.0.0.0/8` | 「这个网络」：连 `0.0.0.0`，Linux 上连的是本机 |
| `10.0.0.0/8`、`172.16.0.0/12`、`192.168.0.0/16` | 内网 |
| `100.64.0.0/10` | 运营商级 NAT 的共享地址：阿里云的元数据 `100.100.100.200`、Tailscale 都在这里 |
| `127.0.0.0/8` | 回环 |
| `169.254.0.0/16` | 链路本地：各家云的元数据 `169.254.169.254` 在这里 |
| `192.0.0.0/24` | IETF 协议分配：Oracle 云的元数据 `192.0.0.192` 在这里 |
| `192.0.2.0/24`、`198.51.100.0/24`、`203.0.113.0/24` | 文档用的 |
| `198.18.0.0/15` | 测评网络用的；透明代理 fake-ip 模式的假地址也在这里（施工单 5-5 待拍板） |
| `224.0.0.0/4` | 组播 |
| `240.0.0.0/4` | 保留，连同广播 `255.255.255.255` |
| `168.63.129.16` | Azure 的宿主通信地址：落在公网段里，单列 |

IPv6：只放全球单播 `2000::/3`，其中下面几段也拦。NAT64 的 `64:ff9b::/96` 是例外，照末尾嵌着的 IPv4 判：只有 IPv6 的网络里，只有 IPv4 的网站解析出来是这样。

| 段 | 是什么 |
|---|---|
| `2001::/23` | IETF 协议分配，连同 Teredo `2001::/32` |
| `2001:db8::/32`、`3fff::/20` | 文档用的 |
| `2002::/16` | 6to4：里面嵌着一个 IPv4 地址 |
| `2000::/3` 以外的 | 回环 `::1`、未指定 `::`、嵌 IPv4 的旧写法 `::/96`、唯一本地 `fc00::/7`（AWS 的元数据 `fd00:ec2::254` 在这里）、链路本地 `fe80::/10`、旧的站点本地 `fec0::/10`、组播 `ff00::/8`，都拦 |

**本机**：过了上面两张表的，再问系统往这个地址发包用哪个源地址（开一个 UDP 套接字 connect 过去，不发包）。源地址就是它自己的，是这台机器网卡上的地址，例如公网的 IPv4、全球的 IPv6，也拦。问不出来的（没有路由这类），这个地址当连不上。

**常量**

| 常量 | 值 | 管什么 |
|---|---|---|
| `HEAD_LIMIT` | 64 KiB | 请求头最长多少字节 |
| `HEADERS` | 100 | 请求头最多几行 |
| `HEAD_TIMEOUT` | 30 秒 | 连上以后多久要把请求头发完 |
| `DIAL_TIMEOUT` | 30 秒 | 解析加连接一共等多久 |
| `STAGGER` | 250 毫秒 | 一个地址还没连上，隔多久接着连下一个 |
| `CONNECTIONS` | 64 | 同时在转的连接最多几个 |
| `PAUSE` | 100 毫秒 | 接连接出错以后歇多久 |

### 怎么走

**开**（`start`）

1. 照 `ports` 从小到大，在 `127.0.0.1` 上绑，绑上了就是它。这一个被占了（地址在用；没有权限，Windows 上 Hyper-V、WSL 保留的端口是这样）换下一个；别的错马上交回。整段都绑不上：出错 `no free port in <头>-<尾>`。
2. 只听 `127.0.0.1`：`::1`、`0.0.0.0` 都不听，别的机器连不上。
3. 不设密码：它只通公网，本机上的程序自己本来就连得上公网（`11-权限与沙盒.md` 第四节）。
4. 在调它的运行时里起接连接的任务。

**接连接**

1. 一个个接，每个连接一个任务。接出错（打开的文件太多这类）：记 `WARN accept failed`，歇 `PAUSE` 再接。
2. 同时在转的已经 `CONNECTIONS` 个：回 503，断开，记 `WARN too many connections`。免得沙盒里的命令开太多连接，耗尽核心的文件句柄。
3. 转一个连接的任务 panic 了：记 `ERROR connection task failed`，别的连接照旧。
4. 丢掉 `Proxy`：接连接的任务停下，在转的连接一起断开。

**读请求头**

1. 连上以后 `HEAD_TIMEOUT` 以内要把请求头发完（读到空行为止）。到时了直接断开，不回；没发完对方就关了，也断开。
2. 超过 `HEAD_LIMIT` 字节、超过 `HEADERS` 行的回 400（`head too large`）；写法不对、不是 HTTP/1.0、1.1 的回 400（`malformed head`）。
3. 方法是 `CONNECT` 的，目标要写成 `<主机>:<端口>`；别的方法，目标要是带 scheme 的完整地址，只写路径（`/…`）、`*` 的回 400（`not a proxy request`），scheme 不是 `http` 的回 400（`only http:// is forwarded, use CONNECT for https`）。主机、端口不认的回 400（`bad host`、`bad port`）。
4. 请求头后面已经读到的字节（紧跟着 CONNECT 发来的 TLS 握手、转发的请求体开头）留着，连上目标以后先发过去。

**找目标、核对、连**

1. 主机是 IP 地址的，就是它，不解析；是名字的，交给系统解析一次（`getaddrinfo`）。`127.1`、`2130706433` 这类由系统认的数字写法，判的也是解析出来的地址。
2. 每个地址换回 IPv4、照「只通公网」判、再判是不是本机，不过的丢掉。一个名字另外解析出内网地址的，只连公网的那几个：连的都核对过。
3. 一个都不剩：有被拦的，回 403，写上第一个被拦的地址，记 `INFO blocked`；都是问不出来的，照第 5 条的「连不上」办。
4. 剩下的照解析的先后，IPv6、IPv4 交错着排，第一个是哪种就从哪种起。先连第一个；过了 `STAGGER` 还没连上，或者它已经失败了，接着连下一个，前面的不停；谁先连上用谁，别的停掉。
5. 解析不了、解析出来是空的（原话 `no addresses`）、都连不上（原话是最后一个的）：回 502。从开始解析算起过了 `DIAL_TIMEOUT` 还没连上：回 504。这几种都记 `DEBUG failed`。
6. 连上了记 `DEBUG connected`。
7. 连的就是核对过的那个地址，中间不再解析：核对时是公网、连的时候变成 `127.0.0.1` 的 DNS 重绑定骗不过去。

**CONNECT**

1. 连上以后回 `HTTP/1.1 200 Connection established\r\n\r\n`，把请求头后面已经读到的字节发给目标，然后两头原样对拷，不解密。
2. 一头读完了（对方关了写），就关另一头的写，另一个方向照转，两个方向都完了才算完；中间出错也算完。记 `DEBUG closed`。
3. 不设空闲超时：git、下载、WebSocket 这些长连接照样用。

**转发普通 HTTP**

1. 请求行改成 `<方法> <路径和参数> HTTP/1.<原来的小版本号>`，路径是空的写 `/`。
2. 请求头照原来的先后抄过去，去掉 `Host`、`Connection`、`Proxy-Connection`、`Keep-Alive`、`Proxy-Authorization`（名字不分大小写），末尾加上 `Host: <主机>[:<端口>]`（端口是 80 的不写，IPv6 带方括号）和 `Connection: close`。
3. 发过去，接着发已经读到的请求体开头，然后照 CONNECT 第 2、3 条对拷。
4. 一个连接只转一个请求：`Connection: close` 让目标回完就关，客户端的下一个请求另开连接。同一个连接上接着发来的，都发给同一个核对过的目标，到不了别处。
5. 升级（`Upgrade`）做不成：WebSocket 走 CONNECT。

**上游代理**：不接（施工单 5-5 待拍板）。代理自己直连目标；环境里的 `HTTPS_PROXY` 这些只管核心自己请求模型（`http.md`）。

**核心怎么开**（`crates/miyu-core/src/proxy.rs`，`core.md` 起来的先后第 10 步）：`start(PORTS)`。开得了，记 `INFO core proxy addr=127.0.0.1:<端口>`，拿着 `Proxy` 到核心退出；开不了，记 `WARN core proxy unavailable error=<原话>`，核心照样起来。

**管不到的**

1. 透明代理（Clash、sing-box 的 TUN 这类）接管了这台机器的出口：包出了核心往哪去由它定，核对的只是交给它的地址。
2. 路由器把公网地址上的端口转发回这台机器（NAT 回环）：连的是路由器的公网地址，照公网放行。
3. 本机上的虚拟机、容器用了公网 IPv6 的：不是这台机器网卡上的地址，照公网放行。

### 出错

回完就断开。都带 `Content-Type: text/plain; charset=utf-8`、`Content-Length`、`Connection: close`，正文一行，末尾一个换行：

| 码 | 状态行 | 正文 | 什么时候 |
|---|---|---|---|
| 400 | `400 Bad Request` | `miyu proxy: bad request: <为什么>` | 请求读不懂、不认 |
| 403 | `403 Not a public address` | `miyu proxy: blocked: <主机> is not a public address (<地址>)` | 解析出来的地址都被拦了 |
| 502 | `502 Bad Gateway` | `miyu proxy: cannot reach <主机>: <原话>` | 解析不了、解析出来是空的（原话 `no addresses`）、都连不上 |
| 503 | `503 Service Unavailable` | `miyu proxy: too many connections` | 同时在转的已经满了 |
| 504 | `504 Gateway Timeout` | `miyu proxy: timed out reaching <主机>` | 解析加连接过了 `DIAL_TIMEOUT` |

- `<为什么>` 是「读请求头」第 2、3 条里的那几句。`<主机>` 照请求里写的，IPv6 带方括号。
- 这几句她看得到（命令的输出），一律英文。只在出错时出现，不常驻，不进登记簿（照 `sandbox.md`「怎么走」第 6 条）。
- 403 的状态行写明为什么：curl、git 只印状态码，Python 这类会把状态行印出来。

开不了代理：`no free port in 28480-28511`，或者系统的原话；核心只记日志（上面「核心怎么开」）。

### 运行日志

目标 `miyu::proxy`，日志里写 `proxy`。没有会话编号：代理不知道是哪个会话的命令连的。

| 级别 | 这件事 | 键 | 什么时候 |
|---|---|---|---|
| DEBUG | `connected` | `method`、`host`、`port`、`addr`（连上的地址） | 连上了目标 |
| DEBUG | `closed` | `host`、`port`、`up`、`down`（对拷时两个方向各转了多少字节）、`took_ms`；出错结束的没有 `up`、`down`，有 `error` | 一个连接转完 |
| INFO | `blocked` | `host`、`port`、`addr`（第一个被拦的地址） | 拦下 |
| DEBUG | `failed` | `host`、`port`、`error` | 解析不了、连不上、到时 |
| DEBUG | `bad request` | `error`（400 的那句） | 请求读不懂、不认 |
| WARN | `too many connections` | `limit` | 回了 503 |
| WARN | `accept failed` | `error` | 接连接出错 |
| ERROR | `connection task failed` | `error` | 转一个连接的任务 panic 了 |

- 不写：路径和参数、请求头、请求体、对拷的字节（`log.md` 第 7 条）。
- 例子（地址、数是编的）：

```text
2026-09-28 22:10:03.118 INFO  core     proxy addr=127.0.0.1:28480
2026-09-28 22:11:40.502 DEBUG proxy    connected method=CONNECT host=index.crates.io port=443 addr=2600:9000:2352:8c00:1:ed25:8740:93a1
2026-09-28 22:11:41.911 DEBUG proxy    closed host=index.crates.io port=443 up=1523 down=48211 took_ms=1409
2026-09-28 22:12:05.007 INFO  proxy    blocked host=localhost port=8300 addr=::1
```

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-proxy/src/address/tests.rs` | 两张表里每一段的头尾，和紧挨着的外面（`100.63.255.255` 通、`100.64.0.0` 拦这样）；映射的换回 IPv4；NAT64 照嵌着的 IPv4；`2000::/3` 以外的；Azure 那一个 |
| `crates/miyu-proxy/src/target/tests.rs` | 主机、端口、scheme 的每种写法，认的和不认的 |
| `crates/miyu-proxy/src/head/tests.rs` | CONNECT、完整地址、只写路径的、`*`；太长、太多行、读不懂；转发时的请求行、去掉的几个头、`Host`、`Connection: close`，别的头照先后 |
| `crates/miyu-proxy/tests/connect.rs` | 假的 `Net` 连到本机的假目标：回 200、两头字节照原样、先到的字节先发过去、一头关了另一头跟着关 |
| `crates/miyu-proxy/tests/forward.rs` | 假目标收到的请求头照改法；请求体、响应原样；一个连接只转一个请求 |
| `crates/miyu-proxy/tests/blocked.rs` | 真的 `Net`：回环、`0.0.0.0`、`[::1]`、`[::ffff:127.0.0.1]`、`[::ffff:7f00:1]`、`[::127.0.0.1]`、`localhost`、`127.1`、`2130706433`、这台机器出去用的地址，CONNECT 和转发都回 403，假目标一个连接都没收到。数字写法 glibc 上是 403，别的系统解析不了的回 502，一样没连上 |
| `crates/miyu-proxy/tests/dial.rs` | 假的 `Net`：一个请求只解析一次，第二次说成本机也不管；连的都是核对过的；公网、本机混着的只连公网的；一个不剩回 403，都问不出来回 502；错开着连下一个；都失败回 502；到时回 504 |
| `crates/miyu-proxy/tests/limits.rs` | 请求头太长、太多行、读不懂回 400；太慢断开；超过上限回 503 |
| `crates/miyu-proxy/tests/listen.rs` | 第一个端口被占了用下一个；整段都占了出错；只听 `127.0.0.1`；丢掉就停，在转的也断开 |
| `crates/miyu-proxy/tests/net.rs` | 真的 `Net`：`localhost` 解析出回环；回环、这台机器出去用的地址是本机，别人的不是 |
| `crates/miyu-proxy/tests/log.rs` | 每一行；日志里没有路径、参数、请求头 |
| `crates/miyu-core/src/proxy/tests.rs` | 开得了记 `proxy addr=…`；那一段都被占了记 `proxy unavailable` |
| `crates/miyu/tests/core.rs` | 真的核心起来时开代理：记 `proxy` 那一行，端口在 `PORTS` 里 |

### 出处

- `11-权限与沙盒.md` 第四节：网络只经代理、A5（公开网站直接通，本机、内网、元数据地址一律拦）、代理怎么判；第六节：各平台逼着命令只能走代理。
- `01-架构.md` 第九节：第 3 层「执行器」。
- IANA 的特殊用途地址表（IPv4、IPv6）；RFC 6724 第 1 条：发给自己的地址，源地址就选它；RFC 8305：错开着连；RFC 9112 第 3.2.2 节：完整地址的请求，`Host` 照地址换。
- Codex 的 `network-proxy`（只借思路：地址怎么分；Windows 上端口固定在一段里）。

### 还没有的

- 规格里填代理的地址；代理开不了时，沙盒里不联网（5-4）。
- 逼着沙盒里的命令只能走代理：Linux 的网络命名空间（5-6）、macOS 的 Seatbelt（5-7）、Windows 的防火墙规则放行 `PORTS`（5-8、5-9）。
- 命令的环境变量加上代理那几个（`HTTP_PROXY`、`HTTPS_PROXY` 这些，大小写两份，写 `http://<addr>`），`NO_PROXY` 放过沙盒自己的回环（5-10）。
- 核心里的联网工具（`web_fetch`、`web_search`、下载）也经它（`11-权限与沙盒.md` 第四节）。
- 日志带会话编号：要给每个会话的命令一份凭据，代理才认得出是谁。
