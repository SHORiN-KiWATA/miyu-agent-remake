## 检索的底子

### 是什么

一套按关键词、按向量找东西的底子，记忆（`memory.md`）、以后的知识库（`19-知识库.md`）共用：

- **关键词**：中文、日文按两个字一组切，交给 SQLite 的 FTS5，照 bm25 排。
- **向量**：每一条的 embedding 存成 SQLite 里的二进制，查的时候逐条算相似度（R-5）。
- **两路合并**：照名次合（加权的 RRF），关键词为主、向量为辅（R-5）。
- **embedding**：用途 `models.embedding`，本机的小程序 `miyu-embed` 跑 bge-small-zh-v1.5，或者走供应商的 `/v1/embeddings`；模型能换（R-5）。

库都是派生的：真相在会话日志、记忆日志、知识库的文件里，库随时能删掉照真相重建（`07-存储.md` S1、S3）。

状态：图纸（2026-10-07 起草，照 `docs/reviews/2026-10-07-记忆知识库embedding调研.md` 第八、九节项目主人的拍板）。每一节标着由哪一步做，步子见 `memory.md`「施工步子」。做完一步，这一页照做好的样子改写那几节。

做好了的：R-1 切词、检索库（第一、二条，「对外的样子」照做好的写；FTS5 那一列叫 `words`）。R-2（上）检索库多 `marks` 表（版本 2）、`apply`、`mark`、`forget`；R-2（下）`apply` 照到的位置不往回挪。R-3（上）多墓碑表 `buried`（版本 3）、`Edit::Bury`、`Unbury`、`bury`、`is_buried`。

### 在哪

| 代码 | 管什么 | 哪一步 |
|---|---|---|
| `crates/miyu-recall/src/lib.rs` | 纯逻辑的 crate（第 2 层）：对外的几样 | R-1 |
| `crates/miyu-recall/src/terms.rs` | 一段字切成存进索引的词、拼成查询（`index_terms`、`query`） | R-1 |
| `crates/miyu-recall/src/fuse.rs` | 加权的 RRF：几路名次合成一个，只有向量一路找到的过下限（第三条第 3 款） | R-5 下 |
| `crates/miyu-recall/src/vector.rs` | 向量写成字节、读回来、点积 | R-5 下 |
| `crates/miyu-store/src/recall.rs` | 一个检索库：开（坏了删掉重建）、放进一条、拿掉一条、照关键词找（R-1）；一批和照到哪一起写、拿掉一个来源（R-2 上） | R-1 |
| `crates/miyu-store/src/recall/vectors.rs` | 向量表：放、照模型列出缺的、读出来逐条算最像的（第三条第 1、2 款） | R-5 下 |
| `crates/miyu-session/src/memory/vectors.rs` | 算问句的向量（限时）、在后台补这一间缺的；照调的一方手里的配置挑本机、远程、关（第三条第 4 到 6 款，R-5 补加远程） | R-5 下 |
| `crates/miyu-core/src/embed.rs` | 核心起来时照环境拼好 `Embedder` 要的几样（第四条第 1 款） | R-5 下 |
| `crates/miyu-recall/src/embedding.rs` | 本机模型的清单：照原文读、查（第四条第 2 款；R-5 中从 `miyu-embed` 挪进来，核心不能依赖 `miyu-embed`） | R-5 上、中 |
| `crates/miyu-embed/src/manifest.rs` | 小程序读清单的文件 | R-5 上 |
| `crates/miyu-embed/src/wordpiece.rs` | BERT 的 WordPiece 分词（第四条第 5 款） | R-5 上 |
| `crates/miyu-embed/src/model.rs` | 照清单载入模型、算一句的向量：ONNX Runtime 静态链接在这个小程序里 | R-5 上 |
| `crates/miyu-embed/src/serve.rs`、`main.rs` | 小程序 `miyu-embed` 的协议和参数（第四条第 4 款） | R-5 上 |
| `crates/miyu-session/src/embed.rs` | 核心一份的 `Embedder`：备齐文件、按需拉起、一条一条问、空闲退出、连着起不来就不再拉起（第四条第 3、4 款） | R-5 中 |
| `crates/miyu-session/src/embed/fetch.rs` | 照清单核对、下载模型的文件：清单以外的删掉，对不上的删掉重下，边下边写、边算 SHA-256 | R-5 中 |
| `crates/miyu-session/src/embed/worker.rs` | 和跑着的 `miyu-embed` 说话：拉起、等 `ready`、一条一条问、编号对不上、时限、关掉 | R-5 中 |
| `crates/miyu-session/src/embed/remote.rs` | 远程的 embedding：照一家供应商发 `/embeddings`、读向量、归一化、记账、记日志（第四条第 7 款） | R-5 补 |
| `crates/miyu-session/src/route/endpoint.rs` | 照一家供应商拼地址、key、另配的头，不走路由、池、冷却：`provider.test` 和远程的 embedding 共用（从 `route/probe.rs` 抽出来） | R-5 补 |
| `crates/miyu-http/src/post.rs` | 一次 POST、整个读完：有总时限、有大小上限，出错的说法同一次 GET | R-5 补 |
| `resources/models/embed/bge-small-zh-v1.5.toml` | 出厂的清单（模型资料，不给模型看，不进登记簿） | R-5 上 |

