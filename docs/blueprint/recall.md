## 检索的底子

### 是什么

一套按关键词、按向量找东西的底子，记忆（`memory.md`）、以后的知识库（`19-知识库.md`）共用：

- **关键词**：中文、日文按两个字一组切，交给 SQLite 的 FTS5，照 bm25 排。
- **向量**：每一条的 embedding 存成 SQLite 里的二进制，查的时候逐条算相似度（R-5）。
- **两路合并**：照名次合（加权的 RRF），关键词为主、向量为辅（R-5）。
- **embedding**：用途 `models.embedding`，本机的小程序 `miyu-embed` 跑 bge-small-zh-v1.5，或者走供应商的 `/v1/embeddings`；模型能换（R-5）。

库都是派生的：真相在会话日志、记忆日志、知识库的文件里，库随时能删掉照真相重建（`07-存储.md` S1、S3）。

状态：图纸（2026-10-07 起草，照 `docs/reviews/2026-10-07-记忆知识库embedding调研.md` 第八、九节项目主人的拍板）。每一节标着由哪一步做，步子见 `memory.md`「施工步子」。做完一步，这一页照做好的样子改写那几节。

做好了的：R-1 切词、检索库（第一、二条，「对外的样子」照做好的写；FTS5 那一列叫 `words`）。

### 在哪

| 代码 | 管什么 | 哪一步 |
|---|---|---|
| `crates/miyu-recall/src/lib.rs` | 纯逻辑的 crate（第 2 层）：对外的几样 | R-1 |
| `crates/miyu-recall/src/terms.rs` | 一段字切成存进索引的词、拼成查询（`index_terms`、`query`） | R-1 |
| `crates/miyu-recall/src/fuse.rs` | 加权的 RRF：几路名次合成一个 | R-5 |
| `crates/miyu-recall/src/vector.rs` | 向量写成字节、读回来、点积 | R-5 |
| `crates/miyu-store/src/recall.rs` | 一个检索库：开（坏了删掉重建）、放进一条、拿掉一条、照关键词找 | R-1 |
| `crates/miyu-store/src/recall/vectors.rs` | 向量表：放、照模型读出来逐条算 | R-5 |
| `crates/miyu-embed/` | 本机 embedding 的小程序：ONNX Runtime 静态链接在里面 | R-5 |

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

**三、向量和两路合并**（R-5，施工时细化）

1. 每一条另存一份向量：`vectors(id, model, dims, data BLOB)`，`data` 是 f32 的小端字节，长度是维数的 4 倍；`model` 是模型的编号（例如 `local:bge-small-zh-v1.5`），换了模型的旧向量不用，后台照新模型慢慢补。
2. 向量都先归一化，相似度就是点积。查的时候读出这个模型的全部向量逐条算（旧版的规模约 1 万条，几毫秒），取最前面的几条；慢了先量，再看要不要另想办法。
3. 两路合并：照名次 `score = Σ 权重 / (60 + 名次 + 1)`，关键词一路权重 1，向量一路 0.5（起点，测评后定）；只有向量命中的那几条，相似度还要过一个下限。向量那一路用不了（没装、出错、超时）就只有关键词，记一行日志，不停下。
4. 向量照（模型、字的 SHA-256）缓存在系统的缓存目录：库删了重建、换了位置都不重算。

**四、embedding**（R-5，施工时细化）

1. 用途 `models.embedding`：写 `local` 用本机的 `miyu-embed`；写 `<供应商>/<模型>` 走那一家 OpenAI 兼容的 `/v1/embeddings`；没写的，`miyu-embed` 在就用本机的，不在就只有关键词。
2. 本机模型照一份清单认：名字、下载地址、每个文件的 SHA-256、维数、怎么取向量（bge 是第一个 token、归一化）、最长多少 token。出厂一份 `bge-small-zh-v1.5` 的 int8（24 MB，MIT，2026-10-07 项目主人定），配置 `embedding.local` 换成别的清单就换了模型：做成可更换的（同一天项目主人定）。
3. 模型文件第一次用时下载到系统的缓存目录，地址出厂指到 Miyu 自己的 GitHub Release（项目主人定），配置能改；SHA-256 对不上的不用、删掉。
4. `miyu-embed`：核心按需拉起，空闲 600 秒退出；一次一条、单线程（旧版实测：一次 32 条、16 线程时内存冲到 2 GB 不还）；标准输入输出上一行一个 JSON。ONNX Runtime 静态链接在它里面，主程序不带。

### 出错

- 库读写出错：`DbError`，照第二条第 5 款。
- 切不出词：`query` 交回 `None`，不算出错。

### 守着它的

R-1 做好的：

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-recall/src/terms/tests.rs` | 第一条：两两切加单字、只有一个字的段、汉字假名连成一段、片假名的中点和标点断开一段、英文数字照 `unicode61`、查询只出两两的、英文转小写、去重、64 个上限、FTS5 的关键字照普通的词、全是标点的是 `None` |
| `crates/miyu-store/tests/recall.rs` | 第二条：建、重开、坏了删掉重建、版本不对重建；放进、换掉、拿掉以后搜不到；两个字的词搜得到（trigram 搜不到的那种）、一个字搜得到；中了越多的越靠前，名次从 0 数；中英混着的；只给几条、切不出词的找不到；数据根在临时目录。量尺 `measure_ten_thousand_sentences`（`#[ignore]`） |

### 出处

- `docs/reviews/2026-10-07-记忆知识库embedding调研.md` 第三节（选型和实测）、第五节末尾（技术细节照推荐定）、第八节（项目主人的拍板：本机 embedding 用 bge-small-zh、模型从 Miyu 的 GitHub Release 下、做成可更换的）。
- `17-记忆.md` 第四节、`19-知识库.md` N3：关键词加向量、两路合并、索引在 SQLite 里。
- `10-自带软件.md`、`tools/history.md`：混合召回以关键词为主、向量为辅（2026-09-29 项目主人定）。

### 还没有的

- 第三、四条（向量、两路合并、embedding）：R-5。
- 知识库的切块、文件监视：知识库那条线。
- `history` 改用全文索引：照 `tools/history.md`，慢了再做。
