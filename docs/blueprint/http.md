## HTTP：发请求、读流

### 是什么

HTTP 执行器：照驱动编码好的字节发一次请求，流式地读回来，边读边交给驱动的解码器，解出来的增量马上交出去。读到说完、出错、空闲超时或者被叫停为止。一次只发一回：重试、接着说是内核的事（`kernel/session.md`）；编码、解码、分类是驱动的事（`drivers/openai-chat.md`）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-http/src/lib.rs` | 对外的几样 |
| `crates/miyu-http/src/client.rs` | 客户端：TLS、`User-Agent`、连接超时、代理 |
| `crates/miyu-http/src/endpoint.rs` | 端点：地址、key、另配的头；打印时藏起 key |
| `crates/miyu-http/src/send.rs` | 发一次：头、空闲超时、出错、叫停、运行日志 |
| `crates/miyu-http/src/testkit.rs` | 测试用的假服务器，`testkit` 开关打开才编进去 |
| `crates/miyu-session/src/http.rs` | 用它的：会话请求模型的端口，取 blob、编码、发；空闲超时的初值 |
| `crates/miyu-core/src/models.rs` | 造客户端和端点 |

### 对外的样子

| 名字 | 是什么 |
|---|---|
| `client(Proxy)` | 造一个客户端（`Client`，就是 reqwest 的）；造不出来交回 reqwest 的错 |
| `Proxy` | `FromEnvironment`：照环境变量走代理，平时用；`Off`：不走代理，测试连本机的假服务器用 |
| `Endpoint::new(base_url, key)` | 发给谁：地址（例如 `https://api.deepseek.com`，路径由驱动接在后面）、key |
| `Endpoint::with_header(名字, 值)` | 另配一个头，照先后 |
| `Attempt` | 发一次要的：`client`、`endpoint`、`driver`（`Driver`）、`body`（编码好的字节）、`path`（编码交回的 `Encoded.path`）、`idle`（空闲超时） |
| `send(Attempt, cancel, on) -> Outcome` | 发一次；`cancel` 是一个 future，一完成就停；`on` 收一路上交出来的 `Progress` |
| `Progress` | `Sent { request }`：发出去了，`request` 是请求字节的内容哈希；`Delta(增量)` |
| `Outcome` | `Ended { usage, error }`：说完了或者出错了，`error` 是驱动的 `Classified`（分类、原话、要等多久）；`Cancelled`：被叫停了 |

| 常量 | 值 | 在哪 |
|---|---|---|
| 连接超时 `CONNECT_TIMEOUT` | 30 秒 | `client.rs` |
| 出错时读的响应体上限 `ERROR_BODY_LIMIT` | 64 KiB | `send.rs` |
| 空闲超时的初值 `IDLE` | 180 秒 | `crates/miyu-session/src/http.rs` |

### 怎么走

**客户端**

1. 一个核心一个（`crates/miyu-core/src/models.rs` 造一次），各会话拿它的克隆，连接池里的连接跨请求复用。
2. TLS 用 rustls，根证书认两份：系统里装的、webpki 自带的。不用 OpenSSL。reqwest 编进了 HTTP/2。
3. `User-Agent` 是 `miyu/<版本>`，版本是这个包的版本号。
4. 连上一个地址最多等 30 秒，连不上是可重试的错。
5. `FromEnvironment` 照环境变量 `HTTPS_PROXY`、`HTTP_PROXY`、`ALL_PROXY`、`NO_PROXY`（小写的也认），这是 reqwest 的默认做法；不读 Windows、macOS 的系统代理设置。`Off` 一概不走代理。

**发一次**

1. 地址是 `base_url` 去掉末尾的 `/`，接上 `path`：地址后面多写了斜杠，也不会成两个。
2. `POST`，请求体就是那串字节，一个字节不改。头照这个先后加：
   - `Authorization: Bearer <key>`
   - `Content-Type: application/json`
   - `Accept: text/event-stream`
   - 端点另配的头，照先后；和上面同名的，是再加一个，不是换掉。
   - `User-Agent` 由客户端带上。
3. 先报 `Sent`，带上请求字节的 SHA-256，再真的发：连不上的也报过了。
4. 等响应头：最多等 `idle`。
   - 发不出去（连不上、域名解析、TLS、地址或头写得不对）：没有状态，交给驱动分类，响应体是 reqwest 的错连同它的来由，一层层用 `: ` 接起来。
   - 等过了 `idle`：空闲超时，`retryable`。
5. 不是 2xx 的：一片片读响应体，每一片最多等 `idle`；读到 64 KiB、读完、读出错、等超时，就不读了，截到 64 KiB。连同状态、响应头（不是 UTF-8 的值，坏字节换掉）交给驱动分类。
6. 2xx 的：一片片读，每一片最多等 `idle`：
   - 读到一片，交给解码器，解出来的增量一条条交给 `on`；解码器说不用再读了（见到 `[DONE]` 或者出了错），停。
   - 读完了，停。
   - 读出错（读到一半断了），记下来由，停。
   - 等过了 `idle`：马上交回空闲超时，`retryable`；之前交出去的增量照样作数，解码器不收尾：`finish_reason` 已经到了、只差 `[DONE]` 的，也算空闲超时。
7. 停下以后解码器收尾（它知道说没说完）：
   - 它说是 `retryable` 的、又是读到一半断了的，原话换成「连接断了：<来由>」。
   - 说完了才断的，算说完了。
   - 正常说完的，收尾交回的增量（流完了才冲刷出来的那一条解出的、收块的 `End`）也交给 `on`；出了错的，一条都不交。
   - 交回用量和出错；这类出错没有要等多久。