`miyu-recall` 只用白名单里的 crate（`01-架构.md` 第九节），不碰 I/O。SQLite 的那一半放在 `miyu-store`：它已经有 `rusqlite` 和开库的规矩（`sqlite.rs`，`store/index.md`），检索库照同一套开、坏了删、版本不对删。

### 对外的样子

**切词**（`miyu_recall::terms`，R-1）：

- `index_terms(字) -> String`：存进 FTS5 的那一列，词和词之间一个空格。
- `query(字) -> Option<String>`：拼成 FTS5 的 `MATCH` 写法；一个词都切不出来的交回 `None`，不查。

**检索库**（`miyu_store::recall::RecallIndex`，R-1）：

| 方法 | 做什么 |
|---|---|
| `open(路径) -> (RecallIndex, Opened)` | 照 `sqlite::open` 开：没有就建，坏了、版本不对删掉建空的；删了也打不开的是 `Opened::Unusable`，这个库这一回找不到任何一条、写什么都不写。`Opened` 照会话索引那四种 |
| `reset()` | 用着用着读出坏了的：关上、连同 `-wal`、`-shm` 删掉，建一份空的 |
| `put(键, 字, 时刻)` | 放进一条；键已经有的整条换掉（字、时刻、词都换） |
| `remove(键)` | 拿掉一条，没有的不要紧 |
| `search(查询, 最多几条) -> Vec<Hit>` | 照关键词找，bm25 最相关的在前；`Hit { key, rank }`，`rank` 是第几名（从 0 起） |
| `keys()` | 库里有哪些键，照键排：重建时和真相比，多的拿掉 |
| `apply(来源, 几处改动, 照到)` | R-2：一批放进、拿掉，连同这个来源照到了哪个序号，在一个事务里写。这个来源照到的位置已经在这一批以后的、整个来源埋了墓碑 `来源/` 的（会话删掉了），整批不写，照到的位置不往回挪、也不记（R-2 下：后台补旧会话的一批可能落后、可能夹着删会话，不该把撤销了的、删掉的又放回去）；一样的照写 |
| `mark(来源)` | R-2：这个来源照到了哪个序号，没照过的没有 |
| `forget(来源)` | R-2：拿掉键以 `来源/` 开头的全部和它的照到哪 |
| `bury(键)`、`is_buried(键)` | R-3 上：单埋一块墓碑（不碰照到哪，删会话时用）、这个键埋了没有。`Edit` 多 `Bury { key }`、`Unbury { key }`，在 `apply` 里一起写；撤销的那一轮照它判记忆的出处活不活，删掉的会话照会话的目录判（`memory.md` 第二条第 4 款，R-3 三补） |

- 键是调的一方起的字符串（例如回合索引用 `会话编号/回合`），库不解读。
- 一个库一个文件，一个核心开一个连接、一直开着，拿锁护着（`07-存储.md` 第六节：同一个进程里开了又关同一个库文件，会丢掉 SQLite 的文件锁）。

**库的结构**（`user_version` 1，R-1）：

```sql
CREATE TABLE items (
  id   INTEGER PRIMARY KEY,
  key  TEXT NOT NULL UNIQUE,
  text TEXT NOT NULL,
  at   INTEGER NOT NULL
);
CREATE VIRTUAL TABLE terms USING fts5(words, content='', contentless_delete=1, tokenize='unicode61');
```

- `terms` 的 rowid 就是 `items.id`，词在 `words` 那一列（列名不能和表同名：FTS5 有一列和表同名的隐藏列）。`contentless`：词只进倒排索引，不另存一份原文；原文在 `items.text`。
- `at` 是毫秒，给以后的排名用（越老越靠后，`memory.md` 第八条）；R-1 只存不用。
- R-2 加一张 `marks(source TEXT PRIMARY KEY, upto INTEGER NOT NULL)`：每个来源（例如一个会话）照到了哪个序号，版本加一成 2。
- R-3 上加一张 `buried(key TEXT PRIMARY KEY)`：墓碑，版本加一成 3。
- 向量表随 R-5 加，版本跟着加一：派生的，删掉重建，不写迁移。

### 怎么走

**一、切词**（R-1）

1. 先把字照 Unicode 分成一段段：**连着的汉字、平假名、片假名**算一段（`CJK` 段），别的照原样。
2. `CJK` 段：每两个挨着的字是一个词（重叠的：`记忆系统` → `记忆`、`忆系`、`系统`），**每个字也单独是一个词**。只有一个字的段只出它自己。
   - 为什么存单字：她只问一个字的（「猫」）也要搜得到；bm25 的 IDF 让常见的单字分低，不会喧宾夺主。
   - 为什么两两切、不用词典（jieba）：中日文都行，三个平台一样，没有词典要带、要更新；新词、人名、网络语也切得出（调研第三节）。
3. 别的段照原样交给 FTS5 的 `unicode61`：它照空白、标点切英文和数字，大小写不分。
4. `index_terms`：几段切出来的词照原来的先后用一个空格连起来。
5. `query`：
   - 照第 1、2 条切这一句。`CJK` 段长度不小于 2 的只出两两的词（单字在长的段里只是噪声）；只有一个字的段出那个字。别的段照 `unicode61` 的规矩切成词（字母、数字连着的算一个，转成小写）。
   - 去重，照先后留前 64 个：一句话再长，查询也有个上限。
   - 每个词包上双引号，用 ` OR ` 连起来：几个词里中了越多、越少见的，bm25 越靠前。双引号也让 `OR`、`NOT`、`NEAR` 只当普通的词；词里只有字母、数字、汉字假名，不会有双引号，不用转义。
   - 一个词都没有的（只有标点、空白）交回 `None`。

**二、检索库**（R-1）

1. **开**：照 `store/index.md` 第一条：版本 0 建表，1 跑 `quick_check`，别的删掉重建；WAL、`synchronous` `NORMAL`。
2. **放进一条**（`put`）：一个事务里，键有旧的先从 `terms` 删掉旧的那一行、再换 `items` 那一行，然后照 `index_terms(字)` 写进 `terms` 的 `words`。
3. **拿掉一条**：一个事务里从 `terms`、`items` 都删。
4. **找**：`query` 是 `None` 的交回空的。否则：

   ```sql
   SELECT items.key FROM terms JOIN items ON items.id = terms.rowid
   WHERE terms MATCH ?1 ORDER BY bm25(terms) LIMIT ?2
   ```

   bm25 越小越相关，照它排；名次从 0 数。`MATCH` 的写法是 `query` 拼好的，不会是坏的；SQLite 照样报错的，如实交回 `DbError`，不吞。
5. 读写出错交回 `DbError`，调的一方照派生数据的规矩处理：记一行 `WARN`，这一次不用索引，下次照真相补（`store/index.md` 第二条第 4 款）。

**三、向量和两路合并**（R-5 下）