8. **叫停**：发请求、等响应头、读每一片的时候，`cancel` 一完成（先看它，再看别的），马上交回 `Cancelled`，丢掉连接，不再交出任何东西。
9. 一次只发一回：怎么收场都交回去，不自己重试。

**会话怎么用它**（`crates/miyu-session/src/http.rs`）

1. 每次请求派一个任务，不占会话的 actor；任务带着会话的 span，这里的日志行跟着写上会话编号。
2. 照驱动列的清单，在阻塞线程里从 blob 取字节；取不出来的（丢了、坏了、读不了）不交。
3. 编码。缺了 blob 的：不发、不报 `Sent`，当场说完，分类 `other`，原话「编码要用的 blob <哈希> 取不出来」。
4. 发：`idle` 是 180 秒；`Sent` 报给内核「发出去了」，带上端点、模型、哈希；增量一条条报；`Ended` 报用量、出错，出错里的要等多久交给内核（`kernel/session.md`：要等超过 2 分钟的不等，这一轮以出错结束）；`Cancelled` 什么都不报。

### 出错

| 什么时候 | 分类 | 原话 |
|---|---|---|
| 发不出去 | 驱动分，没有状态：多半是 `retryable` | reqwest 的错连同来由 |
| 等响应头、等下一片超过 `idle` | `retryable` | `空闲超时：<秒> 秒没有收到新的内容`，秒数照小数写：180 秒写成 `180`，200 毫秒写成 `0.2` |
| 不是 2xx | 驱动分 | `HTTP <状态>: <原话>`（`drivers/openai-chat.md`） |
| 读到一半断了、没说完 | `retryable` | `连接断了：<来由>` |
| 流里报的错、流坏了 | 解码器分 | 见 `drivers/openai-chat.md` |

原话给查问题的人看，记进 `model.called`，不进上下文，也不进运行日志。

### 运行日志

来源 `miyu::http`（日志里写成 `http`），`DEBUG` 级，每次发两行：

| 行 | 什么时候 | 键 |
|---|---|---|
| `sent` | 开始发之前 | `host`、`bytes`（请求多少字节） |
| `ended` | 正常说完 | `host`、`status`、`took_ms` |
| `failed` | 出错 | `host`、`status`（收到了响应头才有）、`class`、`retry_after_ms`（有才写）、`took_ms` |
| `cancelled` | 被叫停 | `host`、`took_ms` |

- `host` 是 `base_url` 里的主机名，读不出来写 `?`。用时从开始发算起，毫秒的整数。
- 不写：key、请求体、回复里的字、地址的路径和参数（有的供应商把 key 放在地址里）、出错的原话（可能回显请求里的字）。
- 在会话里发的，行上带着会话编号。一行怎么排见 `log.md`：

```text
2026-09-27 21:03:18.411 DEBUG http     0199d1e6-3b7a-7c41-8e5d-2f6b4c9f02a3 sent host=api.deepseek.com bytes=5120
2026-09-27 21:03:20.104 DEBUG http     0199d1e6-3b7a-7c41-8e5d-2f6b4c9f02a3 ended host=api.deepseek.com status=200 took_ms=1693
```

**端点打印出来**（`{:?}`）：`base_url` 照写，key 写成 `***`，另配的头只写名字，值不写。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-http/tests/send.rs` | 先报 `Sent`、增量和直接解码一样；发出去的方法、路径（多一个斜杠不成两个）、四个头、另配的头、请求体一字不差；见到 `[DONE]` 就停；HTTP 出错交给分类、要等多久；停住了空闲超时、之前的增量照样交出；叫停马上停、连接断开；没人听是 `retryable`；说到一半断开是 `retryable`；声明了长度没写够是「连接断了：」；打印端点不漏 key 和头的值 |
| `crates/miyu-http/tests/log.rs` | 说完、限速、连不上、地址读不出主机名、叫停，各记哪两行；key、请求体和回复里的字、路径和参数、出错的原话一个字都不记 |
| `crates/miyu-session/tests/http.rs` | 经驱动和 HTTP 请求一次；限速了等够再请求；打断了断开连接；缺 blob 出错、不发；断了走接着写的路径；空闲超时；图片照字节发出去 |
| `crates/miyu-session/tests/http_log.rs` | HTTP 的两行带上会话编号 |

### 出处

- `05-内核接口.md` 第七节「HTTP 执行器」：发、读、空闲超时、出错、打断、一次只发一回、缺 blob、连接复用、密钥不进日志。
- `15-模型与供应商.md` 第五节：空闲超时的基数 180 秒。
- `28-运行日志.md` 第二节（一行怎么写）、第四节（写什么，不写什么）。
- `07-存储.md` 第九节：密钥永远不进日志。

### 还没有的

- 空闲超时按思考强度放大：高 2 倍、更高 3 倍、最高 4 倍（`15-模型与供应商.md` 第五节）。
- 等第一个字的时候定时给头发心跳（同上；`03-事件模型.md` 第五节 `status` 那一格）。
- 连接预热（`15-模型与供应商.md` 第五节）。
- 子进程的传输：借用 agent CLI 的订阅（`05-内核接口.md` 第七节 `transport`）。
- 端点从配置来、一个供应商几个 key：现在只有 `DEEPSEEK_API_KEY` 那一家（`15-模型与供应商.md` 第二节、M4）。