1. **向量表**：检索库多一张 `vectors(key, model, data)`，主键是（条目、模型），`data` 是 f32 的小端字节（`miyu_recall::vector`），长度是维数的 4 倍；`model` 是模型的编号：本机的是 `local:<id>`（`local:bge-small-zh-v1.5`），远程的是 `<供应商>/<模型>`（R-5 补）。条目拿掉、整个来源拿掉、换了字（同样的字照留）的，它的向量跟着拿掉；放向量时条目已经没了的不放。不换版本：每次开库、重建以后照 `CREATE TABLE IF NOT EXISTS` 补上这张表，以前的库也就有了（换版本要删掉重建，删掉的会话那几块墓碑是删会话时写的，重建补不回来）。换了模型的照新模型补，旧的留着不碍事。
2. **查**：向量都归一化过，相似度就是点积。读出这个模型的全部向量逐条算（旧版的规模约 1 万条，几毫秒），取最像的几条，带着条目的字和时刻；慢了先量，再看要不要另想办法。
3. **两路合并**（`miyu_recall::fuse`）：照名次合，不照分数（bm25 和余弦的尺度对不上）：`分 = Σ 权重 / (60 + 名次 + 1)`，名次从 0 数；关键词一路权重 1，向量一路 0.5；只有向量一路找到的，相似度不到 0.40 的不要；分一样的照关键词一路的先后、再照向量一路的。0.40 是 2026-10-09 在 bge-small-zh-v1.5 上量的起点：八句和记忆一个字都不重合的问题（「我家宠物叫什么」对「用户养了一只猫」这种），对的那一条都排第一，相似度最低 0.436、平均 0.564；不对的平均 0.297、九成在 0.368 以下、最高 0.456。几个数测评集有了再定。
4. **搜**（`memory_search`、`memory.search`）：先算问句的向量，最多等 1 秒（第一次拉起小程序要一两百毫秒；那一条照样在后台算完，不打断小程序的一问一答）；算出来了，记忆库、回合库各照关键词一路、向量一路取（各取四倍再挑），照第 3 款合并，再照原来的规矩挑。算不出（还在备、用不了、过了时限）的只走关键词，和原来一样，记一行 `DEBUG query not embedded`，不停下。
5. **补**：搜的时候起。照这一间的记忆库、回合库各起一个后台的，一次取 16 条还没有这个模型的向量的（照行号往后取），一条一条算、写回；模型还在备的每秒再问一次，等它备好；这一条算不出的跳过、接着补别的（下次搜再试），连着 3 条算不出的这一回不补了（远程那一家挂了、key 错了，不一条一条地等，R-5 补）；用不了的（下不成、起不来过三次、远程那一家没配）这一回不补了。同一份库同一时刻只有一个在补。补了几条记一行 `INFO memory vectors filled index=… count=… took_ms=…`，读写不了记 `WARN memory vectors not filled error=…`。之后才放进库的（这一轮自己的回合）等下一次搜。
6. 照哪一路（R-5 补）：照调的一方手里的配置的 `models.embedding` 挑，她的工具照这一轮冻结的配置（派出去那一刻交给端口），协议照这时的配置。不写、写 `local` 的照本机的（第四条第 3、4 款），写 `<供应商>/<模型>` 的照那一家（第四条第 7 款），`off` 的不算问句、不补、不下模型、不拉起小程序，只照关键词找。
7. 向量照（模型、字的 SHA-256）缓存在系统的缓存目录、库删了重建不重算：还没做。重建很少见，重算一万条约二十秒，后台慢慢补；量出来是问题再加。

**四、embedding**（R-5 上、中、下）

1. 用途 `models.embedding`（R-5 下，`models.md` 的 `[models]`；类型是 `model_or`，`config.md`「配置项的类型」）：写 `local` 用本机的 `miyu-embed`；写 `off` 不下模型、不拉起、只照关键词找（2026-10-09 项目主人定加 `off`）；写 `<供应商>/<模型>` 的走那一家 OpenAI 兼容的 `/v1/embeddings`（第 7 款，R-5 补，方向 2026-10-07 定）；不写的照 `local`，本机的小程序、缓存目录、清单哪一样没有，就只有关键词。下一个回合开始时生效。不收 `@池`：向量要和存下的同一个模型比，池里换了成员就对不上。设置页上这一项叫「语义模型」，`local` 叫「内置模型」，后面暗字写模型名（`config.schema` 选项的 `note`），指定一家的模型从 `model.list` 里 `embedding: true` 的挑（R-5 再补，2026-10-09 项目主人定）。核心起来时照环境拼好交给 `Embedder`（`miyu-core/src/embed.rs`），连同模型资料交给协议端点（`Core::with_vectors`，远程的照它查供应商、记账）：小程序在主程序真实位置的旁边（照软件包的找法），清单是资源目录的 `models/embed/bge-small-zh-v1.5.toml`，模型放在缓存目录的 `embed/` 下，下载照环境变量走代理。
2. **清单**（R-5 上）：本机模型照一份 TOML 认，出厂的在 `resources/models/embed/bge-small-zh-v1.5.toml`，配置 `embedding.local` 换成别的清单就换了模型：做成可更换的（2026-10-07 项目主人定）。几格：
   - `id`：模型的名字，向量的模型编号写成 `local:<id>`；`dims`：几维；`pooling`：怎么取一句的向量，现在只认 `cls`（取 `[CLS]` 那一格）；`max_tokens`：一句最多几个词（连 `[CLS]`、`[SEP]`），至少 2。
   - `[[files]]`：每个文件的 `role`（`model`、`vocab` 各正好一个）、`name`（一个单纯的文件名，不带目录）、`url`、`sha256`、`size`。
   - 少格、多格、不认识的取法、`role` 重了或缺了、`dims` 是 0、`id` 或文件名带目录的都拒，说哪里不对。向量一律归一化。WordPiece 以外的分词、`cls` 以外的取法，换到那样的模型时再加。
   - 出厂的是 bge-small-zh-v1.5 的 8 位量化 ONNX（`model_quantized.onnx`，24 MB，512 维，MIT）：十句和原版 fp32 的余弦平均 0.991，`model_int8.onnx` 只有 0.970，一样快（2026-10-09 量）。
3. **下载**（R-5 中）：模型文件第一次用时下载到系统的缓存目录，地址出厂指到 Miyu 自己的 GitHub Release（项目主人定；Release `models-bge-small-zh-v1.5`，预发布，2026-10-09 建，放模型、`vocab.txt`、FlagEmbedding 的 MIT 许可证、`SHA256SUMS`），换清单就换了地址：
   - 放在缓存目录（`store.md` 第 3 条）的 `embed/<id>/<文件名>`；`id` 和文件名都是单纯的名字（清单查过）。
   - 第一次要向量时在后台备：目录里清单以外的删掉（崩了留下的临时文件、换下来的旧文件）；已经有的照大小、SHA-256 核对，对不上的删掉（`WARN embedding model mismatch file=…`）；没有的下。备着的时候交回「还在备」，要的一方照只有关键词走，不等。
   - 下：走 `miyu_http::fetcher`，代理照环境变量；一个文件最多 10 分钟；边下边写旁边的临时文件（`generated::Staged`）、边算 SHA-256，超过清单的 `size` 当场停；SHA-256 对得上才改名成正式的，对不上的不留（少了的 SHA-256 一样对不上，不另比大小）。一个不成就停，记一行 `WARN embedding model not downloaded model=… error=…`，这一回 1 小时内不再试（照模型目录的 `RETRY`），之间要的交回用不了。下成一个记一行 `INFO embedding model downloaded model=… file=… bytes=… took_ms=…`。
   - 2026-10-09 照 Release 真下一次（`crates/miyu-session/tests/embed.rs` 的量尺）：下 24 MB、核对、第一次拉起、算一条合计 2.3 秒，之后一条 2.1 毫秒。
4. **`miyu-embed`**：核心这边（R-5 中，`Embedder`）：
   - 在主程序真实位置的旁边找（照沙盒助手的找法）；没有的、缓存目录算不出的、清单读不了的，造的时候记一行 `WARN embedder unavailable reason=…`，以后每一条都交回用不了。
   - 要用时拉起，等 `ready` 最多 30 秒；一次一条，同时来的排队；一条最多等 10 秒。它回一句错的，这一条算不出、它接着用；它退出了、回的编号对不上、读不懂、过了时限的，当它坏了：关掉，这一条算不出（`WARN embedder failed error=…`、`INFO embedder stopped reason=failed`），下一条再拉起。起来了记一行 `INFO embedder started model=… took_ms=…`。
   - 这一回起不来过三次，就不再拉起，以后每一条都交回用不了（核心重起来再数）。
   - 600 秒没有新的请求就关它的标准输入、等它退出（最多 5 秒，没退的杀掉），记一行 `INFO embedder stopped reason=idle`；下一条再拉起。
   - 它跟着核心走：核心放下它就杀掉（`kill_on_drop`）；核心整个没了，Windows 上作业对象收掉它，Unix 上它读到标准输入关了自己退出。它不算核心「忙」：核心空闲退出照旧。
   - 交回的三种：还在备、用不了（这一阵）、这一条算不出；原话是英文短句，进运行日志。

   小程序本身（R-5 上）：
   - `miyu-embed --manifest <清单> --dir <模型文件的目录>`：照 `role` 在目录里找文件，下载、核对是核心的事。
   - 载入成了，标准输出印一行 `{"ready":{"model":"local:<id>","dims":<维数>}}`；清单读不懂、文件没有、模型载入不了，印一行 `{"error":"…"}`，退出码 1；参数写错，标准错误印用法，退出码 2。
   - 之后标准输入一行 `{"id":"…","text":"…"}`，标准输出回一行 `{"id":"…","vector":[…]}`（f32，JSON 的最短写法，读回 f32 一位不差）；读不懂的、不是 UTF-8 的、算不出的回 `{"id":"…","error":"…"}`（读不出 `id` 的写 `null`），接着读下一行；行尾的 `\r` 不算。标准输入关了退出码 0；读不了标准输入、写不进标准输出的，原因写到标准错误，退出码 1。
   - 一次一条、单线程（ONNX Runtime 的 `intra`、`inter` 都是 1；旧版实测一次 32 条、16 线程时内存冲到 2 GB 不还）。错误的原话是英文短句；协议只有核心读，不给模型看，不进登记簿。
   - ONNX Runtime 静态链接在它里面（`ort` 钉死 `=2.0.0-rc.13`，ONNX Runtime 1.28，编译时下 pyke 预编好的静态库），主程序不带。
5. **分词**（R-5 上）：BERT 的 WordPiece，照 bge-small-zh-v1.5 原版的配置，和 Hugging Face `tokenizers` 的结果一样（`crates/miyu-embed/tests/tokens.rs` 逐个比）：
   - 去掉 `U+0000`、`U+FFFD` 和 Unicode「其他」类的字（控制、格式、代理、私用、没分配；制表、换行、回车算空白）；空白处断开；汉字（CJK 统一表意文字那几段）、标点（ASCII 的标点和 Unicode 的七种标点类）一个一个成词。不转小写、不去重音（原版的配置）。
   - 一个词超过 100 个字整个算 `[UNK]`；不然从头起每次取最长的、在词表里的一段（不是开头的带 `##`），有一段取不出来整个词算 `[UNK]`。
   - 前后加 `[CLS]`、`[SEP]`；超过 `max_tokens` 的截掉后面的，留住 `[SEP]`。词表一行一个词，行号是编号，没有 `[CLS]`、`[SEP]`、`[UNK]` 的、一个词写了两行的拒。
   - 和 `tokenizers` 不一样的两处：字里写着 `[CLS]` 这种的照普通的字切（人说的话不变成控制用的词）；Unicode 的类别照新的表，Unicode 9 以后才有的字切成 `[UNK]`（它的表旧，当没分配去掉）。
   - 原版不转小写：大写的英文词（`RTX`、`Hello`）、片假名的词都是 `[UNK]`，那一部分靠关键词那一路（第一条的两两切词管得到日文）；改不改等测评集。
6. **实测**（本机的，2026-10-09，Linux x86_64，release 没 strip）：程序 29 MB，没有 ONNX Runtime 的动态库；起来到 `ready` 130 到 580 毫秒，一句短的（十来个字）4 到 9 毫秒、一百来个字 12 到 25 毫秒（同一台机器上别的会话在编译，数跳得厉害；仓库外单独试的是载入 46 毫秒、一句 1 到 2 毫秒），峰值内存 62 MB。

7. **远程**（R-5 补，`embed/remote.rs`；怎么配、怎么取端点、怎么记账 2026-10-09 核心的主会话定）：`models.embedding` 写 `<供应商>/<模型>` 的，一次算一句：
   - 端点：照调的一方手里的配置找那一家，拼地址、key、另配的头（`route/endpoint.rs`，和 `provider.test` 同一段），不走路由、池、冷却；认证头照这一家的驱动写，另配的头照种子 `embedding` 换。客户端照地址挑：落在本机的不走代理，别的照环境变量（`ModelData::fetcher_for`）。这一家没配、地址或 key 取不到、没有客户端的算「用不了」。
   - 发：`POST <地址>/embeddings`，`{"model":"<模型>","input":"<字>"}`，一次最多等 10 秒（问句照旧最多等 1 秒，第三条第 4 款），回应最多 4 MiB。读 `data[0].embedding`、归一化（各家不一定归一化过）。发不出去、回的不是 2xx、超时、读不懂、没有向量、长度是零的算「这一条算不出」。一次一句：补的时候一条一条发，量出来慢了再想批量。
   - 记账：报了 `usage.prompt_tokens` 的照一次性调用记一笔 `usage.oneshot`（用途 `embedding`、供应商、模型、输入 token，有价格的照这个模型的价格算金额，`models.md`「怎么走」第九条第 4 条），记在调的那个账号名下（她的工具是会话的属主，协议是连上来的管理员）；回的向量用不了的（全是零、没有）也记，那一家照样收了钱；没报的、出错的、读不懂的不记。问句等不到的那一条照样在后台算完、照样记。
   - 日志：每一次记一行，目标 `miyu::session`，和一次性入口那一行同一个写法：成了 `INFO model call purpose=embedding provider=… model=… input=… took_ms=…`，没成 `INFO model call failed purpose=embedding provider=… model=… took_ms=… error=…`；原话不带地址、key。

### 出错

- 库读写出错：`DbError`，照第二条第 5 款。
- 切不出词：`query` 交回 `None`，不算出错。
- 本机 embedding 用不了、算不出：照第四条第 3、4 款交回三种之一、记日志（目标 `miyu::session`），不停下，要的一方照只有关键词走（第三条第 3 款）。
- 远程的用不了、算不出：照第四条第 7 款交回两种之一、记 `model call failed`，同上照只有关键词走。

### 守着它的

R-5 补做好的：

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-session/tests/memory_remote.rs` | 第三条第 5、6 款，第四条第 7 款，假服务器当那一家：写了 `<供应商>/<模型>` 的照它算问句、补向量，一个字都不重合的照意思找得到；请求是 `POST /v1/embeddings`，带着认证、模型名、那一句；向量照 `<供应商>/<模型>` 存、归一化过；请求带着档案另配的头（种子是 `embedding`）；报了用量的每一次记一笔 `usage.oneshot`（用途 `embedding`，记在属主名下，读的是 `prompt_tokens`），没报的不记；问句等不到的等 1 秒照关键词找；补的时候出错的跳过，算成一条的重新数，读不懂、全是零、没有向量连着三条，这一回不补了、后面的不发；出错的、读不懂的不记账，全是零的报了用量照记 |
| `crates/miyu-endpoint/tests/memory_meaning.rs` | 第四条第 7 款：协议照这时的配置发给远程那一家，补齐以后照意思找得到，用量记在管理员名下 |
| `crates/miyu-http/tests/post.rs` | 第四条第 7 款的发：带着头和 JSON 的请求体、交回整个响应体；回的不是 2xx 的带状态码和响应体；超过上限的、超时的出错；连不上的原话不带地址、key |

R-5 下做好的：

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-recall/src/vector/tests.rs`、`fuse/tests.rs` | 第三条第 1、3 款：写成字节读回一位不差、小端、长度不对的读不出、点积、维数对不上算 0；只有一路的照它的先后、两路都有的在前、只有向量一路找到的过下限、关键词也找到的不看下限、名次照原来的先后数、分一样的照先来后到 |
| `crates/miyu-store/tests/recall_vectors.rs` | 第三条第 1、2 款：缺的照模型列、从哪一行往后列、最多几条；最像的在前、带着字和时刻、别的模型的不算；条目拿掉、换了字、整个来源拿掉、一批里拿掉的，向量跟着没；条目没了的不放；以前的库（没有这张表）照样开、不重建、补上表 |
| `crates/miyu-session/tests/memory_vectors.rs` | 第三条第 4 到 6 款，真的 `miyu-embed`、手造的小模型：一条和问句一个字都不重合的记忆，第一次只走关键词找不到，后台补上向量以后照意思找得到；以前的对话也一样；`off` 的不补、不拉起小程序，关键词照旧找得到 |
| `crates/miyu-endpoint/tests/memory_meaning.rs` | 第三条第 4 款：真核心接上本机 embedding，`memory.search` 第一次只走关键词，补齐以后照意思找得到 |

R-5 中做好的：

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-session/tests/embed.rs` | 第四条第 3、4 款，假服务器给手造的小模型、真的 `miyu-embed`、缓存目录在临时目录里：第一次交回「还在备」、下完算得出、和小程序直接算的一样、不留临时文件；有了的、核对过的不再下，清单以外的删掉；对不上的只重下那一个；下坏了的（大小不对、超过大小、404）不留、一小时内不再试；没有小程序、没有缓存目录、清单读不了的用不了、不下；两个一起要的都拿到；闲了退出、下一条再拉起；起不来过三次以后不再拉起。量尺 `measure_the_real_model`（`#[ignore]`，联网照 Release 真下一次） |
| `crates/miyu-session/src/embed/worker/tests.rs` | 和小程序说话，对面是内存里的管道：`ready` 报模型编号、报错、退出了、过了时限；回的照编号收；回一句错的照收、接着问；编号对不上、读不懂、没有向量、退出了、不回话的当它坏了 |
| `crates/miyu-recall/src/embedding/tests.rs` | 第四条第 2 款：清单的原文怎么读、怎么查（R-5 中从 `miyu-embed` 挪过来，多一条：`id` 带目录的拒） |

R-5 上做好的：

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-embed/tests/tokens.rs` | 第四条第 5 款：真词表上 39 句照 `tokenizers` 0.23.2 的结果逐个比（空的、空白、汉字、全角标点、大写英文、带重音的、片假名、韩文、emoji、零宽空格、BOM、控制字符、`U+FFFD`、不间断空格、100 和 101 个字的词、扩展区和兼容区的汉字、全角字母、`##` 开头的）；截断留住 `[SEP]`；字里的 `[CLS]` 照普通的字；词表少了特殊的词、写了两行的拒；`\r\n` 的词表也认 |
| `crates/miyu-embed/tests/manifest.rs` | 第四条第 2 款：出厂的清单读得出、每一格对；文件读不了说是哪个（原文的读法 R-5 中挪进 `miyu-recall`） |
| `crates/miyu-embed/tests/protocol.rs` | 第四条第 4 款，手造的小模型（`fixtures/tiny/`，`fixtures/make.py` 造）：`ready`；向量和 Python 的 ONNX Runtime 算的每格差不过 1e-6；超长截断、空字也有向量；不是 JSON、没有 `id`、`id` 不是字、没有 `text`、空行、不是 UTF-8 的回错，接着读下一行；行尾的 `\r`；标准输入关了退出码 0；清单读不了、文件不在、模型是坏的退出码 1；清单的 `dims` 和模型对不上的每一句回错；参数写错（少了、多了、写了两次）退出码 2、印用法 |
| `crates/miyu-embed/tests/real.rs` | 真模型（`#[ignore]`，`MIYU_EMBED_MODEL_DIR` 指到 Release 的文件）：四句的向量和 Python 的 ONNX Runtime 算的余弦不低于 0.999 |

R-1 做好的：

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-recall/src/terms/tests.rs` | 第一条：两两切加单字、只有一个字的段、汉字假名连成一段、片假名的中点和标点断开一段、英文数字照 `unicode61`、查询只出两两的、英文转小写、去重、64 个上限、FTS5 的关键字照普通的词、全是标点的是 `None` |
| `crates/miyu-store/tests/recall.rs` | 第二条（R-3 上加墓碑：一批里埋、揭，埋两次、揭没埋的不碍事；出处活不活：撤销的那一轮、删掉的会话、别的人格的库里没埋；R-3 三补：会话照目录判，回合库删掉重建以后进了回收处的照样死的、恢复的又活了、别的账号名下的会话也认得出）：建、重开、坏了删掉重建、版本不对重建；放进、换掉、拿掉以后搜不到；两个字的词搜得到（trigram 搜不到的那种）、一个字搜得到；中了越多的越靠前，名次从 0 数；中英混着的；只给几条、切不出词的找不到；数据根在临时目录。量尺 `measure_ten_thousand_sentences`（`#[ignore]`） |

### 出处

- Release `models-bge-small-zh-v1.5` 的文件来自 Hugging Face 的 `Xenova/bge-small-zh-v1.5`（commit `75c43b06`，`BAAI/bge-small-zh-v1.5` commit `7999e1d3` 转的 ONNX），原样转发；`crates/miyu-embed/tests/fixtures/` 的标准答案照那里的 `make.py` 生成（`docs/construction/R-5-本机embedding的小程序（上）.md`）。
- `docs/reviews/2026-10-07-记忆知识库embedding调研.md` 第三节（选型和实测）、第五节末尾（技术细节照推荐定）、第八节（项目主人的拍板：本机 embedding 用 bge-small-zh、模型从 Miyu 的 GitHub Release 下、做成可更换的）。
- `17-记忆.md` 第四节、`19-知识库.md` N3：关键词加向量、两路合并、索引在 SQLite 里。
- `10-自带软件.md`、`tools/history.md`：混合召回以关键词为主、向量为辅（2026-09-29 项目主人定）。

### 还没有的

- 换清单的配置项（`embedding.local`，第四条第 2 款）：还没排。
- 远程的一次发几句（批量）：现在一次一句，量出来慢了再说。
- 第三条第 7 款，照字的哈希缓存向量：量出来是问题再加。
- 知识库的切块、文件监视：知识库那条线。
- `history` 改用全文索引：照 `tools/history.md`，慢了再做。
