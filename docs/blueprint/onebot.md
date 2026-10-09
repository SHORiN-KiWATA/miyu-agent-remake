## 通讯平台的桥 `miyu-onebot`

### 是什么

软件包 `miyu-onebot`：经 OneBot v11 接 QQ 的桥，和终端界面、网页平级的一个头（`docs/designs/18-通讯平台.md` 第三节、Q17）。它把 QQ 上的人接进场所会话，把她的回复发回 QQ；要不要开口、限流、出站这些和平台无关的部分在群聊内核 `miyu-chat`（`chat.md`），这里只管 QQ 这一头和跟核心的那一头。

状态：图纸，随施工 O-8 起草（2026-10-07）。O-8 只有骨架：主人的私聊、只有文字（第一条）；O-8 补照 `chat.md` 第七条第 1 条改了编号的拼法（第一条第 7、8 条）；WebUI（第二条）随 O-16、O-17 起草，O-16 做了骨架和「连接」页，O-17 做了「主人与自己人」页。O-18 改成由核心拉起：经标准输入输出说协议，`miyu onebot start/stop/restart/status/logs`（第一条，2026-10-08）。O-19 接上斜杠命令：主人的私聊里 `/` 开头的先交核心的 `command.run`（第一条「斜杠命令」，2026-10-08）。O-20 改成用核心交的配置：握手回应的 `config`、推送 `extension.config`，桥不再自己读系统配置和密钥文件，`onebot.*` 四项挪进清单的 `[settings]`（第一条，2026-10-09）。O-21 读场所规则和出厂数据：出厂的、系统的规则文件、出厂参数、违规词表照群聊内核读好、套到场所上，系统的改了下一次用就照新的；`miyu onebot venue show` 印一个场所每一项的值和来处（第一条「场所规则和出厂数据」，2026-10-09）。O-22 把群消息记进场所会话：一律旁听、不开回合，名字、@、引用、带的东西记进场所的格，群里的斜杠命令照私聊的办法交，撤回记 `venue.recalled`，私聊也带上场所的格（第一条「群消息」「撤回」，2026-10-09）。O-23（上）群里叫她就回：订阅群会话、从日志投影、过进站链、判走哪条路，主人冲她来的开一轮，她的回话照纯文本拆段发回群里、记 `venue.delivered`，每判一条记 `ext.onebot.chat.decided`；要问判官的两条路只记判断（第一条「群里怎么叫她」，2026-10-09）。O-23（下）接上判官：线路规程四种分开走，同一个人补发的照群聊内核顶替，要问判官的经核心的 `model.call` 问、读回答算分，`ext.onebot.chat.decided` 记全（第一条「群里怎么叫她」第 10 到 14 条，2026-10-09）。O-23（补）判官带人格：群会话用的人格照订阅回应记下，原文经 `persona.read` 读、记一阵，夹进判官的请求，场所规则写了 `judge = { persona = false }` 的群不带（「群里怎么叫她」第 1 条、第 12 条第 3 款，2026-10-09 项目主人定）。O-25（上）接上出站链：她的每一条话（群、私聊）先过群聊内核的出站链（清理、去重、引用和 @），过了的照纯文本拆段、第一段带上引用和 @，丢了的记一行运行日志；群里的命令回执发出 3 秒后撤回（第一条「群里怎么叫她」第 9 条、「怎么走」第 10 条、「斜杠命令」第 7 条，2026-10-09）。O-25（中）接上出站队列：她要说出去的一切（群里她的话、限流的提示、命令回执，私聊的回话）先记 `ext.onebot.venues.queued` 再交 NapCat，失败、过期的记 `ext.onebot.venues.failed`；她被禁言（`group_ban` 通知）记 `ext.onebot.venues.muted`、`unmuted`，禁言时这个群的出站排着、进站链照它算；没连着的排着、连上了发；排着的过了 `queue_expire_seconds` 作废；去重照入队算（第一条「出站队列」，2026-10-09）。O-25（下）接上退信和贴表情：她的话没发出去的，经核心的 `session.note` 给那个会话记一块 `undelivered` 事实，她下一步看到；群里判过要回、主触发是冲她来或续聊的，在她要回的那一条上贴表情，她回了第一段、那一轮完了或者过了 `reaction_seconds` 摘掉（第一条「退信」「贴表情」，2026-10-09）。下载图片和文件、WebUI 的其余几页随后面的步子。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-onebot/`（第 5 层，头） | 桥 |
| `crates/miyu-onebot/src/main.rs` | 程序的入口：先找资源目录、读给人看的字（照系统的语言），再认子命令。`serve`（只由核心拉起，O-18）：装运行日志、读 `bridge.json` 和清单里两个端口的默认值（O-20），标准输入输出交给 `serve.rs`，握手回了语言就照它说（O-20）；`start`、`stop`、`restart`、`status`（O-18）交给 `control.rs`；`logs [-f]`（O-18）交给 `logs.rs`；`web [--print]`（O-16）：WebUI 的端口照状态文件的 `web`，没有状态文件的照清单的默认值（O-20），交给 `open.rs`；`venue show <场所>`（O-21）交给 `venue.rs`；`-h`、`--help` 印用法（O-18）。`serve` 握手以前还读出厂的场所规则、出厂参数、违规词表（`rules.rs` 的 `Factory`，O-21） |
| `resources/packages/onebot.toml`（O-18） | 软件包清单：`process` 包，子命令 `onebot`、程序 `miyu-onebot`，`[process] args = ["serve"]`、`start = "manual"`；`[settings]` 四项（O-20）；`capabilities` O-23 多 `events.write`（「软件包清单」） |
| `crates/miyu-onebot/src/settings.rs` | 桥用的配置（O-20）：握手交来的 `config`、推送来的 `extension.config` 照键读成两个端口、令牌（`Settings`），没有的、`null` 的端口照清单 `[settings]` 的默认值（`Defaults`，照资源目录里的清单读），令牌没有就是没有；`onebot.trusted` 读成自己人的平台身份（`trusted`，O-23：跟核心的那一头照它认自己人） |
| `crates/miyu-onebot/src/tuning.rs` | 读 `bridge.json`：桥自己的数（O-16 多 `web` 一格；O-23 下多判官的全局并发、排队等多久两格；O-23 补多判官带的人格原文记多久；O-25 上多命令回执几秒后撤回一格；O-25 中多出站排着的多久过期一格；O-25 下多贴的表情、多久摘两格） |
| `crates/miyu-onebot/src/serve.rs` | 起来：经核心亲手给的管道（标准输入输出，O-18）握手、开两个监听（NapCat 的、WebUI 的），把几样接起来；推来的配置交给 `current.rs`，端口变了照 `/apply` 的办法当场换（O-20）；`/apply` 开好的新监听换掉旧的（O-16 补二）；状态文件跟着写（O-18）；核心关了管道就停，跟核心的那一头崩了就退 |
| `crates/miyu-onebot/src/status_file.rs`（O-18） | 状态文件 `state/packages/onebot/status.json`：起来时、NapCat 连上断开、问到是哪个实现、换了端口时（推送来的、`/apply` 的）照这一刻写（「状态文件」） |
| `crates/miyu-onebot/src/control.rs`（O-18） | `start`、`stop`、`restart`、`status`：照终端的样子连核心，调 `extension.enable`、`disable`、`restart`、`status`；`status` 再读状态文件；照回应说 |
| `crates/miyu-onebot/src/logs.rs`（O-18） | `logs [-f]`：印运行日志，标准错误那一份有内容的先另起一段；`-f` 跟着看 |
| `crates/miyu-onebot/src/current.rs`（O-16 补二，O-20 改） | 桥手里最新的配置：握手交来的那一份，推送来了照它换（O-20）；NapCat 的监听、WebUI 共用 |
| `crates/miyu-onebot/src/web.rs`（O-16） | WebUI：核对 Host，`/ws` 原样转给核心、`/status`、`/token`、`/apply`（补二）、`/human`、页面文件；底子是共用的 `miyu-webserve`（`webserve.md`） |
| `crates/miyu-onebot/src/web/login.rs`、`web/checked.rs`（O-16） | 登录令牌拿去和核心握手验、验过的记一阵（只记哈希）；`/status`、`/token`、`/apply` 都照它（补二从 `status.rs` 挪出来） |
| `crates/miyu-onebot/src/web/status.rs`、`web/token.rs`、`web/apply.rs`（O-16，后两个补二） | `/status`：NapCat 的状态、两个端口、令牌设没设、平台的名字（O-17）；`/token`：令牌的值；`/apply`：照桥手里最新的配置换端口，推送来的端口变化也照它换（O-20） |
| `crates/miyu-onebot/src/web/human.rs`（O-16） | `/human`：登录以前页面要的字 |
| `crates/miyu-onebot/src/open.rs`（O-16） | `miyu-onebot web`：桥在不在跑、要一次性码、开浏览器 |
| `crates/miyu-onebot/src/listen.rs` | NapCat 反连进来的那一下：路径、令牌（和桥手里最新的比，O-20）、升级 |
| `crates/miyu-onebot/src/listen/connection.rs`、`listen/bots.rs` | 一条 NapCat 的连接：回应、消息（私聊、群，O-22）和撤回（O-22）、别的事件各交给谁，连上就问 `get_version_info`（回的实现、版本记在这条连接上，O-16 的 `/status` 用）；一个机器人号一条连接，新的顶掉旧的；号认出来了（`X-Self-ID` 或第一条事件）告诉跟核心的那一头（`Event::Connected`，O-25 中：这个号排着的照先后发） |
| `crates/miyu-onebot/src/onebot.rs`、`onebot/text.rs`、`onebot/calls.rs` | OneBot v11 的事件和动作：认一帧（私聊、群消息带上事件的 `time`、发的人的名字和认出来的段，撤回，O-22；她被禁言、解禁的 `group_ban`，O-25 中）、读出私聊的文字、写 `send_private_msg`、`send_group_msg`（O-22；O-25 上第一段能带引用和 @：`Lead`）、`delete_msg`（O-25 上）、`set_msg_emoji_like`（O-25 下）、调用和回应按 `echo` 配对；平台的名字 `qq`（`PLATFORM`）只写在 `onebot.rs`。几个小函数照它拼编号：`venue(种类, 号) -> Result<Venue, FormatError>`（O-22）、`private_venue(号) -> Result<VenueId, FormatError>`、`person(号) -> Result<ExternalId, FormatError>` 经群聊内核拼（`Venue::new`、`miyu_chat::person`），`command_id(机器人的号, 消息编号, 时刻) -> String`（第 7、8 条） |
| `crates/miyu-onebot/src/onebot/segments.rs`（O-22） | 认消息段：正文（字、@、占位）、引用、@全体、带的东西（「群消息」第 2 条） |
| `crates/miyu-onebot/src/onebot/members.rs`（O-22） | 群成员的名字缓存：按群、按号记一阵；`get_group_member_info` 的参数和回应里的名字（「群消息」第 5 条） |
| `crates/miyu-onebot/src/core.rs`、`core/route.rs` | 跟核心的那一头：在给的管道上（`Pipe`：程序里是标准输入输出，O-18；测试里是内存里的管道）握手、不带凭据，取握手回应的 `config`（O-20）、桥自己的 `account`（第 7 条），私聊带 `as`、`venue` 的 `session.send`（O-22 带 `venue`）、她的回复发回去；推来的 `extension.config` 交给 `serve.rs`（O-20），`onebot.trusted` 自己记一份（O-23）；`core.rs` 交出留着的一个会话的推送（O-23，「群里怎么叫她」第 3 条） |
| `crates/miyu-onebot/src/core/caller.rs`（O-23 下） | 并着发的调用口（`Caller`）：问判官的任务经它调 `venue.records`、`model.call`，不等跟核心的那一头手上的事；回应照编号分给等它的那一个，等不到了的回应来了丢掉（「群里怎么叫她」第 12 条，「施工时定的」第 86 条） |
| `crates/miyu-onebot/src/core/route/session.rs`（O-22 从 `route.rs` 挪出来） | 找会话：`venue.session`（私聊认陌生人、群的规则写错只记一行）、订阅私聊的、群的从头订阅（O-23）、会话不在了再找一次 |
| `crates/miyu-onebot/src/core/route/group.rs`、`route/names.rs`（O-22） | 群消息：套场所规则、找会话、正文里的 @ 写成名字（缓存里没有的问 NapCat）、记成旁听（「群消息」），记下了交给 `called.rs` 判（O-23） |
| `crates/miyu-onebot/src/core/route/projection.rs`（O-23） | 从日志投影一个群（纯逻辑）：人说的话谁发的、是不是主人、什么时刻，开过的回合，主线这一轮回的是谁，她的回复，她发过的平台编号，限流提示过的时刻，判过要回、她还没回完的（O-23 下），她被禁言到什么时候（O-25 中）；她新说的话交出来，O-25 上连同出站链要的：她回的那一条、那之后的动静、这一轮入队了的（O-25 中照 `ext.onebot.venues.queued`，桥入队记成了先算进去）（「群里怎么叫她」第 1、2、9、11 条） |
| `crates/miyu-onebot/src/core/route/decide.rs`（O-23） | 判一条（纯逻辑）：过进站链、加值项，O-23 下多线路规程、顶替、额度满了，走哪条路、结论（第 5、6、10、11、14 条） |
| `crates/miyu-onebot/src/core/route/discipline.rs`（O-23 下） | 线路规程（纯逻辑）：四种各留下哪些条件、看不看顶替、走哪条路（第 10 条） |
| `crates/miyu-onebot/src/core/route/body.rs`（O-23 下，从 `decide.rs` 挪出来） | `ext.onebot.chat.decided` 的 `body`（纯逻辑，第 7 条那张表） |
| `crates/miyu-onebot/src/core/route/called.rs`（O-23） | 叫她：群消息记下以后，照场所规则、投影、这一条填好交 `decide.rs`，记判断，开一轮或回一句提示（入队，O-25 中）；要问判官的交给 `judges.rs`（第 3 到 8、11 条）；`Ctx.muted` 照投影（O-25 中） |
| `crates/miyu-onebot/src/core/route/judges.rs`（O-23 下） | 在判的：桥内存里的 `Judging`、问判官的任务、全局的名额（第 11、12 条）；群会话用的人格（O-23 补，第 1 条） |
| `crates/miyu-onebot/src/core/route/ask.rs`（O-23 下） | 问一次判官（一个任务）：排队、`venue.records`、带上人格（O-23 补）、拼请求、`model.call`、读回答、重试（第 12、13 条） |
| `crates/miyu-onebot/src/core/route/persona.rs`（O-23 补） | 判官带的人格的原文：`persona.read` 读，读到的记一阵，几个问判官的任务共用（第 12 条第 3 款） |
| `crates/miyu-onebot/src/core/route/judged.rs`（O-23 下） | 判官回来了：算分、记判断、回的开一轮；被放下的丢掉（第 7、11、13 条） |
| `crates/miyu-onebot/src/core/route/speak.rs`（O-23） | 群会话推来的事件：收进投影，她新说的话过出站链（O-25 上，`outbound.rs`）、照纯文本拆段，一段一条入队（O-25 中，`sending.rs`），第一段带引用和 @，NapCat 回了成功的记 `venue.delivered`（第 1、9 条）；私聊里她的话也从这里过出站链、入队（「怎么走」第 10 条，O-25 上） |
| `crates/miyu-onebot/src/core/route/queue.rs`（O-25 中） | 出站队列（纯逻辑，钟由调的一方交进来）：每个场所会话一条先进先出的队，排着的过期没有，门开着的照先后交出来；下一次该醒的时刻（最早的过期、禁言到期）；禁言到什么时候（此刻加秒数）；NapCat 的回应算成功还是失败、为什么（「出站队列」） |
| `crates/miyu-onebot/src/core/route/sending.rs`（O-25 中） | 出站队列的那一头：入队先记 `ext.onebot.venues.queued`、拿到序号，门开着（没被禁言、号连着）的照先后放进写队列、回应交给别的任务等；结局照放进写队列的先后交回来，群里她的话记 `venue.delivered`、群里的回执交 `receipt.rs` 撤，失败的记 `ext.onebot.venues.failed`，她的话失败了经 `session.note` 退信（O-25 下，「退信」）；连上了、解禁了、定时醒了把排着的再看一遍（「出站队列」） |
| `crates/miyu-onebot/src/core/route/muted.rs`（O-25 中） | 她被禁言、解禁：记 `ext.onebot.venues.muted {until}`、`unmuted`，收进投影，排着的再看一遍（「出站队列」第 7 条） |
| `crates/miyu-onebot/src/core/route/outbound.rs`（O-25 上） | 出站（纯逻辑）：她的一条话过群聊内核的出站链，群里的情形照投影填，私聊的照桥这一轮自己入队了的（`Spoken`，O-25 中改成入队时记）；过了的照 `plain`、`split` 拆段，交出几段和第一段带的引用、@；丢了的原因写成什么（第 9 条） |
| `crates/miyu-onebot/src/core/route/receipt.rs`（O-25 上） | 命令回执：入队（O-25 中）；群里的 NapCat 回了编号，`receipt_recall_seconds` 秒后经那时的连接 `delete_msg` 撤回，撤不成的记一行（「斜杠命令」第 7 条） |
| `crates/miyu-onebot/src/core/route/reaction.rs`（O-25 下） | 贴表情：判下来要回、主触发是冲她来或续聊的群消息，`session.respond` 成了以后在她要回的那一条上贴；记着贴着的几条各进了哪一轮，推来的 `venue.delivered`、`turn.ended` 发摘的信号；一条一个另起的任务：贴、等信号或到时候、摘，只摘一次（「贴表情」） |
| `crates/miyu-onebot/src/core/route/applied.rs`（O-22） | 场所规则套到这一条上：`venue.session` 带的人格、预设、工作区，`managers` 认的身份，`show_ids`，此刻睡没睡；O-23 多进站链要的 `rate`、`sleep`、`allow`、`keywords` 和此刻（本机的钟、时区） |
| `crates/miyu-onebot/src/core/route/fields.rs`（O-22） | `session.send` 的 `venue` 格：照核心的写法洗名字、编号（「群消息」第 6 条） |
| `crates/miyu-onebot/src/core/route/recall.rs`（O-22） | 撤回记 `venue.recalled`（「撤回」） |
| `crates/miyu-onebot/src/core/route/command.rs`（O-19） | 斜杠命令：`/` 开头的先交 `command.run`，回执、被拒的那一句发回去（群里的发回群里，O-22；交 `receipt.rs`，群里的过几秒撤回，O-25 上），认不出的交回去照普通的话发（「斜杠命令」） |
| `crates/miyu-onebot/src/rules.rs`（O-21） | 场所规则和出厂数据（「场所规则和出厂数据」）：出厂的起来时读一次、查一次（`Factory`）；系统的照群聊内核读好、和出厂的合起来（`load`、`Loaded`），套到场所上（`Loaded::at`）；什么时候重读（`Venues`：隔一阵看一眼系统的两处变没变，变了整份重读，问题记运行日志） |
| `crates/miyu-onebot/src/rules/judge.rs`（O-23 下） | 读判官的说明：资源 `software/onebot/judge/` 的十三份，交 `JudgeTexts::new` 查（「场所规则和出厂数据」第 2 条） |
| `crates/miyu-onebot/src/rules/facts.rs`（O-25 下） | 读给她看的事实的模板：资源 `software/onebot/facts/undelivered.txt`，`Template::parse` 读、字段只认 `why`、`detail`、`text`（「退信」第 3 条，「场所规则和出厂数据」第 2 条） |
| `crates/miyu-onebot/src/rules/files.rs`（O-21） | 读文件：照配置文件的读法读一份（`miyu_store::config_file::read`），读不了的变成群聊内核的 `Problem`；列出 `venues.d/` 里的规则文件；系统的两处这一刻的样子（文件列表、修改时刻、大小） |
| `crates/miyu-onebot/src/venue.rs`（O-21） | `venue show <场所>`：不连核心，照 `rules.rs` 读同样的文件，一项一行印值和来处，问题印在后面 |
| `crates/miyu-onebot/src/texts.rs` | 说给人听的字：挑哪一句、换进什么字段，字照 `Human::load` 读（「给人看的字」）；O-23 多发进群里的限流提示 |
| `resources/software/onebot/bridge.json` | 桥自己的数：认的路径、调用等多久、两个队列多长、接不了连接歇多久、握手等多久和 `logs -f` 隔多久看一次（O-18）、隔多久看一次系统的场所规则变没变（O-21）、群成员的名字记多久（O-22）、判官全局最多同时问几个、排队等多久（O-23 下）、判官带的人格原文记多久（O-23 补）、群里的命令回执几秒后撤回（O-25 上）、出站排着的多久过期（O-25 中）、贴的表情和多久摘（O-25 下）（「对外的样子」） |
| `resources/software/onebot/venues.d/`、`defaults.toml`、`moderation.txt` | 出厂的场所规则、出厂参数、违规词表（写法、内容在 `chat.md` 第一条、第八条、第二条）；O-21 起桥读它们（「场所规则和出厂数据」） |
| `resources/software/onebot/facts/undelivered.txt`（O-25 下） | 退信那一块事实的模板（给模型看的字，登记在 26 第十节；「退信」第 3 条） |
| `resources/software/onebot/human/{zh,en,ja}.json` | 桥说给人听的字（「给人看的字」）；WebUI 页面的字（`web/` 开头，O-16；`web/people/` 开头的 O-17，第二条「给人看的字」） |
| `resources/software/onebot/web/`（O-16） | WebUI 的页面：`index.html`、`app.js`（登录、骨架、「连接」页）、`people.js`（「主人与自己人」页，O-17）、`style.css`，原生 JS 的模块（第二条「施工时定的」第 25 条） |
| `xtask/src/ledger.rs` | 登记簿门禁豁免 `software/onebot/bridge.json` 这一份文件：是数据，不发给模型；O-16 再豁免 `software/onebot/web/` 这一个目录：给浏览器的 |

### 一、骨架：主人的私聊（施工 O-8）

**对外的样子**

配置（软件包清单的 `[settings]`，O-20 从核心挪过来；设置页在「软件包」那一页的「QQ 桥」组，名字、说明三种语言写在清单里。都只能写在系统配置，都当场生效：核心拉起桥时在握手的回应里交来最终值，变了推过来，桥照新的用，不用重启（第 1、2 条）。`onebot.web`、`onebot.trusted` 见第二条「对外的样子」）：

| 键 | 类型 | 默认 | 说明 |
|---|---|---|---|
| `onebot.listen` | 整数 1024 到 65535 | 8301 | NapCat 反连进来的端口，只听本机 `127.0.0.1`；改了当场换（照 `/apply` 的办法，第 1 条，O-20） |
| `onebot.token` | 密钥（`{ secret = … }` 或 `{ env = … }`） | 没有 | NapCat 连进来时出示的访问令牌；没设、取不到，桥照样起来，NapCat 连进来一律 401；设了、换了、删了不用重启（第 1、2 条，O-16 补二、O-20） |

桥自己的数在资源目录的 `software/onebot/bridge.json`，不进配置清单（照网页软件的 `web.json`）：

| 格 | 出厂 | 说明 |
|---|---|---|
| `paths` | `["/onebot/v11/ws", "/ws"]` | NapCat 连得进来的路径（第 2 条） |
| `call_timeout_seconds` | 10 | 一次 OneBot 调用等回应最多几秒（第 4 条） |
| `write_queue` | 64 | 往一条 NapCat 的连接写，最多攒几帧没写出去；满了写的一方等着。至少 1 |
| `inbound_queue` | 256 | 读出来的消息、撤回（O-22 起群的也算）最多攒几条没交给跟核心的那一头；满了读 NapCat 的那一头等着。至少 1 |
| `accept_retry_millis` | 100 | 接不了 TCP 连接（打开的文件太多这类）时歇几毫秒再接，不空转 |
| `hello_seconds` | 10 | 跟核心握手，最多等几秒回应；等不到的（从终端跑起来的）说 `failure/not-spawned`、退出码 1（第 1 条，O-18） |
| `follow_millis` | 500 | `logs -f` 隔几毫秒看一次运行日志长了没有（O-18） |
| `rules_check_millis` | 1000 | 要用场所规则时，隔几毫秒才看一眼系统的两处变没变（O-21，「场所规则和出厂数据」第 3 条） |
| `member_names_seconds` | 600 | 群成员的名字记几秒（O-22，「群消息」第 5 条） |
| `judge_concurrency` | 4 | 判官全局最多同时问几个（O-23 下，「群里怎么叫她」第 12 条；18 第七节）。至少 1 |
| `judge_queue_seconds` | 15 | 名额满了，问判官的排队最多等几秒，等不到的当判不了（O-23 下，同上） |
| `judge_persona_seconds` | 60 | 判官带的人格原文读到以后记几秒，这段时间里同一个人格不再读（O-23 补，「群里怎么叫她」第 12 条第 3 款） |
| `receipt_recall_seconds` | 3 | 群里的命令回执发出去几秒后撤回（O-25 上，「斜杠命令」第 7 条；18 第十节）。0 是 NapCat 回了就撤 |
| `queue_expire_seconds` | 60 | 出站排着的（她被禁言、机器人号没连着）入队以后过几秒还没交出去的作废，记 `failed`（O-25 中，「出站队列」第 5 条）。至少 1 |
| `reaction_emoji` | `"289"` | 群里判过要回的那一条上贴哪个表情：QQ 表情的编号，写成字（O-25 下，「贴表情」第 2 条；18 第七节） |
| `reaction_seconds` | 600 | 贴了以后过几秒她还没回、这一轮还没完的，摘掉（O-25 下，「贴表情」第 3 条）。0 是贴了就摘 |
| `web` | 见第二条「对外的样子」 | WebUI 的数（O-16）：`csp`、`types`、`status_cache_seconds` |

多一格、少一格、队列写 0、判官的并发写 0、排着的过期写 0、读不了：起不来（「出错」）。

命令（O-18，18 第三节 Q17）：`miyu onebot …` 经核心的子命令转交（9-2，`packages.md`「怎么走」第 5 条）到 `miyu` 旁边的 `miyu-onebot …`，直接跑 `miyu-onebot …` 也一样。

| 命令 | 做什么 |
|---|---|
| `start` | 调 `extension.enable`：打开开关，核心拉起桥，以后核心每次起来都拉起它。说 `control/started`，再照回应说它这时的样子（同 `status` 的第一句） |
| `stop` | 调 `extension.disable`：关开关，核心请桥退出、等它退出（`extensions.md`「怎么走」第 4 条）。说 `control/stopped` |
| `restart` | 调 `extension.restart`：核心请桥退出、重新拉起，连续失败从零数。说 `control/restarted`，再照回应说它这时的样子。关着的核心拒绝（`extension_off`） |
| `status` | 调 `extension.status`，取 `onebot` 那一个说：关着、正在起来、在跑（进程号）、退避中（几秒后再拉起、连续失败几次）、停下了（原因，带标准错误的最后几行）。在跑的、状态文件的进程号和它对得上的，再说 NapCat 连没连上、哪个实现和版本、机器人的号、两个地址（「状态文件」）。几个场所、出站队列积压几条随后面的步子 |
| `logs [-f]` | 印运行日志 `state/logs/onebot.log`；标准错误那一份 `state/logs/onebot.stderr` 有内容的，先印它、再印运行日志，各带一行标题。`-f`：印完接着跟运行日志，每 `follow_millis` 看一次，文件变短了（换了一份）从头读，Ctrl+C 停 |
| `web [--print]` | 打开 WebUI（第二条） |
| `venue show <场所>`（O-21） | 一个场所每一项的值和来处，像 `udevadm info`（18 第四节「看和改」）：场所编号写成 `qq:group:<群号>`、`qq:private:<号>`。不连核心，照系统的语言说；读的文件和桥一样（「场所规则和出厂数据」第 6 条） |
| `serve` | 只由核心拉起（`extensions.md`）：标准输入输出是协议，说给人听的在标准错误上。从终端跑起来，照协议发握手、等回应，`hello_seconds` 内等不到就说 `failure/not-spawned`、退出码 1 |
| `-h`、`--help` | 用法印在标准输出上，退出码 0（`miyu help onebot` 转成 `--help`，9-2） |

`start`、`stop`、`restart`、`status` 照终端的样子连核心（出示本机令牌，没在跑就拉起，和 `miyu-onebot web` 同一个 `miyu_webserve::open::Core`），握手以后照核心回的语言说。说的印在标准输出上，退出码 0；连不上核心（`failure/core`）、核心拒绝（照核心的原话）、核心那边没有 `onebot` 这个包（`status/missing`）印在标准错误上，退出码 1。`logs` 不连核心：日志印在标准输出上（原样的字节），还没有运行日志的在标准错误上说 `logs/none`。`venue show` 也不连核心：印在标准输出上，退出码 0（读文件时发现了问题也是 0：问题是印出来的一部分）；场所编号认不出的在标准错误上说 `venue/bad-venue`，退出码 2；出厂的数据有问题的在标准错误上说 `failure/factory` 和每一条问题，退出码 1。

**软件包清单**（O-18，`resources/packages/onebot.toml`，照 `packages.md`）：

```toml
[package]
kind = "process"
protocol = [1, 1]
name = { en = "QQ bridge", zh = "QQ 桥", ja = "QQ ブリッジ" }
summary = { en = "Talk with her on QQ through NapCat", zh = "经 NapCat 在 QQ 上和她说话", ja = "NapCat 経由で QQ で会話する" }

[command]
name = "onebot"
program = "miyu-onebot"
about = { en = "Start, stop and look at the QQ bridge", zh = "开、关、查看 QQ 桥", ja = "QQ ブリッジを起動・停止・確認する" }

[process]
args = ["serve"]
start = "manual"
capabilities = ["sessions.drive", "act_for_external", "events.read", "events.write", "network"]

[settings.listen]
type = "int"
min = 1024
max = 65535
default = 8301
layers = ["system"]
applies = "now"
name = { … }
description = { … }

[settings.web]
type = "int"
min = 1024
max = 65535
default = 8302
layers = ["system"]
applies = "now"
name = { … }
description = { … }

[settings.token]
type = "secret"
layers = ["system"]
applies = "now"
name = { … }
description = { … }

[settings.trusted]
type = "list"
element = "text"
max = 128
layers = ["system"]
applies = "now"
name = { … }
description = { … }
```

不写 `[check]`：随 `miyu onebot check` 那一步。`capabilities` 随 9-4（下上）；O-23 多 `events.write`：桥记 `ext.onebot.*`、`venue.*`（`events.append`，`05-内核接口.md` 第三节）。`[settings]` 四项（O-20）照核心原来替桥声明的那一份（O-8 的 `OnebotSettings`）：类型、默认值、层一样；生效时机都是 `now`（两个端口原来是 `head_start`：推送来了桥当场换，「施工时定的」第 40 条）；设置页从高级页的「QQ 桥」组挪到「软件包」那一页、这个包那一组（组名是包的名字「QQ 桥」，`packages.md`「配置项」第 4 条）。名字、说明（`name`、`description`，三种语言写在清单里，不进 `core/human`）：

| 键 | 中文 | 英文 | 日文 |
|---|---|---|---|
| `listen` 名字 | QQ 桥的端口 | QQ bridge port | QQ ブリッジのポート |
| 说明 | NapCat 反向 WebSocket 连进来的端口，只听本机。NapCat 那边的地址填 ws://127.0.0.1:<端口>/ws。 | The port NapCat's reverse WebSocket connects to, local only. In NapCat use ws://127.0.0.1:<port>/ws. | NapCat のリバース WebSocket が接続してくるポートです。このマシンからだけ受け付けます。NapCat 側のアドレスは ws://127.0.0.1:<ポート>/ws です。 |
| `web` 名字 | QQ 桥网页的端口 | QQ bridge web port | QQ ブリッジの Web ページのポート |
| 说明 | QQ 桥自己的网页（连接 NapCat、换令牌）的端口，只听本机。用 miyu-onebot web 打开。 | The port of the QQ bridge's own web page (NapCat connection, token), local only. Open it with miyu-onebot web. | QQ ブリッジ自身の Web ページ（NapCat の接続、トークン）のポートです。このマシンからだけ受け付けます。miyu-onebot web で開きます。 |
| `token` 名字 | QQ 桥的令牌 | QQ bridge token | QQ ブリッジのトークン |
| 说明 | NapCat 连进来时要出示的访问令牌，NapCat 那边填同一个。写 { secret = "名字" }（用 miyu login 存）或 { env = "环境变量" }。没设的，QQ 桥照样起来，NapCat 连进来会被拒；在 QQ 桥的网页上能生成一个。改了以后，NapCat 下一次连进来就照新的。 | The access token NapCat shows when it connects; set the same one in NapCat. Write { secret = "name" } (saved with miyu login) or { env = "VARIABLE" }. Without it the QQ bridge still starts but refuses NapCat; you can generate one on the QQ bridge web page. After a change, NapCat's next connection uses the new one. | NapCat が接続するときに示すアクセストークンです。NapCat 側にも同じものを設定します。{ secret = "名前" }（miyu login で保存）か { env = "環境変数" } を書きます。未設定でも QQ ブリッジは起動しますが、NapCat の接続は拒否します。QQ ブリッジの Web ページで生成できます。変えると、NapCat の次の接続から新しいものを使います。 |
| `trusted` 名字 | QQ 桥的自己人 | QQ bridge friends | QQ ブリッジの仲間 |
| 说明 | 私聊里能叫她、不限流、睡着时私聊也放行的人，写平台上的身份的列表，例如 ["qq:20017"]。在 QQ 桥的网页上改。QQ 桥接通群以后才照它认人。 | People who can call her in private chat, with no rate limit, even while she sleeps: a list of platform identities, for example ["qq:20017"]. Edit it on the QQ bridge web page. The QQ bridge uses it once groups are connected. | 個人チャットで呼べて、回数制限がなく、寝ている間も個人チャットが通る人です。プラットフォーム上の ID のリストで書きます。例：["qq:20017"]。QQ ブリッジの Web ページで変えます。QQ ブリッジがグループにつながってから、これで人を見分けます。 |

**状态文件**（O-18）：桥把这一刻的样子写进 `<数据根>/state/packages/onebot/status.json`（核心给的工作目录就是这个目录；照数据根算，不照相对路径）：

```json
{"pid": 12345, "listen": 8301, "web": 8302, "napcat": {"connected": true, "self_id": "30003", "implementation": "NapCat.Onebot", "version": "4.8.2"}}
```

- `pid`：桥的进程号；`listen`、`web`：实际听的两个端口（换过的照换过的：推送来的、`/apply` 的）；`napcat`：和 WebUI 的 `/status` 一样（连着的号里最小的那一个，问到了是哪个实现的才带 `implementation`、`version`；没连着的只有 `connected: false`）。
- 什么时候写：两个端口听上以后写一次；NapCat 连上、断开、认出号、问到是哪个实现，换了端口（推送来的、`/apply` 的），照这一刻的再写。和磁盘上一样的不写；先写旁边的临时文件再改名盖上（`miyu_store::generated::write`），读的人看不到写了一半的。写不进记一行 `WARN status not written`，桥照跑。
- 桥退出不删它：`status` 只在 `extension.status` 说在跑、进程号和文件里的一样时才用它，旧的、上一个进程的、读不懂的都不用。

NapCat 那边要配成「反向 WebSocket」（NapCat 的网络配置里叫「WebSocket 客户端」），地址 `ws://127.0.0.1:<onebot.listen>/ws`（`/onebot/v11/ws` 也认，WebUI 上写短的那个，O-16 补二），访问令牌和 `onebot.token` 一样，消息格式选数组（字符串格式也认，第 6 条）。

**怎么走**

1. **起来**（O-18，O-20 改）：由核心拉起（`extensions.md`「怎么走」第 1 条：工作目录是 `state/packages/onebot/`，环境里的 `MIYU_HOME`、`MIYU_RESOURCES` 是核心正在用的那两个，标准错误接到 `state/logs/onebot.stderr`）。找资源目录、读给人看的字（照系统的语言）；读 `bridge.json` 和资源目录里自己的清单（`packages/onebot.toml`）的 `[settings]` 里两个端口的默认值（O-20）。握手以前不读配置、不说话（O-20）：起不来的（资源目录、给人看的字、`bridge.json`、清单读不进来）照系统的语言说一句、退出码 1；运行日志装不上的那一句等握手以后照握手回的语言说（握手不成的，说握手不成那一句以前先说它）。经标准输入输出跟核心握手：照头的样子，`head.kind = "onebot"`，不带凭据（核心亲手给的管道，`protocol.md`「握手」第 3 条）。`hello_seconds`（出厂 10 秒）内等不到回应的（从终端跑起来的）说 `failure/not-spawned`、退出码 1；被拒、握手时管道关了、回应没带 `language`（`protocol.md`「握手」说一定带，没带是协议不对），照连不上核心说、退出码 1。握手回了语言，以后说的都照它，端口被占那一句也是（O-20：O-19 时发现原来只在「听上了」那一刻换语言，端口被占那一句照的还是握手以前的语言）。标准输出只给协议，一行一条，桥别处不往上面写；说给人听的都在标准错误上，运行日志照旧写 `onebot.log`。握手回应的 `config`（`extensions.md`「配置」：这个包自己的键、最终值，密钥是真值，没设、取不到的那一键不放）取两个端口和令牌（O-20）：`onebot.listen`、`onebot.web` 是 0 到 65535 的整数的照它（核心只收 1024 到 65535；0 是测试让系统挑），没有的、不是的照清单的默认值（出厂 8301、8302，「施工时定的」第 38 条）；`onebot.token` 是字的照它（去掉前后空白），没有的、空的就是没有令牌；`onebot.trusted` 由跟核心的那一头读成自己人（O-23，「群里怎么叫她」第 3 条；推来的照样换），别的键不认。然后开两个监听（NapCat 的、WebUI 的）；端口被占了说清是哪个端口，退出码 1：核心不再重启（`config_error`），`miyu onebot status` 带出标准错误的最后几行。听上了写一次状态文件（「状态文件」）。令牌没设、引用的密钥或环境变量取不到：照样起来，两个端口照开，说完在哪等 NapCat 再说一句 `notice/no-token`（到 WebUI 的「连接」页生成一个，填进 NapCat，几秒内就连上），运行日志记一行 `WARN no token yet`。NapCat 这时连进来一律 401；人在 WebUI 里生成令牌以后，NapCat 下一次连就通，不用重启（第 2 条；18 第三节「还没配好就 `start`：桥只开 WebUI，不连 NapCat，等配好」，O-16 补、补二）。之后核心推 `extension.config`（`{"keys": {键: 新值或 null}}`，只放变了的键，不用订阅），桥照它换上手里的那一份（`current.rs`，O-20）：`null` 的当没有（端口照清单的默认值，令牌没了）；令牌换了、没了照第 2 条；两个端口变了的照第二条 `/apply` 的办法当场换（先开新的，都开上了才关旧的、换上新的，状态文件跟着写；有一个开不了，两个都不换，旧的照旧开着，运行日志记一行 `WARN apply port in use`），已经连着的（NapCat 的那一条、页面正在用的连接）不断。推来的值不进运行日志：令牌换了只记一行 `INFO token changed`，端口换了记 `INFO applied listen=… web=…`。
2. **连进来**：只认 `bridge.json` 的 `paths`（出厂 `/onebot/v11/ws` 和 `/ws` 两个），别的 404。令牌照 `Authorization: Bearer <令牌>`、`Authorization: Token <令牌>` 或查询参数 `access_token` 取，和桥手里最新的那个（握手交来的，推送换过的照换过的，第 1 条）按常数时间比；对不上、桥手里没有令牌的 401（O-20：去掉了 O-16 补二「对不上时重读一次配置再比」，推送来了就换，用不着读盘）。令牌换了以后，已经连着的那一条不断（它握手时出示的是当时对的），下次重连照新的；令牌删了的，以后连进来的一律 401。不是 WebSocket 的升级请求 400；算出来的 `Sec-WebSocket-Accept` 放不进回应的头（照说不会），也是 400，不升级。机器人的号照 `X-Self-ID` 头取，没有的等第一条事件的 `self_id`。同一个号再连进来，新的顶掉旧的。号认出来了（连上时的 `X-Self-ID`，或者第一条事件），告诉跟核心的那一头（`Event::Connected {bot}`，O-25 中）：排着的照先后发出去（「出站队列」第 3 条）。
3. **对端是谁**：连上以后调一次 `get_version_info`，把实现的名字和版本记进运行日志（18 第十三节：排查时先确认连着的是哪个实现）。quirk 表随后面的步子。
4. **调用**：发出去的动作带 `echo`（桥自己编，同一条连接里不重），等带同样 `echo` 的回应，等了 `call_timeout_seconds`（出厂 10 秒）还等不到算失败；连接断了，在等的都算失败。
5. **收消息**：`post_type = message` 的私聊（`message_type = private`）、群消息（`group`，O-22，「群消息」）和 `post_type = notice` 的两种撤回（O-22，「撤回」）、她被禁言和解禁（`group_ban`，O-25 中，「出站队列」第 7 条）才看；机器人自己发的（`user_id` 等于 `self_id`）不看；别的事件（别的通知、请求、心跳、生命周期）记一行调试日志就丢。
6. **读出文字**：`message` 是段的数组时，取 `text` 段的 `data.text` 依次接起来；是字符串（CQ 码）时，去掉 `[CQ:…]`，再把 `&#91;`、`&#93;`、`&#44;`、`&amp;` 换回来。图片、表情这些别的段这一步不管。接出来的字去掉首尾空白是空的，不送。
7. **找会话**：`venue.session {venue: "qq:private:<user_id>", kind: "private", peer: "qq:<user_id>"}`。场所编号由群聊内核的 `Venue::new("qq", VenueKind::Private, <user_id>)` 拼，平台上的人由 `miyu_chat::person("qq", <user_id>)` 拼，桥不手拼（`chat.md` 第七条第 1 条）；拼不出来的（号是整数，照说不会）记一行运行日志 `WARN`、这条不送，不 panic。桥在内存里记着「场所 → 会话编号」，每个场所只在桥起来以后第一次来消息时问；回 `session_not_found` 这类会话不在了的，忘掉、再问一次。回 `no_system_account`（不是主人，O-4 以前没有系统账号）：这个人的消息这一步不接，同一个人只记一行运行日志。回应里带 `account`（这个会话的属主，核心 O-4 中起找回、新造都带）、而且等于桥自己的账号（握手回应的 `account`：O-4 中以后核心拉起的桥是系统账号 `onebot`）的，是陌生人：照 `no_system_account` 办，这个人的消息不进任何会话、不回话，同一个人只记一行运行日志，会话编号不记进「场所 → 会话」、不订阅，下一条照样再问。没带 `account` 的照常接。接群那一步以前私聊只接主人，接群时照进站链判（`chat.md`）（「施工时定的」第 49 条）。
**斜杠命令**（O-19，2026-10-08；`venues.md`「斜杠命令」、`protocol.md` 的 `command.run`）：第 7 条找到会话以后、第 8 条以前走这一段（不占编号）。

1. **认**：私聊的文字去掉开头的空白（照核心的认法，Unicode 的空白）以后以 `/` 开头的，先当命令交 `command.run {session, text, as: {external}}`：`text` 照原样（同第 8 条，不去掉空白），`as.external` 同第 8 条；`cwd` 不带（场所会话没有头所在的目录，相对路径照会话现在的工作区接）。请求的 `id` 和第 8 条同一个拼法（`qq:<机器人的号>:<消息编号>:<时刻>`）：一条消息要么是命令、要么是话，编号只用一次；断线重发、平台重发核心都只执行一次（`command.run` 第 7 条）。会话不在了照第 7 条忘掉、再找、再交一次。
2. **成了**：回应的 `said` 原样经 `send_private_msg` 发回给这个人，和她的回话走同一条路（第 10 条：入队，收进它的那个机器人号现在的连接，空的不发、没连着的排着，「出站队列」）。她不开新的一轮。同一条消息重发的，核心回的和头一次一样，回执照样再发一次，命令只执行一次。
3. **不是命令**：核心回 `unknown_command`（认不出的 `/xxx`；`/` 后面是空白、只有一个 `/` 的也是，`command.run` 第 1 条）：照普通的话走第 8 条，同一个编号，她照常看到（例如有人发一个路径 `/home/...`）。
4. **被拒**：别的原因（`command_not_allowed`、`owner_only`、`turn_running`、`nothing_to_clear`、`memory_unavailable`、`memory_too_long`、`path_unreadable`、`bad_params`……）：核心拒绝的回应里 `error.message` 就是照这个连接的语言写好的那一句（`protocol.md`「出错」「给人看的字」），原样发回给这个人（同第 2 条那条路）；不交给她，核心什么都不记。场所会话没有记忆（`memory.md`），私聊里 `/remember` 回的是 `memory_unavailable` 那一句。
5. **运行日志**：成了记 `INFO command ran venue=… message=… command=<正名>`（正名照回应的 `command`），被拒记 `INFO command refused venue=… message=… reason=<原因码>`，不是命令记 `DEBUG not a command venue=… message=… reason=…`、再照第 8 条记；回执、被拒的那一句发出去和她的回话一样记 `reply sent`。都不记原文（`/remember` 后面是人说的话）。
6. **`/stop` 打断这一轮**：桥不等她的回话（第 8 条、这一段都只等核心的回应），不会卡住；她这一轮没说出字的，照第 10 条不发空的。
7. **群里的回执撤回**（O-25 上，18 第十节「命令回执带着 3 秒后撤回入队」）：群里发回去的那一句（第 2 条的回执、第 4 条被拒的那一句，「群消息」第 7 条；和别的一样入队、排着，O-25 中）NapCat 回了编号的，等 `bridge.json` 的 `receipt_recall_seconds`（出厂 3 秒），经收进这个群的那个机器人号那时的连接调 `delete_msg {message_id}` 撤回。撤成了记 `INFO receipt recalled venue=… message=<回执的编号>`；没连着、失败、发的时候 NapCat 回的里没有编号的记 `WARN receipt not recalled venue=… …`。发命令的那条消息不撤；私聊的回执不撤；限流的提示（「群里怎么叫她」第 7 条）不是回执，不撤。不过出站链：回执是核心写的，不是她的话（「施工时定的」第 114 条）。

8. **发进去**：不是命令的（上面「斜杠命令」第 3 条）`session.send {session, text, as: {external: "qq:<user_id>"}}`（`as.external` 和第 7 条的 `peer` 是同一个，照第 7 条拼），请求的 `id`（命令编号）是 `qq:<机器人的号>:<message_id>:<time>`（`chat.md` 第七条第 1 条）：断线重连、平台重发，核心都只收一次（`venues.md`）。`time` 是事件里平台给的时刻，整数秒，写成整数的字符串也认（和号一样）；加上它，NapCat 重置本地库以后消息编号重号，新消息也不会被当成重发吞掉。没带 `time`、读不出来的照 `0` 拼，消息照送（「施工时定的」第 20 条）。回的是重了，当成功。O-22 起带上场所的格 `venue`（`venues.md`「场所的格」）：`msg`、`name`、`reply_to`、`media`、`show_ids` 照「群消息」第 2、6 条认、洗，`show_ids` 照这个私聊套出来的场所规则；不写 `ambient`、`asleep`（私聊每一条都开回合，第 49 条的主人以外的照旧不接）。正文照旧只取 `text` 段（第 6 条），去掉首尾空白是空的照旧不送，只有图的私聊也是（「施工时定的」第 67 条）。`command.run` 不带 `venue`（它不收这一格）。
9. **订阅**：找回一个私聊的会话以后订阅它的事件流（群的从头订阅，O-23，「群里怎么叫她」第 1 条），不写 `after`：只要订阅以后的新事件，桥重启不会把以前的回复再发一遍。桥起来以前、桥停着时她说的不补发（「出站队列」第 6 条）。
10. **发回去**：事件流里每来一条 `message.assistant`，取它的 `text` 块依次接起来（思考、工具调用不发），先过出站链（O-25 上，同「群里怎么叫她」第 9 条：`target` 两样都是假；`sent` 是桥这一轮自己入队了的那几段，入队记成了就算上，内存里一个会话一份，换回合就清，桥重启就丢，「施工时定的」第 113 条），丢了的记一行、不发；过了的照 `plain` 转成纯文本、`split` 照这个私聊套出来的 `Params::split_chars` 拆开，一段一条入队（「出站队列」，O-25 中），照先后 `send_private_msg {user_id, message: [{type: "text", data: {text}}]}` 发回那个私聊（拆出来是空的不发）。哪条连接：这个会话是哪个机器人号收进来的，就用那个号现在的连接；没连着的排着，连上了发（「出站队列」第 3 条）。私聊不记 `venue.delivered`。
11. **断开**：NapCat 断了，等它自己重连，桥不退。标准输入读到头（核心请它退出，或者核心不在了）、标准输出写不进：桥停下，退出码 0，不说话，运行日志记一行 `INFO core closed, stopping`；读到头 5 秒内退出（核心等 5 秒，没退的杀掉）。崩了以后由核心退避重启（O-18，「施工时定的」第 5、21 条）。跟核心的那一头崩了、发回话的任务崩了（都是 bug），说一句带原话、退出码 1，运行日志记一行 `ERROR`。Ctrl+C、SIGTERM 也是好好停下，退出码 0。
12. **运行日志**：照 `miyu-log` 写 `state/logs/onebot.log`，满了照核心的换法。消息正文不进运行日志，只记场所、消息编号、字数（`28-运行日志.md`）。

**群消息**（O-22，2026-10-09；施工单「要定的」三条照推荐定：QQ 的群主、管理员不算 `manager`，@ 的名字照群成员的缓存、拿不到写号，合并转发、卡片写占位）：群里的每一条消息都记进这个群的场所会话，一律旁听，交 `session.send` 时不开回合（18 第五节）；记下以后判要不要开回合是「群里怎么叫她」（O-23），出站链（O-25 上）、出站队列（O-25 中）见「群里怎么叫她」第 9 条、「出站队列」。不占编号，和「斜杠命令」一样（「施工时定的」第 57 条）。

1. **认**：`post_type = message`、`message_type = group`，带得出 `self_id`、`user_id`、`group_id`、`message_id`（整数，写成整数的字符串也认，同第 5 条）；`time` 同第 8 条。机器人自己发的不看（第 5 条，「施工时定的」第 69 条）。
2. **认段**（`onebot/segments.rs`）：`message` 是段的数组时一段一段认：

   | 段 | 记成 |
   |---|---|
   | `text` | 正文，`data.text` 照原样接 |
   | `at` | `data.qq` 是 `all` 的：`mentions_all`，正文里不写（「施工时定的」第 60 条）；是号的：正文里写 `@名字`（第 5 条），是机器人的号的记 `mentions_me`，别人的进 `mentions`（照先后、不重） |
   | `reply` | `reply_to`：`data.id`；几段的只认第一段 |
   | `image` | `image`；`sub_type` 是 1 的（表情包）记成 `sticker`；带 `emoji_id` 的（NapCat 把商城表情发成 `image`）记成 `sticker`，`name` 取 `summary` |
   | `mface` | `sticker`，`name` 取 `summary` |
   | `face`（QQ 的小黄脸） | `sticker`，`name` 取 `raw.faceText`（有的话） |
   | `record` | `voice` |
   | `video` | `video` |
   | `file` | `file`，`name` 取 `name`、`file_name`、`file` 里头一个有的 |
   | `forward`（合并转发） | 正文里写占位 `[forward]`，不展开 |
   | `json`、`xml`（卡片） | 正文里写占位 `[card]` |
   | 别的（`poke`、`dice`、`rps`、`markdown`……） | 不记 |

   带的东西的平台编号取 `data` 里 `file_id`、`file`、`id`、`emoji_id` 头一个有的（字，或者写成十进制的整数），一个都没有的那一样不记。不下载（随平台工具那一步）。`message` 是字符串（CQ 码）的照第 6 条只读字，@、引用、带的东西都不认（「施工时定的」第 62 条）。
3. **找会话**：先把场所规则套到这个群上（`Loaded::at`；隔 `rules_check_millis` 才看一眼系统的变没变，「场所规则和出厂数据」第 3、5 条）。`venue.session {venue: "qq:group:<群号>", kind: "group", persona?, preset?, cwd?}`：`persona`、`preset` 照规则的同名项，`cwd` 照 `workspace`，没设的不写。场所编号经群聊内核拼（`Venue::new("qq", VenueKind::Group, <群号>)`，桥里是 `onebot::venue`），拼不出的同第 7 条。群的会话归系统账号，回应的 `account` 本来就是桥自己的账号：群里不照第 7 条认陌生人。回 `unknown_persona`、`unknown_preset`、`preset_invalid`：这一条不记，同一个群只记一行运行日志 `WARN group not recorded, fix the venue rules venue=… reason=…`（桥起来以后，同私聊的陌生人，「施工时定的」第 66 条）；别的原因同第 7 条。会话编号记进「场所 → 会话」（第 7 条），会话不在了同第 7 条忘掉、再找一次。找到了从头订阅（O-23，「群里怎么叫她」第 1 条；O-22 不订阅，「施工时定的」第 64 条）。
4. **发的人**：`as: {external: "qq:<user_id>", role}`：`role` 照规则的 `managers` 认，这个人的平台身份在里面的是 `manager`，别的 `member`；QQ 的群主、管理员不自动算（`managers` 管的是谁能用管理命令，和群管理员是两回事，「施工时定的」第 58 条）。主人由核心照对应表认（记成外部身份带 `account`），桥不另报。
5. **正文**：照段的先后接起来。`@名字` 的名字照群成员的缓存（`onebot/members.rs`）：按群、按号记 `bridge.json` 的 `member_names_seconds`（出厂 600 秒），过了的不用。每一条群消息的发的人顺手记下（名字同第 6 条的 `name`，没有的不记）；@ 了缓存里没有的人，经收进这条消息的那个机器人号现在的连接调 `get_group_member_info {group_id, user_id, no_cache: false}`，回的 `card` 去掉首尾空白不空的照它、不然 `nickname`，记下。没连着、过了时、失败、回的两个都是空的：写 `@<号>`；这一条里有一个调不成的，后面缓存里没有的不再调，也写号（「施工时定的」第 59 条）。名字照第 6 条洗过。
6. **记**：`session.send {session, text, as, venue}`，命令编号同第 8 条（`qq:<机器人的号>:<消息编号>:<时刻>`）。`venue` 的格（`venues.md`「场所的格」；假的、空的格不写）：
   - `msg`：消息编号写成十进制的字。
   - `name`：`sender.card` 去掉首尾空白不空的照它，不然 `sender.nickname`；去掉控制字符，截到 64 个字符；去掉首尾空白是空的不写。
   - `reply_to`：引用的编号，1 到 128 个字符、没有控制字符的才写。
   - `mentions`（平台身份 `qq:<号>`）、`mentions_me`、`mentions_all`：第 2 条。
   - `media`：第 2 条；编号 1 到 128 个字符、没有控制字符的才记，不合的那一样不记、消息照记；`name` 同 `name` 的洗法，截到 200 个字符（「施工时定的」第 63 条）。
   - `ambient`：一律真。
   - `asleep`：此刻落在这个群的 `sleep` 规则里的真。照群聊内核的回合闸问（`gate()`，只交睡眠：闸说推迟就是睡着），时区照本机此刻的偏移（「施工时定的」第 65 条）。
   - `show_ids`：规则的 `show_ids` 是真的才写。

   正文去掉首尾空白是空的、又没有带的东西的，不送（记一行调试日志 `nothing to send`）。只有带的东西的照样交，`text` 是空的字：核心 O-13 补以后收它，记成没有内容块的 `message.user`（`venue.media` 照记）；桥没有为它改（「施工时定的」第 67 条）。成了记一行 `INFO message sent in venue=… message=… chars=…`，和私聊一样，不记原文。
7. **斜杠命令**：正文去掉开头的空白以后以 `/` 开头的，照「斜杠命令」交 `command.run {session, text, as}`（`as` 带 `role`，不带 `venue`）；回执、被拒的那一句经 `send_group_msg {group_id, message}` 发回这个群，走收进它的那个机器人号现在的连接（同第 10 条：入队，空的不发，没连着、她被禁言的排着），发出去 3 秒后撤回（O-25 上，「斜杠命令」第 7 条）；认不出的照第 6 条记成旁听。

**撤回**（O-22）：`post_type = notice`、`notice_type = group_recall`（群，带 `group_id`、`user_id`、`operator_id`、`message_id`）、`friend_recall`（私聊，带 `user_id`、`message_id`）。记 `events.append {session, kind: "venue.recalled", body: {msg: "<message_id>", by: "qq:<撤的人>"}}`：撤的人群里是 `operator_id`，私聊是 `user_id`。会话照「群消息」第 3 条（群）、第 7 条（私聊：陌生人的照旧不接）找；会话不在了同第 7 条再找一次。命令编号自己编（同 `venue.session`）：平台不重发通知（「施工时定的」第 70 条）。成了记 `INFO recall noted venue=… message=…`，被拒的 `WARN recall refused venue=… message=… reason=…`。

**群里怎么叫她**（O-23 上，2026-10-09；施工单「要定的」四条照推荐定：主人照核心记下的 `by` 认、只有主线、当没被禁言、要问判官的两条路只记判断）：群消息记下以后（「群消息」第 6 条），照群聊内核判这一条怎么办，该开回合的经 `session.respond` 开（`venues.md`「照记下的几条开一轮」），她的回话发回群里。这一步只走不用判官的路：没条件的只记下，主人冲她来的直接回（Q4：只有主人的 @ 不过判官）；要问判官的两条路只记一笔判断、不回（判官随 O-23 下）。不占编号（「施工时定的」第 57 条）。

O-23 下（2026-10-09；施工单「要定的」三条照推荐定：放下在判的请求不真的取消、回来的回答丢掉，判官的全局并发和排队等多久放 `bridge.json`，违规时给她看的那句预检结论这一步不做）接上判官：线路规程四种分开走（第 10 条），同一个人在顶替窗口里补发的照群聊内核顶替（第 11 条），要问判官的经核心的 `model.call` 问（第 12 条）、读回答、算分（第 13 条），额度满了不抽样、不问判官（第 14 条）；`ext.onebot.chat.decided` 记全（第 7 条）。核心的 `model.call` 在后台答（施工 8-20 补，`protocol.md` 的 `model.call` 第 2 条）：判官一次要几十秒，这段时间桥别的请求不等它，她的话、新的群消息照常走（「施工时定的」第 86 条）。

O-25 上（2026-10-09；施工单「要定的」三条照推荐定：一轮几条触发的引用最后一条，引用和 @ 只带在第一段，丢了的这一步只记运行日志）接上出站链：她的话先过群聊内核的出站链（清理、去重、引用和 @），过了的照纯文本拆段、第一段带上引用和 @（第 9 条；私聊同「怎么走」第 10 条）；群里的命令回执发出 3 秒后撤回（「斜杠命令」第 7 条）。

O-25 中（2026-10-09；施工单「要定的」六条照推荐定：排着的 60 秒过期、放 `bridge.json`，全员禁言不认，桥重启时入队了没结局的不补发，私聊不记 `venue.delivered`，去重入队记成了就算上，禁言的复查随 quirk 那一步）接上出站队列：她的话、提示、回执都先入队再发，失败、过期的记下；她被禁言时这个群的出站排着，进站链照投影算禁言（第 2、5、7、9 条，「出站队列」）。

1. **订阅群会话**：桥起来以后头一次找到一个群的会话（「群消息」第 3 条），订阅它的事件流，写 `after: 0`：核心先把日志从头补过来，再接着推新的（`protocol.md`「补发」）。补来的、推来的走同一条路收进这个群的投影（第 2 条）；回应的 `upto` 记下：序号不大于它的是从前的，她的话不再发（桥重启不会把以前的回复再发一遍，同第 9 条）。订阅不上的记一行 `WARN not subscribed`，这一条不记，下一条再找（同第 7 条）。掉了队（`resync`）照收到的最后一条的序号再订阅（`after` 写它），补来的照样收进投影、她的话不发（不补发，「出站队列」第 6 条）。会话不在了（第 7 条）连投影一起忘掉，再找、再订阅。回应的 `persona`（这个群会话用的人格，照日志第一条 `session.created`，`protocol.md` 的 `subscribe`；没有这一格的是无人格）记下，判官照它带人格（第 12 条第 3 款，O-23 补）。
2. **从日志投影**（`chat.md` 第七条第 4 条那张表）：内存里只放从日志算得出的，桥重启照第 1 条重建。推来的事件照序号收，重的、更早的不收。
   - 人说的话（`message.user`）：序号 → 发的人（`by.id`）、是不是主人（`by` 是外部身份、带 `account`）。
   - 开过的回合（`turn.started`）：开始的时刻（事件的 `at`），`triggers` 每一条的发的人。
   - 主线在不在跑、这一轮回的是谁：`turn.started` 开了一轮，回的人是它 `triggers` 的发的人；`turn.joined` 带同一个回合编号的，并进它的 `triggers` 的发的人（不重）；`turn.ended` 这一轮完了。并进去的那一轮没再请求就结束的，核心接着开一轮，它的 `turn.started` 没有 `triggers`、`trigger` 是那条 `turn.joined` 的序号：照那条的 `triggers` 找回发的人，回合的触发的人（限流）、这一轮回的人（`venue.delivered` 的 `to`、续聊）都照它（O-23 下，「施工时定的」第 101 条）。
   - 她的回复（`venue.delivered`）：同一条线、同一轮的几条并成一笔（`chat.md` 第三条的 `Reply`）：时刻取第一条的 `at`，回的人取并集。每一条的平台编号 `msg` 记下（第 4 条认「引用她」）。
   - 限流提示过的时刻：`ext.onebot.venues.queued` 里 `kind` 是 `notice`、`reason` 是 `rate_limited` 的那几条的 `at`。
   - 出站链要的（O-25 上，第 9 条）：每条人说的话另记平台编号（`venue.msg`）；主线这一轮她回的那一条：`turn.started` 的 `triggers` 的最后一条，`turn.joined` 并进来的换成它的最后一条（核心接着开的一轮照找回的那几条，同上），和核心记 `tool.call` 的 `by` 一个取法（`providers.md`「是谁要的」）；这一轮入队了的她的话的正文（O-25 中：`ext.onebot.venues.queued` 里 `kind` 是 `reply` 的那几段，照 `turn` 认是哪一轮；换了回合就清，晚到的上一轮的不算。桥入队记成了就先算进去，日志推来的同一段照正文认：这一轮已经有一样正文的不再加；桥重启照日志补，「出站队列」第 2 条）；最后一条人说的话、最后一条 `venue.delivered` 的序号。
   - 她被禁言到什么时候（O-25 中，「出站队列」第 7 条）：最后一条 `ext.onebot.venues.muted` 的 `until`；之后有 `unmuted` 的是没被禁言。
   - 别的不看。
3. **发的人是谁**（`Standing`）：照核心记下的这一条的 `by` 认（`chat.md` 第七条第 4 条）：群里外部身份带 `account` 的是主人；不是主人、平台身份在 `onebot.trusted`（握手交来的配置，推来新的就换、记一行 `INFO trusted changed count=…`；字的列表，不是列表的当空的，列表里不是字的不要）里的是自己人；别的是别人。核心记下的那一条在 `session.send` 的回应以前就推到了（「先见结果，后见回应」）：桥判以前先把这个会话留着没办的推送收进投影（第 1、9 条那条路，她新说的话照样发），投影就到了这一刻。`session.send` 被拒的没有序号，不判；只有带的东西的照样有序号、照样判（`media_only`，第 6 条）。
4. **冲她来**（群聊内核的 `addressed`）：@ 了她（`mentions_me`）；引用的是她的消息（`reply_to` 在第 2 条记下的平台编号里）；正文（交给核心的那一份，@ 已经写成名字）去掉开头的空白以后，以场所规则 `keywords` 里的某一个开头。
5. **过进站链**（`Chain::builtin`）：`Inbound` 是群、`Said`（发的人、第 3 条、第 4 条）、正文；`Ctx` 的 `rate`、`sleep`、`allow` 照套到这个群上的场所规则（`Rate::read`、`Sleep::read`，没设的是空的），`turns` 是投影里触发的人全是主人或自己人（照判的这一刻的 `onebot.trusted`）以外的回合的开始时刻（没有 `triggers` 的照算），`notices` 照投影，`muted` 照投影：她被禁言到的时刻晚于此刻（O-25 中；O-23 上写死是假），`moderation` 是用着的违规词表和这个群的 `Params::base64`；此刻照本机的钟，时区照本机此刻的偏移（同「群消息」第 6 条的 `asleep`）。`RecordOnly` 的只记下；`Notice`（限流满了冲她来的）回一句提示（第 7 条）；`Pass` 往下走。
6. **加值项和走哪条路**：`Facts`（场所、序号、`Said`、@ 了别人没有、引用的是不是别人的消息、只有表情、只有带的东西）、进站链插的旗、投影里她的回复、此刻、这个群的 `Params::chatty` 交 `conditions`；再照这个群的线路规程留下算数的条件、定走哪条路（第 10 条，O-23 下），看不看顶替也照它（第 11 条）。`Record` 只记下；`Commit` 开一轮；`Judge`、`ModerationOnly` 问判官（第 12、13 条），额度满了的不问（第 14 条）。只有表情（`textless`）：正文去掉首尾空白是空的、带的东西都是表情；只有带的东西（`media_only`）：正文去掉首尾空白是空的、带了东西。不看 `parallel`：只有主线，不调 `dispatch`（施工单 O-23 上「要定的」第 2 条，「施工时定的」第 74 条；`discipline` O-23 下看了，第 75 条作废）。
7. **记判断、做**：每判一条先记 `events.append {session, kind: "ext.onebot.chat.decided", body}`（命令编号是这条消息的命令编号加 `/decided`；几条一起判的照最后一条的拼，第 11 条），再做（`chat.md` 第七条第 4 条：桥保证先记判断、再开回合）。要问判官的等判官回来、算完分才记（第 13 条）：
   - 回：`session.respond {session, to: 判的那几条的序号}`（命令编号加 `/respond`），不带事实；正在跑一轮的核心并进去（`turn.joined`）。成了记一行 `INFO respond asked venue=… message=…`，主触发是冲她来、续聊的贴表情（O-25 下，「贴表情」）；回 `already_answered`、`not_ambient` 的不再开，记一行 `INFO respond refused venue=… message=… reason=… messages=…`（`data.messages` 是那几条）；别的拒绝记 `WARN`。`events.append` 被拒的（照说不会）记 `WARN event not appended session=… kind=… reason=…`。
   - 提示：把 `group/rate-limited` 那一句（照握手回的语言）入队（「出站队列」第 2 条：`body` 是 `{kind: "notice", reason: "rate_limited", text}`，命令编号加 `/queued`），照先后经 `send_group_msg` 发回这个群。
   - 只记下：不再做什么。

   `body` 的格（「施工时定的」第 76 条，O-23 下多 `discipline`、`supersede`、`judge`、`score`，第 97 条）：

   | 格 | 什么时候有 | 是什么 |
   |---|---|---|
   | `msgs` | 都有 | 判的是哪几条：序号的列表，照先后；顶替重判的几条一起（第 11 条） |
   | `standing` | 都有 | 第 3 条：`owner`、`trusted`、`member` |
   | `inbound` | 都有 | 进站链的结果：`pass`、`record_only`、`notice` |
   | `why` | `record_only`、`notice` | `asleep`、`muted`、`not_allowed`、`rate_limited` |
   | `flags` | 插了旗的 | 进站链插的旗，照先后：`moderation` |
   | `discipline` | `pass` | 线路规程：`chatty`、`when-called`、`wake`、`every-message`（第 10 条） |
   | `conditions` | `pass` | 算数的条件（照线路规程留下的，第 10 条），照插槽的先后，每个 `{kind, bonus}`；`kind` 是 `direct`、`continuation`、`after_speaking`、`moderation`、`probability`。顶替的是合起来的（第 11 条）。一个都没有是空的列表 |
   | `supersede` | 顶替了的 | `{"inherit": 序号}`：接过那一条，不再判；`{"rejudge": 序号}`：放下那一条在判的，几条一起重判（第 11 条） |
   | `route` | `pass` | 走的路：`record`、`commit`、`moderation_only`、`judge`；接过去的是 `commit` |
   | `judge` | 走 `judge`、`moderation_only` 的 | 问判官的那一次：`mode`（`reply`、`moderation_only`）；问了的另有 `tries`（问了几次）、`millis`（从交给判官到有结果的毫秒数，排队也算）、`model`（回答的那一家和模型，`<供应商>/<模型>`；一次都没回的没有）；读出来了的是 `answer`：`scores`（五维，照相关、意愿、社交、时机、连贯）、`should_reply`、`to_bot`、`severity`（没查的没有）、`reason`；判不了、没问的是 `unjudged`：`rate_full`（额度满了，没问，第 14 条）、`queue`（排队等不到）、`timeout`（等不到回答）、`refused`（核心拒了，`detail` 是原因码）、`unreadable`（读不出，`detail` 是 `no_object`、`no_severity`、`dimension:<五维的名字>`） |
   | `score` | 判官的回答读出来了的 | 算分的每一项：`raw`、`adjust`、`bonus`、`lift`、`threshold`、`total`、`reply`（`chat.md` 第三条的 `Score`；只查违规的数都是 0，`reply` 只看违规） |
   | `outcome` | 都有 | 结论：`reply`（回）、`record`（只记下，判不了的也是）、`notice`（回一句提示） |

   例子：主人 @ 她，`{"msgs": [12], "standing": "owner", "inbound": "pass", "discipline": "chatty", "conditions": [{"kind": "direct", "bonus": 0.3}], "route": "commit", "outcome": "reply"}`；限流满了别人第二次叫她，`{"msgs": [15], "standing": "member", "inbound": "record_only", "why": "rate_limited", "outcome": "record"}`；别人 @ 她、判官说回，`{"msgs": [20], "standing": "member", "inbound": "pass", "discipline": "chatty", "conditions": [{"kind": "direct", "bonus": 0.3}], "route": "judge", "judge": {"mode": "reply", "tries": 1, "millis": 812, "model": "deepseek/deepseek-flash", "answer": {"scores": [8, 7, 5, 6, 7], "should_reply": true, "to_bot": true, "severity": 0, "reason": "…"}}, "score": {"raw": 0.68, "adjust": 0.2, "bonus": 0.3, "lift": 0.0, "threshold": 0.8, "total": 1.18, "reply": true}, "outcome": "reply"}`；判官两次都回得读不出，`"judge": {"mode": "reply", "tries": 2, "millis": 2410, "model": "deepseek/deepseek-flash", "unjudged": "unreadable", "detail": "no_object"}`、`"outcome": "record"`。运行日志每判一条记 `INFO chat decided venue=… message=… outcome=…`，判不了的另记 `INFO not judged venue=… message=… why=…`；判官的理由、消息的原文不进运行日志。
8. **重发的不再判**：`session.send` 回的序号在收这个会话留着的推送以前就已经在投影里了（平台重发、桥重启以后 NapCat 又推了一遍）：不判、不记，一条消息只判一次（「施工时定的」第 78 条）。
9. **她的回话发回群里**：群会话推来的 `message.assistant`，序号大于第 1 条的 `upto` 的，取 `text` 块依次接起来（同「怎么走」第 10 条），先过出站链、再发（O-25 上；O-23 上照纯文本拆段就发）。
   1. **过出站链**：`OutChain::builtin().judge(outgoing, &ctx)`（`chat.md` 第五条）。`outgoing` 是 `{text, images: []}`（她的图随后面的步子）。`ctx` 照第 2 条的投影填：
      - `sent`：这一回合已经入队的：投影里这一轮入队了的她的话的正文（第 2 条；O-25 中，「施工时定的」第 112 条）。
      - `target`：本来想要引用她回的那一条、@ 发它的人（第 2 条：这一轮几条触发的照最后一条，施工单「要定的」第 1 条）；那一条没有平台编号的不引用，发的人解不出号的不 @；这一轮没有她回的那一条的（回报、定时开的一轮）两样都是假。
      - `since`：`others` 是她回的那一条以后（照序号）来的人说的话里不是那个人说的有几条（她的话不是 `message.user`，本来就不算）；`elapsed` 是本机此刻减那一条的 `at`，毫秒；`last_is_own` 是她发出去的最后一段的序号比最后一条人说的话的大：她的一段照她说它的那条 `message.assistant` 的序号（照 `venue.delivered` 的 `turn` 认是哪一轮说的），这个群的日志里没有那一轮她说的话的（主线发来的）照 `venue.delivered` 自己的序号。没有她回的那一条的，`others`、`elapsed` 是 0。
      - `outbound`：这个群此刻套出来的 `Params::outbound`（`quote_after`、`mention_after`、`min_bigrams`、`similar`、清理的名单，`chat.md` 第八条）。
   2. **丢了的**（`Out::Drop(why)`）：不发，运行日志记一行 `INFO reply dropped venue=… why=… chars=…`（`why` 是 `leaked`、`blank`、`aside`、`repeated`），不记原文；不入队、不另记事件（「施工时定的」第 107 条）。
   3. **发的**（`Out::Send { outgoing, target }`）：`outgoing.text` 照群聊内核的 `plain` 转成纯文本、`split` 照这个群的 `Params::split_chars` 拆开，一段一条入队（「出站队列」，O-25 中），照先后 `send_group_msg` 放进收进这个群的那个机器人号现在的连接的写队列（没连着、她被禁言的排着；拆出来是空的不发）。实际要带引用、@ 的，只带在第一段（施工单「要定的」第 2 条）：`message` 是 `[{type: "reply", data: {id: "<那一条的平台编号>"}}, {type: "at", data: {qq: "<号>"}}, {type: "text", data: {text: " "}}, {type: "text", data: {text: <这一段>}}]`，不带的那一样不写它的段（空格那一段跟着 @）；后面几段只有字（「施工时定的」第 108、109 条）。
   4. **记**：NapCat 回了成功的，照放进写队列的先后（不照 NapCat 回的先后，「施工时定的」第 84 条）记 `events.append {kind: "venue.delivered", body: {line, turn, to, msg, text, images: []}}`：`line` 是这个群的会话编号（主线），`turn` 是这条回复的回合编号，`to` 是这一轮回的人（第 2 条），`msg` 是 NapCat 回的 `message_id` 写成十进制的字，`text` 是这一段（不带引用、@）；命令编号自己编。回的里没有 `message_id` 的记一行 `WARN delivered without a message id`，不记。核心照 `venue.delivered` 认出哪些话不重复渲染（`venues.md`「桥记的事件」）。失败的记 `ext.onebot.venues.failed`（「出站队列」第 4 条）。
10. **线路规程**（O-23 下；场所规则的 `discipline`，18 第七节那张表）：照套到这个群上的规则，没设的照 `chatty`（「施工时定的」第 88 条）。

    | 线路规程 | 算数的条件 | 顶替（第 11 条） | 走哪条路 |
    |---|---|---|---|
    | `chatty` | 全部（额度满了的去掉抽样，第 14 条） | 看 | 群聊内核的 `route`：没条件只记下，主人冲她来开一轮，只有违规旗判官只查违规，别的判官打分 |
    | `when-called` | 冲她来、续聊、违规旗 | 看 | 只有违规旗的判官只查违规（`moderation_only`），严重程度够了才回；别的有条件的开一轮；没有的只记下 |
    | `wake` | 冲她来、续聊、违规旗：「回复以后一小段时间内不用再叫」照续聊算，她刚回过的人在续聊的窗口里接着说（「施工时定的」第 89 条） | 不看 | 同 `when-called` |
    | `every-message` | 冲她来、续聊、刚说过话、违规旗（只记进判断，不定走哪条路） | 不看 | 有字的开一轮（违规旗不改它）；只有带的东西（`media_only`）的只记下 |

    不是 `chatty` 的不抽样、不打分。违规关键词只能把判官拉起来，不能直接定违规（18 第七节「违规审核」）：`when-called`、`wake` 里只有违规旗的照 `chatty` 的只查违规问判官，和别的条件一起的照原来的走（冲她来照开）；`every-message` 有字的本来就回，违规旗不改它（「施工时定的」第 90 条）。给她看的那句预检结论另起一小步（施工单「要定的」第 3 条）。私聊不走这里：每一条都开回合（「怎么走」第 6 到 8 条），`discipline` 写什么都一样。
11. **顶替**（O-23 下；群聊内核的 `supersede`，`chat.md` 第四条）：线路规程看顶替的，拿这一条的 `Facts`、算数的条件、这个群还没回完的（`Pending`）、此刻、这个群的 `Params::supersede_window`（出厂 7 秒）问它。
    - **还没回完的有两种**。`Committed` 照日志投影（`chat.md` 第七条第 4 条）：判断的 `outcome` 是 `reply` 的那几条（`msgs`：最后一条是 `msg`，前面的是 `absorbed`），发的人、时刻是最后一条 `message.user` 的 `by`、`at`，条件照判断的 `conditions`；收了它的那一轮（`turn.started.triggers`、`turn.joined.triggers` 里有它）`turn.ended` 了就回完了，还没进哪一轮的照旧算（「施工时定的」第 91 条）。`Judging` 只在桥的内存里：交给判官时记下，判官回来、被放下时拿掉；桥重启就丢，不补判（窗口只有 7 秒）。
    - `None`：照第 10 条自己走。
    - `Inherit`：不再判。记判断（`supersede: {inherit: 那一条}`、合起来的条件、`route: commit`、`outcome: reply`），照第 7 条 `session.respond {to: [这一条]}`：她收了那一条的这一轮还在跑的，核心并进去（`turn.joined`），下一步就听到。
    - `Rejudge`：放下那一条在判的（桥这边拿掉它，那一次判官请求不取消，回来的回答丢掉，施工单「要定的」第 1 条），几条一起照合起来的条件走路（第 10 条）：要问判官的一起问（第 12 条，`Judging` 的 `msg` 是这一条、`absorbed` 是前面几条），开一轮的 `session.respond {to: 这几条}`；记判断时 `msgs` 是这几条、`supersede: {rejudge: 放下的那一条}`。
12. **问判官**（O-23 下）：交给一个另起的任务（`judges.rs`、`ask.rs`），跟核心的那一头接着办别的消息；任务经并着发的调用口（`core/caller.rs`）调核心，不等那一头手上的事。
    1. **排队**：全局最多同时问 `bridge.json` 的 `judge_concurrency` 个（出厂 4），名额满了排队，最多等 `judge_queue_seconds`（出厂 15 秒），等不到的当判不了（`unjudged: queue`），不重试。一个名额占到这一次问完：重试接着占着；被放下的那一次照样等到回答才放（「施工时定的」第 95 条）。
    2. **群聊记录**：`venue.records {session, msg: 判的最后一条, count}`，`count` 是这个群的 `Params::judge.records`（1 到 100，和核心收的一样，`chat.md` 第八条，「施工时定的」第 93 条）。被拒的当判不了（`refused`，`detail` 是原因码），不重试。几条一起判的，前面几条在记录里（「施工时定的」第 92 条）。
    3. **拼请求**：`Ask { persona, records, current, decoded, mode }`。`persona` 是这个群会话所用的人格的说明（O-23 补，2026-10-09 项目主人定：判官也带人格，给一个开关，默认开）：
       - 哪一个人格：第 1 条记下的订阅回应的 `persona`；没有的（无人格）不带。这个群的 `Params::judge.persona` 是假的（场所规则写 `judge = { persona = false }`，`chat.md` 第八条）也不带、不读。不带的是 `None`，那一段整个不出现。
       - 原文：拿到群聊记录以后读 `persona.read {"persona": <编号>, "prompt": "persona"}` 的 `text`：属主那几层叠好的、现在文件里的那一份，不是会话快照里冻着的（施工单「不做什么」第 1 条）。`text` 是 `null` 或空的（这个人格没写人设）不带。
       - 读到的记 `bridge.json` 的 `judge_persona_seconds`（出厂 60 秒），几个问判官的任务共用：这段时间里同一个人格不再读，人格改了判官最多晚这么久看到（「施工时定的」第 103 条）。
       - 读不到的（核心拒了：人格删了、写错了、读出错）：这一次不带，运行日志记一行 `WARN persona not read`（`session`、`persona`、`reason`），照样问判官；不记下，下一次再读。核心断开了的，这一次问判官的任务交回空的（同 `venue.records`）。

       `decoded` 是判的几条的正文用换行接起来交这个群的 `Params::base64` 的 `Base64::reveal`；`mode` 照走的路（`judge` 是 `Reply`，`moderation_only` 是 `ModerationOnly`）。`request(&判官的说明, &ask, &这个群的 Params::chatty)` 拼成两条，写成 `messages: [{role: "system", text}, {role: "user", text}]`。
    4. **调**：`model.call {purpose: "judge", model?, max_tokens, messages}`：`model` 照 `Params::judge.model`，没写的不带（核心照 `models.chat`）；`max_tokens` 照 `Params::judge.max_tokens`。每一次最多等 `Params::judge.timeout`（只查违规的 `moderation_timeout`），等不到的不再等（核心那边照样做完，回来的回应丢掉）。判不了的（等不到、核心拒了、读不出，第 13 条）再问，最多再问 `Params::judge.retries` 次，记进判断的是最后一次的为什么（「施工时定的」第 94 条）。
    5. **判官的说明**：资源 `software/onebot/judge/` 的十三份，桥起来时和出厂数据一起读、查过（「场所规则和出厂数据」第 2 条）。
13. **读回答、算分**（O-23 下）：回应的 `text` 交 `read(text, mode, Params::judge.reason_chars)`，读不出的（`Unreadable`）当判不了。判官回来了，被放下的（第 11 条）不再理；没被放下的先把这个会话留着的推送收进投影（同第 3 条），读出来了的拿合起来的条件、投影里她的回复、判官回来的这一刻（冷静照这时的近期发言量）、问判官时套的那一份 `Params::chatty` 交 `score`：`reply` 是真的回，假的只记下（第 7 条）。判不了的照不回算，只记下（18 第七节），`judge.unjudged` 写为什么（「施工时定的」第 100 条）。
14. **额度满了**（O-23 下；群聊内核的 `rate_full`，`Ctx` 同第 5 条）：`chatty` 的群这段时间不抽样（条件里去掉抽样）、不问判官：要问判官的只记下，`judge` 只有 `mode` 和 `unjudged: rate_full`。进站链已经把别人挡下了，走到这里的是主人和自己人：主人冲她来的不过判官、照回，自己人冲她来的这段时间只记下（「施工时定的」第 98 条）。

**出站队列**（O-25 中，2026-10-09；施工单「要定的」六条照推荐定：排着的 60 秒过期、放 `bridge.json`，全员禁言不认，桥重启时入队了没结局的不补发，私聊不记 `venue.delivered`，去重入队记成了就算上，禁言的复查随 quirk 那一步；18 第十节、Q15）：她要说出去的一切先记账再发，像邮件系统的队列。排着、过期、禁言到期、回应算成什么是纯逻辑（`route/queue.rs`），入队、交出去、记结局的在 `route/sending.rs`，禁言、解禁在 `route/muted.rs`（「施工时定的」第 116 条）。不占编号（「施工时定的」第 57 条）。

1. **什么要入队**：群里她的话（「群里怎么叫她」第 9 条：过了链、拆好的每一段）、限流的提示（第 7 条）、命令回执和被拒的那一句（「斜杠命令」第 2、4 条，群里、私聊的都是）；私聊她的回话（「怎么走」第 10 条：拆好的每一段）。出站链丢了的不入队（「施工时定的」第 107 条）；回执、提示去掉首尾空白，空的不入队。
2. **入队**：`events.append {session, kind: "ext.onebot.venues.queued", body}`，命令编号：提示照「群里怎么叫她」第 7 条加 `/queued`，别的自己编。`body`：

   | 格 | 什么时候有 | 是什么 |
   |---|---|---|
   | `kind` | 都有 | `reply`（她的话）、`notice`（提示）、`receipt`（回执、被拒的那一句） |
   | `text` | 都有 | 要发的这一段（不带引用、@） |
   | `line`、`turn` | `reply` | 这个场所会话的编号（主线）、她说这一段的回合编号 |
   | `reason` | `notice` | `rate_limited` |

   例子：`{"kind": "reply", "text": "看完了。", "line": "01a0d78c-…", "turn": 42}`、`{"kind": "notice", "reason": "rate_limited", "text": "这会儿叫的人太多了，过几分钟再来吧。"}`、`{"kind": "receipt", "text": "已全部停下。"}`。记成了（回应交回序号）才往下走：她的话这时就算进这一回合已经入队的（群里先算进投影，私聊记进桥内存里的 `Spoken`；去重照它，「群里怎么叫她」第 2、9 条，「怎么走」第 10 条，「施工时定的」第 112、113 条）。核心拒了的（照说不会）记一行 `WARN event not appended`，不发；核心断开了照第 11 条桥停下。本机的钟读不出的（照说不会）记一行 `WARN clock not readable, reply dropped`，不入队。
3. **发**：一个场所会话一条队，先进先出。门开着（这个群没被禁言，第 7 条；收进这个会话的那个机器人号连着）的照入队的先后交 NapCat：一段一段放进那个号现在的连接的写队列，前一段放进去了才放下一段，等回应交给别的任务（「施工时定的」第 10、84 条）。门关着的排着，运行日志记一行 `INFO reply waiting venue=… why=muted|disconnected chars=…`。什么时候再看一遍排着的：入队以后；机器人号连上了（`Event::Connected`，「怎么走」第 2 条）；记下禁言、解禁以后；到了最早的过期时刻、禁言到期的时刻（第 8 条）。看的时候先把过期的记了，再照先后交门开着的（「施工时定的」第 123、125 条）。
4. **结局**：照放进写队列的先后交回来（第 84 条）。NapCat 回了成功的，运行日志一行 `INFO reply sent venue=… chars=…`；群里她的话记 `venue.delivered`（「群里怎么叫她」第 9 条第 4 项），群里的回执回了编号的过几秒撤回（「斜杠命令」第 7 条），提示、私聊的不再记。没成的记 `events.append {kind: "ext.onebot.venues.failed", body: {queued, why, detail?}}`（命令编号自己编；`queued` 是入队那一条的序号），运行日志一行 `WARN reply not sent venue=… why=… chars=…`：

   | `why` | 什么时候 |
   |---|---|
   | `rejected` | NapCat 回了，`status` 不是 `ok`：`detail` 是它的 `message` 去掉首尾空白、截到 200 个字符，空的不写 |
   | `timeout` | 等了 `call_timeout_seconds` 还没回 |
   | `disconnected` | 写不进（那个号的连接正在断）、等的时候连接断了 |
   | `expired` | 排着，过了期限（第 5 条） |

   例子：`{"queued": 57, "why": "rejected", "detail": "发送失败"}`、`{"queued": 61, "why": "expired"}`（「施工时定的」第 120 条）。她的话没成的，记了 `failed` 再退信（O-25 下，「退信」）。
5. **过期**：入队以后过了 `bridge.json` 的 `queue_expire_seconds`（出厂 60）还排着的作废，记 `expired`：不会在解禁、连上以后一股脑发出去（18 第十节）。入队的时刻加期限不晚于此刻就算过了。限流的提示同样过期（过了期的提示不再提示）。时刻照本机的钟（「施工时定的」第 118 条）。回执的撤回从 NapCat 回了编号起算，不变。
6. **桥重启**：内存里排着的丢了；日志里入队了、没有结局的，当没发出去，不补发、不另记（施工单「要定的」第 3 条：可能已经发出去了、只是回执没记下，补发会说两遍）。群里这一回合入队过的，照日志重建投影时照样算进这一回合（「群里怎么叫她」第 1、2 条）；私聊的 `Spoken` 跟着丢。
7. **她被禁言**：NapCat 推来 `post_type = notice`、`notice_type = group_ban`（带 `self_id`、`group_id`、`user_id`、`sub_type`、`duration`，号的读法同「怎么走」第 5 条），`user_id` 等于 `self_id`（禁的是她）的才认；全员禁言（`user_id` 是 0）、禁别人的不认（施工单「要定的」第 2 条），记一行调试日志就丢（同第 5 条）。`sub_type = ban` 带 `duration`（秒）大于 0 的是禁言；`lift_ban`、`duration` 是 0 的是解禁；没带 `duration`、是负的、别的 `sub_type` 不认（「施工时定的」第 121 条）。
   - 禁言记 `events.append {session, kind: "ext.onebot.venues.muted", body: {until}}`：`until` 是收到通知时本机此刻加 `duration` 秒，写法同事件的 `at`（`miyu_kernel::time::Timestamp`，「施工时定的」第 122 条），算出来超出范围的不记（记一行 `WARN mute not noted`）；解禁记 `ext.onebot.venues.unmuted`，`body` 是 `{}`。会话照「群消息」第 3 条找，会话不在了同「怎么走」第 7 条再找一次；命令编号自己编（平台不重发通知，同「施工时定的」第 70 条）。成了记 `INFO mute noted venue=… seconds=…`、`INFO unmute noted venue=…`；被拒的 `WARN mute refused venue=… reason=…`。记下以后把这个会话留着的推送收进投影（「群里怎么叫她」第 2 条），再看一遍排着的（第 3 条）。
   - 她被禁言着（投影里 `until` 晚于此刻，第 2 条）：这个群的出站排着；进站链的 `Ctx.muted` 是真（「群里怎么叫她」第 5 条：谁说的都只记下，主人 @ 也是）。解禁的通知来了、`until` 到了，照先后发；`until` 到了不另记 `unmuted`（`until` 是绝对的时刻，照日志算得出）。
   - 只照通知：NapCat 查禁言用的成员缓存会说错（18 第十三节），复查随 quirk 那一步（施工单「要定的」第 6 条）；错过了解禁的通知，最多等到禁言本来到期。
8. **定时**：跟核心的那一头等推送、等 NapCat 的消息的同时，睡到最早的过期时刻或者禁言到期的时刻（有东西排着的会话里最早的那一个；没有排着的不睡），醒了照第 3 条再看一遍：过期的记 `expired`，门开了的发出去。
9. **不在这一步的**：积压几条进 `miyu onebot status` 随状态页那一步；定时消息（带「不早于」入队）随定时的那一步。退信、贴表情是 O-25 下，见下面两段。

**退信**（O-25 下，2026-10-09；施工单「要定的」第 1、2 条照推荐定：一段一块、带那一段的开头；18 第十节「发不出去就退回给她，像退信」、Q15）：她的话没发出去（NapCat 拒了、等不到、连接断了、排过期了），她自己要知道，不然会以为群里看到了。经核心的 `session.note`（`venues.md`「记几块事实」）给那个会话记一块事实：这一轮还在跑的，她下一次请求看到；不在跑的，下一轮开头看到。接在 `route/sending.rs` 记 `failed` 的后面（「施工时定的」第 129 条）。不占编号。

1. **退哪些**：只退她的话（`queued` 的 `kind` 是 `reply`，群里的、私聊的都是）；提示、回执失败了不退：那是桥、核心写的，不是她说的。
2. **怎么退**：记了 `failed` 以后（「出站队列」第 4 条，四种 `why` 都算），`session.note {session, facts: [{kind: "undelivered", text}]}`，命令编号 `<入队那一条的序号>/note`。一段一块（施工单「要定的」第 1 条）：一段只有一个结局，只退一次；同一个命令编号再发核心只算一次。
3. **写什么**：`text` 照资源 `resources/software/onebot/facts/undelivered.txt` 填（给模型看的字，登记在 26 第十节；桥起来时和出厂数据一起读、查过字段，「场所规则和出厂数据」第 2 条）：`why` 是 `failed` 的 `why`；`detail` 是 `failed` 的 `detail`（NapCat 说的原因），没有的填空的；`text` 是她那一段去掉首尾空白以后的头 30 个字符，认得出是哪一句（施工单「要定的」第 2 条，「施工时定的」第 130 条）。字段照内核的模板转义（`Template::render`：没有引号、尖括号）。出厂的样子（排过了期的一段）：

   ```
   <undelivered why="expired" detail="">Your message starting "大家说得都有道理。那就这么定了：周六上午十点在东门集合，别迟" was not sent; the chat never saw it.</undelivered>
   ```

4. **记**：成了记一行 `INFO undelivered noted venue=… why=…`；被拒的（照说不会）记一行 `WARN note refused venue=… reason=…`，不再试；核心断开了照「怎么走」第 11 条桥停下。正文不进运行日志。
5. **不重发**：她看到了自己决定要不要再说（施工单「不做什么」）；她再说一遍是想要的。

**贴表情**（O-25 下，2026-10-09；施工单「要定的」第 3 到 5 条照推荐定：桥重启时贴着的不摘、贴在她要回的那一条、判下来要回才贴；18 第七节那张表的「贴表情」一列，旧版的效果）：群里冲她来、续聊她的那一条，判过要回的，先在那条消息上贴一个表情，让人知道她看到了、在回；她回了、这一轮完了、或者过了一阵还没回，摘掉。在 `route/reaction.rs`（「施工时定的」第 131 条）。不占编号。

1. **什么时候贴**：群消息判下来要回（「群里怎么叫她」第 7 条，结局是回），`session.respond` 成了（开了一轮或者并进一轮；回 `already_answered`、`not_ambient` 的不贴，「施工时定的」第 133 条），主触发是冲她来（`direct`）、续聊（`continuation`）的才贴：主触发照算数的条件取（`Conditions::primary`，`chat.md` 第三条第 7 条），顶替的照合起来的条件取，随前一条（`chat.md` 第四条）。抽样、刚说过话、违规旗的不贴；判官在判的时候不贴（施工单「要定的」第 5 条：判官可能说不回，贴了又摘，看着像她犹豫）；私聊不贴。
   - 贴在她要回的那一条上：判的几条里最后一条，和引用同一条（施工单「要定的」第 4 条，「施工时定的」第 106 条）。
   - 顶替接过去的（`supersede.inherit`）：前一条贴着的当场摘掉，贴到这一条上（旧版同样，「施工时定的」第 134 条）。
2. **怎么贴**：`set_msg_emoji_like {message_id, emoji_id, set: true}`：`message_id` 是那一条的平台编号（`venue.msg`）原样写成字，`emoji_id` 是 `bridge.json` 的 `reaction_emoji`（出厂 `"289"`）；经收进这个群的那个机器人号那时的连接（「施工时定的」第 132 条）。贴、摘是另起的任务，跟核心的那一头不等它。
3. **什么时候摘**（`set: false`，别的同上）：收了那一条的那一轮（推来的 `turn.started`、`turn.joined` 的 `triggers` 有它的那一轮）发出去第一段（推来的 `venue.delivered` 的 `turn` 是它）、那一轮结束（`turn.ended`）、贴了以后过了 `bridge.json` 的 `reaction_seconds`（出厂 600），先到哪个算哪个，只摘一次。贴不上的不摘。并进一轮、那一轮没再请求就结束、核心接着开一轮的，照前一轮结束摘（和判过要回的那一笔一样，「施工时定的」第 91 条）。
4. **记**：不入出站队列、不记事件。贴、摘成了各记一行 `INFO reaction set|removed venue=… message=…`；没成的记一行 `WARN reaction not set|not removed venue=… message=… error=…`（调用的错；那时没连着的照断了记，`Closed`），不再试。桥停下、重启时贴着的不摘（施工单「要定的」第 3 条：不记事件就不知道贴了哪些；贴着一个表情的代价小，重启少见）。

**场所规则和出厂数据**（O-21，2026-10-09；施工单「要定的」三条照推荐定：用的时候看一眼修改时刻、系统的写错了只丢坏的、系统的违规词表放 `system/modules/onebot/`）：写法、怎么套由群聊内核管（`chat.md` 第一条、第八条、第二条第 12 条），读文件是桥的事（群聊内核在第 2 层，不碰磁盘）。不占编号，和「斜杠命令」一样。

1. **在哪**：

   | 什么 | 出厂（资源目录的 `software/onebot/`） | 系统（数据根的 `system/`） |
   |---|---|---|
   | 场所规则 | `venues.d/*.toml` | `venues.d/*.toml`（07 第二节） |
   | 出厂参数 | `defaults.toml` | 没有：要改写场所规则（`chat.md` 第八条「怎么走」第 3 条） |
   | 违规词表 | `moderation.txt` | `modules/onebot/moderation.txt`：在的话整份替换出厂的（`chat.md` 第七条第 5 条；位置 2026-10-09 核心的主会话定：包自己的系统数据放 `system/modules/<包编号>/`，和 `home/<账号>/modules/<模块>/` 对称；不放 `system/packages/<包>/`，那是装进来的包本身，重装会整个换掉） |
   | 判官的说明（O-23 下） | `judge/*.txt`，十三份（`chat.md` 第六条，登记在 26 第十节） | 没有：给模型看的字随包走 |
   | 给她看的事实的模板（O-25 下） | `facts/undelivered.txt`（「退信」第 3 条，登记在 26 第十节） | 没有：给模型看的字随包走 |

   规则文件是 `venues.d/` 里名字以 `.toml` 结尾、不以 `.` 开头的普通文件（跟着链接）；名字不是 UTF-8 的不认（「施工时定的」第 51 条）。
2. **读**：一份文件照配置文件的读法读（`miyu_store::config_file::read`：超过 1 MiB 不读、开头的 BOM 去掉、不是 UTF-8 不读，`config.md`「怎么走」第二条第 2 条），读不成的变成群聊内核的 `Problem`：原因码照配置的 `unreadable`（带系统的原话）、`too_big`、`not_utf8`，没有第几条规则、键、位置；列不出 `venues.d/` 的也是一条 `unreadable`，文件名写 `venues.d`。规则文件交 `Rules::parse`（出厂的 `Source::Factory`、系统的 `Source::System`，文件名不带目录），出厂参数交 `Params::read`，违规词表交 `Moderation::parse_keywords`，判官的说明（O-23 下）照文件名（`JudgeSources` 的格名把 `_` 换成 `-`、加 `.txt`）读成 `JudgeSources` 交 `JudgeTexts::new`，退信的模板（O-25 下）交内核的 `Template::parse`，字段只认 `why`、`detail`、`text`。
   - **出厂的**：起来时读一次（`Factory::load`），握手以前，和 `bridge.json` 一样。规则文件单独过一遍 `Rules::parse`（合上系统的以后，被同名替换的那一份不读，单独过才查得全）。有一条问题（警告也算，照 `chat.md` 第八条施工时定的第 10 条）、出厂参数读不出来、哪一份不在或读不成、判官的说明 `JudgeTexts::new` 不收（O-23 下：`violations.txt` 模板写坏了，记一条 `bad_format`，文件写 `judge/violations.txt`，原话是模板的错；不在、读不成的文件名也带 `judge/`）、退信的模板写坏了或者要了别的字段（O-25 下：同样记一条 `bad_format`，文件写 `facts/undelivered.txt`），都是打包的错：在标准错误上说 `failure/factory`，接着每一条问题缩进两格一行（`venue/problem`），退出码 1，核心不再重启（`config_error`）。之后放在内存里，跑着不再读（「施工时定的」第 52 条）。
   - **系统的**：出厂的规则文件合上系统的过 `Rules::parse`，坏的那一项、那一条规则、那一份文件照群聊内核的规矩丢，别的照用。系统那一份读不成的照空的用：规则文件照样替换同名的出厂那一份、自己没有规则（和 TOML 写法不对一样整份不用），违规词表照空的（没有词）（「施工时定的」第 50 条）。违规词表：系统那一份在的整份替换出厂的，不在的照出厂的。问题照文件名排，违规词表的在最后。
3. **什么时候重读**（`Venues`）：要用时交进当时的时刻（`Instant`）。离上一次看不到 `bridge.json` 的 `rules_check_millis`（出厂 1000 毫秒）的，照手里的；到了，看一眼系统的两处这一刻的样子：`system/venues.d/` 里规则文件的列表、每一份的修改时刻和大小，`system/modules/onebot/moderation.txt` 的修改时刻和大小（不在也是一种样子；列不出 `venues.d/` 的记这个目录本身）。和上一次的一样，照手里的；不一样，整份重读系统的（第 2 条）。先记样子再读：读的时候又改了的，下一次看得出来。不监视文件（「施工时定的」第 53 条）。
4. **记运行日志**：每读一次系统的（起来时那一次、变了重读的），每条问题一行 `WARN venue rules problem`（原因码、来处、文件、第几条规则、键、行、原文、为什么），读完一行 `INFO venue rules read problems=<条数>`。没变的不记，不会一条问题每秒记一次。
5. **套场所**（`Loaded::at`）：`Rules::resolve(&venue)` 得出每一项的值和来处，再 `Params::at` 套上规则改的参数。
6. **起来时**：握手以后读一次系统的（第 4 条记运行日志）。O-22 起每一条消息照第 3 条用它套场所（「群消息」第 3 到 6 条，私聊的 `show_ids`）；进站链、主动回复判断、出站随后面的步子（「施工时定的」第 54 条）。
7. **`venue show <场所>`**（「施工时定的」第 55 条）：场所编号照内核的 `VenueId` 再 `Venue::parse` 解，解不出的在标准错误上说 `venue/bad-venue`，退出码 2。读出厂的（出厂的有问题照第 2 条那样说，退出码 1），再读系统的（不记运行日志），套到这个场所上，在标准输出上印：
   - 规则设到的每一项一行（`venue/entry`：键、值、来处），照键排；值照 TOML 的写法（字带引号、列表带方括号），参数照 `表.项`（`chatty.probability`）；来处是出厂或系统、文件名、第几条规则、第几行（`venue/rule`）。一项都没有的说 `venue/none`。
   - 接着一句 `venue/defaults`：没列出的参数照出厂的 `defaults.toml`。参数的每一项不印：`Params` 交的是换算好的格，印不回原文；规则设到的已经在上面。
   - 读系统的发现了问题的，一句标题 `venue/problems`，问题缩进两格一条一行（`venue/problem`：在哪、错在哪；在哪照有没有第几条规则、第几行挑 `venue/rule`、`venue/line`、`venue/file`，错在哪一种原因码一句 `problem/…`）。
   - 违规词表不印：它不分场所。

   例子（系统的 `80-test.toml` 给 `qq:group:1` 设了 `rate`、`chatty = { probability = 80 }`，`90-bad.toml` 第 2 条规则的 `rate` 写错了）：

   ```
   chatty.probability = 80（系统 80-test.toml 第 1 条规则，第 4 行）
   discipline = "chatty"（出厂 50-defaults.toml 第 1 条规则，第 14 行）
   parallel = 1（出厂 50-defaults.toml 第 1 条规则，第 16 行）
   rate = "30/60s"（系统 80-test.toml 第 1 条规则，第 3 行）
   没列出的参数照出厂的 defaults.toml。
   读文件时发现的问题（写错的那一项、那一条规则、那一份文件不用，别的照用）：
     系统 90-bad.toml 第 2 条规则，第 6 行：rate 写法不对："abc"
   ```

**样子**：桥起来时在标准错误上说一行「在 127.0.0.1:8301 等 NapCat 连进来」，照握手回的语言（令牌没设的接着再说 `notice/no-token` 那一句，O-16 补二）；NapCat 连上、断开各一行。

**出错**

| 情况 | 说什么 | 退出码 |
|---|---|---|
| 令牌没设、取不到（O-16 补：不算错） | `notice/no-token`，照常跑，NapCat 连进来 401，设了就通（第 1、2 条，补二） | 不退 |
| 端口被占 | 哪个端口被占了、改 `onebot.listen` | 1 |
| 握手被拒、握手时管道关了、回应没带 `language` | 连不上核心，带原因 | 1 |
| `hello_seconds` 内等不到握手的回应（从终端跑起来的，O-18） | `serve` 只由核心拉起，用 `miyu onebot start` | 1 |
| 标准输入读到头、标准输出写不进（核心请它退出、核心不在了，O-18） | 不说，运行日志一行 | 0 |
| 跟核心的那一头崩了、发回话的任务崩了（是 bug） | 出了错、停下，带原话 | 1 |
| 别的原因起不来：听不了、起不了运行时、数据根用不了、`bridge.json` 读不进来、清单里两个端口的默认值读不出来（O-20） | 起不来，带原话 | 1 |
| 出厂的场所规则、出厂参数、违规词表有问题、不在、读不成（O-21，打包的错；`serve` 握手以前、`venue show`） | `failure/factory`，接着每一条问题缩进两格一行 | 1 |
| 系统的场所规则、违规词表写错了、读不成（O-21） | 不说；运行日志每条一行 `WARN venue rules problem`，坏的丢、别的照用 | 不退 |
| 找不到资源目录、给人看的字读不懂 | 这时还没有字可用：`miyu-onebot: <原话>` | 1 |
| `start`、`stop`、`restart`、`status` 连不上核心（O-18） | 连不上核心，带原因 | 1 |
| 核心拒绝（例如 `restart` 关着的，O-18） | 核心的原话 | 1 |
| 核心那边没有 `onebot` 这个包（清单不在、写错了，O-18） | `status/missing` | 1 |
| 用法不对 | 用法（标准错误上） | 2 |
| `venue show` 的场所编号认不出（O-21） | `venue/bad-venue`（标准错误上） | 2 |

**给人看的字**（`serve`、`web` 说的在标准错误上；`start`、`stop`、`restart`、`status`、`logs` 的结果在标准输出上，O-18；`venue show` 的也是，O-21）：放在 `resources/software/onebot/human/{zh,en,ja}.json` 的 `said` 里，照核心的格式和 `Human::load` 的读法（`store/resources.md`「怎么走」第 3 条），说法的编号是 `software/onebot/<编号>`；`texts.rs` 只挑哪一句、换进什么字段，换不出来的（是 bug）印出编号和字段、记一行运行日志。`ja.json` 照英文写，和核心拒绝时的话一样（O-8 施工时定）。握手以前照系统的语言（`zh`、`ja` 开头的照它，别的说英文，和核心照握手的 `locale` 算的一样；O-20 起桥不读配置，不看 `ui.language`），握手以后照核心回的 `language`（`serve` 端口被占那一句也是，O-20）；`start`、`stop`、`restart`、`status`、`web` 握手以后照核心回的说，`logs`、`venue show`（O-21）不连核心，照系统的语言；换成的那种语言的字读不懂，说一行原话、接着照原来的说。

| 编号 | 什么时候 | 中文 | 英文 |
|---|---|---|---|
| `notice/listening` | 起来了 | 在 127.0.0.1:{port} 等 NapCat 连进来。 | Waiting for NapCat on 127.0.0.1:{port}. |
| `notice/connected-as`、`notice/connected` | 连上了 | NapCat 连上了（{bot}）。 | NapCat connected ({bot}). |
| `notice/disconnected-as`、`notice/disconnected` | 断开了 | NapCat 断开了（{bot}），等它重连。 | NapCat disconnected ({bot}); waiting for it to reconnect. |
| `notice/no-token`（O-16 补，原来的 `unready/no-token`；补二改了说法） | 令牌没设：照样起来，跟在 `notice/listening` 后面 | 还没设令牌（onebot.token），NapCat 连进来会被拒。到 WebUI 的「连接」页生成一个，填进 NapCat，几秒内就连上。 | No token (onebot.token) yet, so NapCat will be refused. Generate one on the Connection page of the web UI and put it into NapCat; it connects within seconds. |
| `failure/port-in-use` | 端口被占 | 端口 {port} 被占了。换一个：miyu config set --system onebot.listen <端口>，NapCat 那边跟着改。 | Port {port} is in use. Pick another: miyu config set --system onebot.listen <port>, and change NapCat to match. |
| `failure/core` | 连不上核心 | 连不上核心：{reason} | Could not reach the core: {reason} |
| `failure/crashed` | 跟核心的那一头、发回话的任务崩了 | QQ 桥出了错，停下：{reason} | The QQ bridge hit a bug and stops: {reason} |
| `failure/start` | 别的原因起不来 | QQ 桥起不来：{reason} | The QQ bridge could not start: {reason} |
| `failure/not-spawned`（O-18） | 等不到握手的回应 | 没等到核心的握手回应。miyu-onebot serve 只由核心拉起：用 miyu onebot start 打开 QQ 桥。 | No handshake reply from the core. miyu-onebot serve is started by the core only: turn the QQ bridge on with miyu onebot start. |
| `usage`（O-18 改，O-21 加 `venue show`） | 用法不对、`-h` | 用法：miyu onebot start \| stop \| restart \| status \| logs [-f] \| web [--print] \| venue show <场所>（serve 只由核心拉起） | usage: miyu onebot start \| stop \| restart \| status \| logs [-f] \| web [--print] \| venue show <venue> (serve is started by the core only) |
| `no-log` | 运行日志装不上（照样跑） | 运行日志写不了：{reason} | The run log cannot be written: {reason} |
| `control/started`（O-18，下同） | `start` 成了 | QQ 桥开了：核心拉起它，以后核心每次起来都拉起它。 | The QQ bridge is on: the core starts it now and every time the core starts. |
| `control/stopped` | `stop` 成了 | QQ 桥关了：核心停下它，以后不再拉起。 | The QQ bridge is off: the core stopped it and will not start it again. |
| `control/restarted` | `restart` 成了 | QQ 桥重新拉起了。 | The QQ bridge was restarted. |
| `status/off` | 关着 | QQ 桥关着。用 miyu onebot start 打开。 | The QQ bridge is off. Turn it on with miyu onebot start. |
| `status/starting` | 拉起了、还没握手 | QQ 桥正在起来。 | The QQ bridge is starting. |
| `status/running` | 在跑 | QQ 桥在跑（进程 {pid}）。 | The QQ bridge is running (process {pid}). |
| `status/waiting` | 退避中（`retry_in` 进成整秒） | QQ 桥退出了，{seconds} 秒后再拉起（连续失败 {failures} 次）。 | The QQ bridge exited; it starts again in {seconds} s ({failures} failures in a row). |
| `status/stopped` | 停下了 | QQ 桥停下了，核心不再拉起它：{reason} | The QQ bridge stopped and the core will not start it again: {reason} |
| `status/stderr` | 停下了、带标准错误（接着一行一行缩进两格印） | 它的标准错误的最后几行： | The last lines of its standard error: |
| `status/other` | 不认识的状态（照说不会） | QQ 桥的状态：{state} | QQ bridge state: {state} |
| `status/missing` | 核心那边没有这个包 | 核心那边没有 onebot 这个软件包：清单不在、或者写错了，用 miyu check 查一下。 | The core has no onebot package: its manifest is missing or wrong; check it with miyu check. |
| `status/napcat` | NapCat 连着、问到了实现 | NapCat 连上了：{implementation} {version}，机器人 {bot}。 | NapCat connected: {implementation} {version}, bot {bot}. |
| `status/napcat-bot` | 连着、还没问到 | NapCat 连上了：机器人 {bot}。 | NapCat connected: bot {bot}. |
| `status/no-napcat` | 没连着 | NapCat 还没连上。 | NapCat is not connected. |
| `status/ports` | 两个地址 | NapCat 连 ws://127.0.0.1:{listen}/ws，网页在 http://127.0.0.1:{web}（miyu onebot web 打开）。 | NapCat connects to ws://127.0.0.1:{listen}/ws; the web page is at http://127.0.0.1:{web} (open it with miyu onebot web). |
| `reason/config_error` | `config_error` | 配置错了或者端口被占（退出码 1）。改好以后 miyu onebot restart。 | a configuration error or a port in use (exit code 1). Fix it, then run miyu onebot restart. |
| `reason/failed_repeatedly` | `failed_repeatedly` | 连续失败了 {failures} 次。看 miyu onebot logs，修好以后 miyu onebot restart。 | it failed {failures} times in a row. See miyu onebot logs, then run miyu onebot restart. |
| `reason/not_installed` | `not_installed` | miyu 旁边没有 miyu-onebot 这个程序。 | there is no miyu-onebot program next to miyu. |
| `reason/cannot_start` | `cannot_start` | 起不来：系统不让起，或者建不了它的目录、标准错误的文件。 | it could not be started: the system refused, or its directory or standard error file could not be made. |
| `reason/protocol_mismatch` | `protocol_mismatch` | 清单说的协议版本和核心的对不上。 | its manifest speaks a protocol version the core does not. |
| `logs/stderr` | `logs` 的标准错误那一段的标题 | —— 标准错误 {path} —— | —— standard error {path} —— |
| `logs/log` | 运行日志那一段的标题（有标准错误那一段时才印） | —— 运行日志 {path} —— | —— run log {path} —— |
| `logs/none` | 还没有运行日志 | 还没有运行日志：{path} | No run log yet: {path} |
| `failure/factory`（O-21，下同） | 出厂的数据有问题（接着每一条问题缩进两格一行 `venue/problem`） | QQ 桥的出厂数据有问题（是打包的错），起不来： | The QQ bridge cannot start: its factory data has problems (a packaging error): |
| `venue/entry` | `venue show`：规则设到的一项 | {key} = {value}（{from}） | {key} = {value} ({from}) |
| `venue/rule` | 来处、问题在哪：有第几条规则的 | {source} {file} 第 {rule} 条规则，第 {line} 行 | {source} {file}, rule {rule}, line {line} |
| `venue/line` | 问题在哪：只有第几行的（出厂参数） | {source} {file} 第 {line} 行 | {source} {file}, line {line} |
| `venue/file` | 问题在哪：整份文件的、缺了的 | {source} {file} | {source} {file} |
| `venue/factory`、`venue/system` | 上面的 `{source}` | 出厂、系统 | factory、system |
| `venue/none` | 没有规则设到这个场所 | 没有规则设到这个场所。 | No rule sets anything for this venue. |
| `venue/defaults` | 跟在规则设到的后面 | 没列出的参数照出厂的 defaults.toml。 | Parameters not listed follow the factory defaults.toml. |
| `venue/problems` | 问题那一段的标题 | 读文件时发现的问题（写错的那一项、那一条规则、那一份文件不用，别的照用）： | Problems found while reading (the wrong item, rule or file is not used; the rest is): |
| `venue/problem` | 一条问题 | {at}：{what} | {at}: {what} |
| `venue/bad-venue` | 场所编号认不出 | 认不出场所编号 {venue}。写成 <平台>:group:<群号> 或 <平台>:private:<号>，例如 qq:group:123456。 | Not a venue id: {venue}. Write <platform>:group:<group> or <platform>:private:<user>, for example qq:group:123456. |
| `problem/unreadable` | 上面的 `{what}`：读不成 | 读不了：{why} | cannot be read: {why} |
| `problem/too-big` | 超过 1 MiB | 超过 1 MiB，不读 | over 1 MiB, not read |
| `problem/not-utf8` | 不是 UTF-8 | 不是 UTF-8，不读 | not UTF-8, not read |
| `problem/syntax` | TOML 写法不对 | TOML 写法不对：{why} | TOML syntax error: {why} |
| `problem/unknown-key`、`problem/unknown-key-plain` | 不认识的键，有、没有离得最近的名字 | 不认识 {key}，是不是想写 {suggest}？／不认识 {key} | unknown key {key}; did you mean {suggest}?／unknown key {key} |
| `problem/wrong-type` | 类型不对 | {key} 的类型不对：{got} | {key} has the wrong type: {got} |
| `problem/missing` | 出厂参数缺了一项（`wrong_type`，没有原文） | 缺了 {key} | {key} is missing |
| `problem/not-an-option` | 不是能选的值 | {key} 不是能选的值：{got} | {key} is not one of the options: {got} |
| `problem/out-of-range` | 超出范围 | {key} 超出范围：{got} | {key} is out of range: {got} |
| `problem/bad-format` | 写法不对 | {key} 写法不对：{got} | {key} is not written right: {got} |
| `problem/other` | 别的原因码（照说不会） | {key}：{code} | {key}: {code} |
| `group/rate-limited`（O-23） | 限流满了，别人冲她来：发进群里（「群里怎么叫她」第 7 条，照握手回的语言） | 这会儿叫的人太多了，过几分钟再来吧。 | Too many calls right now; please try again in a few minutes. |

不认识的停下原因照原样印代码。

号没认出来的（没带 `X-Self-ID`、还没来事件）用不带 `-as` 的那一句，不写括号那一段。

**守着它的**（`crates/miyu-onebot/tests/`，O-8；假的 NapCat 是测试里的一个 WebSocket 客户端，核心照别的头的测试起一个真的。O-18 起在进程里跑的桥经内存里的管道连核心：测试那一头把握手补上本机令牌、经本机套接字交给核心，核心那一头照常是 `miyu_endpoint::serve`。O-20 起测试那一头还在握手的回应里填上 `config`（照核心拉起扩展时交的样子），要改配置的照推送的样子往桥那一头写 `extension.config`；测试里的核心照出厂的清单拼进包的配置项，和真核心起来时一样）

- 令牌：三种出示法都认；不对 401（差一个字、多一个字、空的也算）；没出示 401。路径不对 404。端口被占了说是哪个端口。（`listen.rs`）
- 算出来的 `Sec-WebSocket-Accept` 放不进头的回 400、不说升级；放得进的回 101，值照 RFC 6455 的例子。（`src/listen/tests.rs`）
- 发回话的任务崩了，交回「出了错」带原话，桥停下；好好结束的接着办。（`src/core/route/tests.rs`）
- 握手的回应没带 `language`：照连不上核心退，原因里写明。假的核心只回 `hello`。（`core.rs`）
- 管道（O-18）：核心关了管道（读到头），桥好好停下；握手不带凭据、报 `onebot`；`hello_seconds` 里等不到回应说 `failure/not-spawned`。（`pipe.rs`）
- 真的程序 `serve`（O-18）：测试当核心，经它的标准输入输出握手：握手不带凭据，标准输出上每一行都是协议的请求；关了标准输入 5 秒内退出、退出码 0；`-h`、`--help` 印用法到标准输出、退出码 0。握手回应交的 NapCat 端口被占：标准错误上只有一句，照握手回的语言（测试的系统语言是英文、握手回中文）说是哪个端口被占，退出码 1（O-20）。（`stdio.rs`）
- 真核心拉起真的桥（O-18，测试程序里的核心照出厂的清单拉起硬链接在测试程序旁边的 `miyu-onebot`）：`start` 以后在跑、NapCat 连得进来、主人私聊来回，`status` 说在跑、NapCat 连着、两个地址；`stop` 以后桥退出、端口关了、`status` 说关着；桥被杀掉，核心拉起新的一个，NapCat 重连得上；端口被占，核心停下、`status` 说配置错、带出「端口被占」那一句；`restart` 关着的照核心的原话拒绝。O-20：桥照握手交的端口开监听；不重启，`secret.set` 只换令牌的值、`config.set` 把令牌换成引用别的密钥，新的进得来、旧的 401，已经连着的那一条照样收发；`config.set` 换 NapCat 的端口、WebUI 的端口，当场换，旧的关了，状态文件和 `status` 跟着说；`config.set` 删了令牌，以后连进来的一律 401。（`spawned.rs`）
- `status` 说的（O-18）：关着、正在起来、在跑（状态文件的进程号对得上才说 NapCat 和地址，对不上、读不懂的不说）、退避中（毫秒进成整秒）、停下的每一种原因和标准错误一行行缩进、不认识的原因照原样、不认识的状态；核心那边没有这个包。（`control.rs`）
- 状态文件（O-18）：听上了就写，进程号、两个端口、NapCat 没连着；连上以后说号和实现，只从第一条事件认出号的也写，断了说没连着（`status_file.rs`）；推送换了端口跟着换（`apply.rs`）。
- `logs`（O-18）：只有运行日志的原样印；标准错误有内容的先印它、各带标题；还没有运行日志的说一句；`-f` 接着印新写的行，文件换了从头读。（`logs.rs`）
- 主人的私聊走一遍：假 NapCat 发一条私聊，核心里那个场所会话收到一条 `by` 是主人本人、带 `via` 的消息，命令编号是 `qq:<机器人的号>:<消息编号>:<时刻>`；她回了一句，假 NapCat 收到 `send_private_msg`，字对得上。用核心的测试模型替身。（`private.rs`，下面三条同）
- 同一条消息发两次，会话里只有一条；同一个消息编号、时刻不同的是两条，都送进去、都回。没带 `time` 的照样送进去，命令编号的时刻是 `0`。
- 不是主人的私聊：不进任何会话（主人的会话已经有了也不进），不回话。`venue.session` 回应的 `account` 是桥自己的账号（测试那一头照核心 O-4 中以后的样子改写握手和 `venue.session` 的回应）：不接、不交 `session.send`、`command.run`、不订阅，下一条照样再问；`account` 是别的账号、没带这一格的照常接、照常回。同一个陌生人连发几条，运行日志只记一行（`stranger_log.rs`，自己一个测试程序，装着日志订阅者）。
- 段的数组、CQ 字符串两种格式；只有图片的消息不送。同一个号再连进来，新的顶掉旧的，旧的断开不拿掉新的。
- 陌生人的私聊走真核心那条（核心 O-4 下合了以后补，真核心拉起真桥）：陌生人连发两条，核心把场所会话归系统账号 `onebot`、只造一个，桥认出属主是自己，不接：那个会话里没有人说的话，不回话，运行日志 `not the owner, not taken` 只一行；主人的私聊照常来回。（`spawned.rs`）
- 斜杠命令（O-19，真核心加真桥、假 NapCat）：`/clear`、`/stop` 成了，核心照中文写的回执发回 QQ，她不开新的一轮，会话里记一条 `command.ran`；没东西可清的 `/reset` 回核心「上下文为空」那一句、什么都不记、不交给她；`/workspace` 不带路径回现在在哪；`/remember 某句` 回核心 `memory_unavailable` 那一句、不交给她；`/xxx`、`/ 你好`、`/` 照普通的话进会话、她回话；同一条命令平台重发两次只执行一次（回执两次）；开头有空白的 `  /stop` 也认；不是主人的 `/stop` 不进任何会话、不回；会话被删了，命令照第 7 条再找、再交一次；`/stop` 打断她还没开口的一轮，不发空的，桥不卡住、下一句照常来回。（`commands.rs`，下面一条同）
- 运行日志（O-19，真核心拉起真的桥）：记 `command ran` 带正名、`command refused` 带原因码，`/remember` 后面的原文不进日志。
- 思考、工具调用不发回去；空的回复不发。（`replies.rs`，下面一条同）
- 桥重启以后，以前的回复不再发一遍。
- 调用等了给的时限（测试给 3 秒，和出厂的不一样）还等不到算失败，到时以前还在等；连接断了在等的算失败；同时在等的几个照 `echo` 各拿各的。（`calls.rs`，钟停住，照停住的钟算）
- 出厂的 `bridge.json` 读得进、数和上面的表一样（O-21 多 `rules_check_millis`，O-23 下多判官的两格，O-23 补多 `judge_persona_seconds`，O-25 上多 `receipt_recall_seconds`，O-25 中多 `queue_expire_seconds`）；队列写 0、判官的并发写 0、排着的过期写 0、多一格、少一格、不是 JSON、没有文件，都读不进来，说是哪个文件。（`tuning.rs`）
- 三种语言里桥说的每一句都换得出来（O-21 多 `failure/factory`、`venue/`、`problem/` 开头的，O-23 多 `group/rate-limited`）；中文照上面的表、字段换进去；日文和英文一字不差；换语言照新的说；握手以前照系统的语言（`zh`、`ja` 开头的照它，别的英文）。（`texts.rs`）
- 文字怎么读出来：别的段跳过；CQ 码去掉，`&amp;` 最后换。（`text.rs`）
- 编号：场所、平台上的人和群聊内核拼的一样（`qq:private:<号>`、`qq:<号>`，解得回原样）；命令编号带时刻，同一个消息编号、时刻不同的两条编号不同；`time` 是整数、写成整数的字符串都认，没带、读不出（`null`、不是数的字、小数）的是 `0`。（`ids.rs`）
- 握手交来的配置（O-20）：两个端口照交来的；没有的、`null` 的、不是 0 到 65535 的整数的照清单的默认值；令牌是字的照它、去掉前后空白，没有的、`null`、空的、不是字的是没有；推来的只换带了的键，别的键不认；出厂清单的默认值是 8301、8302，清单不在、没写默认值的读不出来、说是哪个文件。（`settings.rs`）
- 令牌没设（O-16 补、补二，O-20 改）：桥照样起来，两个端口都开，先说在哪等 NapCat、再说 `notice/no-token`；NapCat 连进来 401；推来令牌，不重启，NapCat 下一次连就通。再换一个：新的连得进、旧的 401，已经连着的那一条照样收发；推来 `null`：一律 401。真的程序 `miyu-onebot serve` 也这样起来（测试当核心，经它的标准输入输出握手、推送；标准错误、运行日志各一句），推来令牌以后 `/status` 的 `token` 从 `none` 变 `set`，NapCat 不用重启就连上，`/token` 交出值、运行日志里没有这个值；`miyu-onebot web --print` 照状态文件里的端口印出网址。（`no_token.rs`）
- 场所规则和出厂数据（O-21，`rules.rs`，什么时候重读在 `reload.rs`；钟是交进去的时刻，不等）：出厂的读得出、零问题，违规词表 153 个词；出厂、系统两份照文件名的先后套，系统同名的整份替换出厂的，值和来处（出厂或系统、文件、第几条、第几行）都对；名字不以 `.toml` 结尾的、以 `.` 开头的、目录不当规则文件；系统的写错只丢那一项、那一条，读不成的（超过 1 MiB、不是 UTF-8）报出来、照空的用、同名的出厂那份也不用，别的照用；问题照文件名排（读不成的夹在写错的中间也是），`venues.d` 列不出来的报一条、出厂的照用；套场所时参数照规则改；出厂的写错（规则写错、只有一条警告、出厂参数写错、违规词表不在、`venues.d/` 不在）读不出来，交回每一条问题；判官的说明（O-23 下）：`violations.txt` 的模板要了别的字段报一条 `bad_format`（文件写 `judge/violations.txt`，原话带着那个字段），少了一份报一条读不成（`judge/answer.txt`）；改了、加了、删了系统的规则文件，隔够 `rules_check_millis` 的下一次用照新的，没隔够的照旧，没变的不重读；违规词表系统那一份替换出厂的，读不成的照空的，删了回到出厂的。
- `venue show`（O-21，`venue.rs`）：印的值和来处、照键排、没有规则的说没有、问题印在后面一条一行，中文一字不差；没有系统规则的只印出厂的；场所编号认不出说 `venue/bad-venue`、退出码 2；出厂的写错在标准错误上说 `failure/factory` 和每一条问题、退出码 1。真的程序：`miyu-onebot venue show` 照系统的语言印、退出码 0；`serve` 出厂的写错握手以前就退、退出码 1、说是哪条问题；系统的写错照常起来，运行日志里有那条问题。
- 依赖（O-20）：`cargo metadata` 里 `miyu-onebot` 的依赖（开发依赖不算）没有 `miyu-core`、`miyu-endpoint`（18 第一节「桥不依赖核心的 crate」）。（`dependencies.rs`）
- 群消息（O-22，真核心拉起真桥、假 NapCat，`group.rs`；起核心和桥、读群会话的事件 O-23 挪进 `support/group.rs` 共用）：群消息记进 `qq:group:<群号>` 那个会话，属主是系统账号 `onebot`，`by` 是外部身份、`ambient` 真，不开回合、不请求模型；名字取群名片、没有的取昵称；@ 她记 `mentions_me`、@ 别人进 `mentions`、正文里写成 `@名字`（发过言的照缓存、没发过的问 NapCat、问不到的写号），@全体记 `mentions_all`、正文里不写；引用记 `reply_to`；图、表情包、小黄脸、商城表情、语音、视频、文件各自记对，合并转发、卡片在正文里写占位；规则的 `show_ids` 照写；`managers` 里的人报 `manager`，群里的主人带 `account`；机器人自己发的不记；规则里睡觉时间盖住此刻的群 `asleep` 真；规则写了不存在的人格的群什么都不记，连发两条运行日志只一行；群里的 `/stop`、`/clear` 主人、管理的人能用，回执发回群里，别人照核心那一句被拒、什么都不记，认不出的 `/xxx` 记成旁听；撤回记 `venue.recalled`（撤的人是 `operator_id`）。只有一张图、只有一个表情的群消息记得进：没有内容块、`venue.media` 对、运行日志没有 `message refused`（核心 O-13 补以后补，`a_message_with_only_an_image_or_a_sticker_is_recorded`）。
- 私聊带上场所的格（O-22，`private.rs`）：`msg`、`name`、`reply_to`、`media`、`show_ids`（照私聊的规则），没有 `ambient`；照旧来回；私聊的撤回记 `venue.recalled`。
- 认段（O-22，`segments.rs`）：每一种段记成什么；编号取哪一格、整数也认；几段引用只认第一段；@ 同一个人两次只进一次；CQ 字符串只读字。认帧（`frames.rs`）：群消息、私聊带上名字（群名片、昵称、空白的群名片）；机器人自己发的不认；两种撤回认出撤的人；缺了号的不认。
- 群成员的名字缓存（O-22，`members.rs`）：记下的取得到，过了 `member_names_seconds` 的取不到；按群分开；`get_group_member_info` 回的群名片空白的取昵称。
- 场所的格的洗法（O-22，`src/core/route/fields/tests.rs`）：只写有的、真的格；名字去掉控制字符、截到 64 个字符（带的东西的 200），空白的不写；引用、带的东西的编号不合的那一格、那一样不记，消息照记。场所规则套到这一条上（`src/core/route/applied/tests.rs`）：人格、预设、工作区照规则带、没设的不带；`managers` 里有的是管理的人（别的平台的同一个号不算）；`show_ids`；睡觉时间盖住此刻的睡着，没盖住、`off`、没设的醒着。
- 群里叫她（O-23，真核心拉起真桥、假 NapCat、模型替身，`called.rs`）：主人 @ 她、叫她的名字（触发词开头）、引用她发过的消息，各开一轮（`turn.started` 的 `cause` 是消息的命令编号加 `/respond`、`triggers` 是那一条），她的回话转成纯文本发回群里，记 `venue.delivered`（线是这个群的会话、回合、回的人是主人、NapCat 回的编号、那一段字）；每一条都记一笔判断，`cause` 是消息的命令编号加 `/decided`，没条件的、主人 @ 她的 `body` 和图纸一字不差；别人的 @ 走 `judge`，照剧本回的核心没有一次性入口、回 `no_model`，判不了、再问一次，只记下（`judge` 是 `{mode: reply, tries: 2, millis, unjudged: refused, detail: no_model}`、结论 `record`，O-23 下），不开回合、不回；睡觉时间里别人冲她来只记下（`record_only`、`asleep`），主人照回；主人连发两条，正在跑的一轮并进去（`turn.joined`），只开一轮；同一条消息平台重发不再判；原文不进运行日志。
- 限流和重启（O-23，`called_limits.rs`）：规则 `rate = "1/1h"`，别人的一轮（测试照判官点了头的样子经 `session.respond` 开）发回群里，满了以后主人照回；握手交来的自己人冲她来不受限流（`pass`），可额度满了的这段时间不问判官、只记下（`judge` 是 `{mode: reply, unjudged: rate_full}`，O-23 下）；别人冲她来回一句 `group/rate-limited`、记 `ext.onebot.venues.queued {kind: notice, reason: rate_limited}`（`cause` 加 `/queued`），再来只记下（`record_only`、`rate_limited`）；桥重启以后照日志重建：别人冲她来照旧只记下（回合、提示都重建了），主人引用她重启以前的话照样认得、回，以前的回复不再发一遍。
- 她的话照发的先后记（O-23，`called.rs`）：假 NapCat 把拆成两段的一句话的回应倒着回（后发的先回、隔 300 毫秒再回先发的），`venue.delivered` 照发的先后记、编号对得上。
- 投影（O-23，`src/core/route/projection/tests.rs`）：带 `account` 的是主人、没收过的不是；回合去掉触发的人全是主人或自己人的，没有触发的、有一个别人的照算，自己人照交进来的名单；这一轮回的人并进 `turn.joined` 的、不重，别的回合的不算，完了以后是空的；核心为并进去的那几条接着开的一轮（`trigger` 指向那条 `turn.joined`）回的人照它找回，回合的触发的人也是，指向别的的照旧是空的（O-23 下）；她的回复同一轮的并成一笔（时刻取第一条、回的人取并集），编号都记下；只认限流的提示；不大于 `upto` 的她的话不交出来，重的、更早的不收，掉队再补的 `upto` 只往后挪。判过要回的（O-23 下）：结论是回的才算，发的人、时刻照最后一条，前面几条是接过的，认不出的条件不要；开了收它们的那一轮照旧算，并进同一轮的也是；别的一轮完了不算，收了它们的那一轮完了就回完了，还没进哪一轮的照旧算；判的那几条读不出、最后一条没收过的不算。
- 判一条（O-23，`src/core/route/decide/tests.rs`，参数照出厂的 `defaults.toml`）：主人冲她来回，`body` 和图纸一字不差；没条件的只记下；别人、自己人冲她来的问判官打分、只有违规旗的只查违规（O-23 下）；主人没冲她来的只记下；限流满了别人冲她来头一回提示（没有条件、没有路）、提示过了只记下、不冲她来的只记下、自己人和主人不受限流，可额度满了自己人冲她来不问判官、只记下（`rate_full`），抽样必中的也不抽（O-23 下）；睡着的别人和群里的自己人只记下、主人照回；不让叫的只记下。
- 线路规程和顶替（O-23 下，`src/core/route/decide/follow_tests.rs`、`discipline/tests.rs`）：四种的写法读得回来，没设的、认不出的（大小写不同、空的）照 `chatty`；`chatty` 留全部条件（额度满了去掉抽样），`when-called`、`wake` 只留冲她来、续聊、违规旗，`every-message` 去掉抽样，加分照原样；只有 `chatty`、`when-called` 看顶替；`chatty` 照群聊内核走路，`when-called`、`wake` 只有违规旗的只查违规、和别的一起的和别的有条件的开一轮，`every-message` 有字的都开一轮（只有违规旗的也是）、只有图的只记下；`when-called` 冲她来直接回、抽样必中也不抽、只有违规旗的问判官只查违规、额度满了不问只记下（`rate_full`）、冲她来又有违规旗的照回。顶替：前一条判过要回的接过去（`supersede.inherit`、条件是前一条的、`commit`），`when-called` 也接、`wake` 不接；还在判的几条一起问判官（判的是那一条接过的、那一条、这一条），主人补一句 @ 她的几条一起回；正好 7 秒以前的不算，差一毫秒的算。
- 判断的 `body`（O-23 下，`src/core/route/body/tests.rs`）：问了判官的写全模式、几次、多久、模型、五维、`should_reply`、`to_bot`、`severity`（没查的不写）、`reason` 和算分的每一项，分够的回、不够的只记下；判不了的六种（`queue`、`timeout`、`refused` 带原因码、`unreadable` 带 `no_object`、`no_severity`、`dimension:<名字>`）都只记下、没有分；额度满了的只写模式和 `rate_full`；接过的、放下的那一条；条件的五种写法读得回来，认不出的读不回。
- 问一次判官（O-23 下，`src/core/route/ask/tests.rs`；核心那一头是测试，钟停住）：先 `venue.records {session, msg, count: 20}`，再 `model.call`：`purpose` 是 `judge`、`max_tokens` 400、没写模型的不带、两条消息，system 照出厂的说明拼（不带人格、门槛换进去、只查违规的换那一份），user 夹着记录、这一条、base64 解出来的字（没有的不夹）；包在代码块里的回答读得出，模型记成 `<供应商>/<模型>`；记录的条数照参数，写了模型的带上；读不出、核心拒了、等不到的再问，几次照 `retries`，交回最后一次的为什么、回过的那一次的模型；打分等 60 秒、只查违规等 120 秒，等不到的再问一次回了的照用，晚来的丢掉；名额满了等 15 秒还没有的不问（`queue`、0 次），10 秒有了名额接着问、耗时从交出去算；记录被拒的不再问；核心断开了交回空的。
- 判官带的人格（O-23 补，`src/core/route/ask/persona_tests.rs`；同上，钟停住）：有人格的先拿记录、再 `persona.read {persona, prompt: "persona"}`，原文夹在 `persona-open`、`persona-close` 中间、紧跟着 `system.txt`；60 秒里同一个人格不再读，到了 60 秒再读，别的人格另读；`text` 是 `null` 的不带、照样记下；读不到的（`unknown_persona`）照样问判官、不带、不记下，下一次再读；没给人格的不读；读的时候核心断开了交回空的。
- 并着发的调用口（O-23 下，`src/core/caller/tests.rs`）：两个一起等，后发的先回，各拿各的；推送、跟核心的那一头的回应、核心发来的请求原样交回，没人等的回应丢掉；不等了的编号拿掉、晚来的丢掉；读的一头停了，等着的和再来的都断开；管道关了断开。
- 问判官（O-23 下，真核心拉起真桥、假 NapCat、她照剧本说、判官那一次发到本机回环上的假服务器，`judged.rs`）：别人 @ 她判官说回就回、说不回只记下；抽样中了交判官；只有违规旗的只查违规（system 照 `moderation-only.txt`），严重程度 8 照回；`ext.onebot.chat.decided` 的 `judge`、`score` 每一格对（冲她来的免冷静：0.9 + 0.2 + 0.3 对 0.8）；判官看到的是一条 system、一条 user，照出厂的说明拼，模型照 `models.chat`，最多输出 400；两次都读不出的判不了（`unreadable`、`no_object`、两次），头一次少了几维的再问一次回；判官一秒内没回的不等了（`timeout`、一次、没有模型），晚到的回答丢掉、不开回合；理由、原文不进运行日志，判不了的另记一行 `not judged`；判官在判（30 秒才回）的时候，别人说的一句 10 秒内记进、判完（核心的 `model.call` 在后台答，施工 8-20 补）。
- 判官带人格（O-23 补，`judged_persona.rs`，同上，真核心的系统区装上样本人格）：规则给群设了样本人格 `engineer`，判官的 system 开头是 `system.txt`、`persona-open.txt`、样本的人设、`persona-close.txt` 接着；设了人格、又写了 `judge = { persona = false }` 的群不带；无人格的群不带；会话造好以后人格删了的不带、照样问，运行日志有一行 `persona not read`。
- 顶替（O-23 下，`superseded.rs`）：别人 @ 她、判官说回，她还没开口他补一句没 @ 的：接过去，并进她这一轮（`turn.joined`，`cause` 是补的那一条加 `/respond`），判断和图纸一字不差，不再问判官；判官还没回他就补一句：放下前一次，两条一起问（判官看的这一条是后一条，前一条在记录里），回了，一轮的 `triggers` 是两条，判断只有一笔（`msgs` 两条、`supersede.rejudge`），前一次晚回来的说不回丢掉、不记判断。
- 线路规程（O-23 下，`disciplines.rs`，抽样开到必中）：`every-message` 有字的开一轮、只有图的只记下；`when-called` 没叫的只记下（不抽样）、@ 她开一轮、她还没回完补的一句接过去、有人说了违规词没叫她：问判官只查违规（system 照 `moderation-only.txt`），判官说严重程度 2 不回、8 才回；`wake` 没叫的只记下、唤醒词开一轮、她还没回完补的一句不接、她回过的人接着说开一轮（续聊）、别人接着说不算（刚说过话不算）；判官只为那两句违规的问过。
- 自己人的配置（O-23，`settings.rs`）：`onebot.trusted` 字的列表照收，列表里不是字的不要，没有、`null`、不是列表的是空的。
- 出站链（O-25 上，真核心拉起真桥、假 NapCat、她照台词说，`outbound.rs`；台词能在一次回复里既说话又调一件不存在的工具、能等测试放行再说，`support/speaking.rs`）：漏进正文的工具调用清掉、照剩下的发；整条括号旁白、只有空白和零宽字符、整条是工具调用的不发，运行日志各记一行 `why`、不记原文；同一轮说了两遍一样的（前一句记下了再放下一句）只发一次、只记一笔 `venue.delivered`，她连着说的后一句引用；隔了 4 条别人的话以后回，第一段是引用（编号是主人那一条的）加字，后面几段只有字；@ 隔一秒就够的群里，刚说完紧接着回两样都不带，过了一秒多、期间别人说过一句，第一段是 @ 主人、一个空格、字，只隔一条不引用；私聊（进程里的桥）的回话只有一个文字段，同一轮重复的那一句不发。
- 群里的命令回执撤回（O-25 上，`receipts.rs`）：主人私聊的 `/stop` 回执不撤；群里主人的 `/stop` 回执、别人的 `/stop` 被拒的那一句都撤，撤的是 NapCat 回的编号（两个一样等 3 秒，照编号比）；运行日志两行 `receipt recalled`，发命令的那两条不撤。假 NapCat 把撤回另放一处，不插进别的测试等的动作里。
- 撤回执（O-25 上，`src/core/route/receipt/tests.rs`，钟停住）：NapCat 回了编号以后，差一毫秒不到时候不撤，到了撤的是那个编号；那时没连着的不撤（没回编号的不撤，O-25 中挪到 `sending.rs`，照回应算成什么在 `queue/tests.rs`）。
- 投影交给出站链的（O-25 上，`src/core/route/projection/outbound_tests.rs`）：她回的那一条是触发的最后一条，并进来的换上，编号、发的人、时刻都对，这一轮完了是空的；那之后别人说了几条，发它的人自己补的不算；群里最后一条是不是她的照序号比，发出去到回执之间进来的人话算在她那一段以后，主线发来的照回执的序号；这一轮发出去的换了回合就清，晚到的上一轮的不算，别的回合的不给。
- 出站（O-25 上，`src/core/route/outbound/tests.rs`，参数照出厂的）：群里本来想要两样，过了多久从那一条记下算；没有她回的那一条两样都不要，没有编号的不引用；隔了 4 条第一段引用、隔了 15 秒有人说过话 @、紧接着两样都不带，转成纯文本再拆；私聊两样都是假；丢了的四种原因和名字；清理以后照剩下的拆；私聊这一轮发出去的换了回合就清。
- 测试程序旁边链桥（O-25 上顺手修，`linking.rs`）：八个线程同时去链同一个、一连二十轮，都成，链出来的是同一个文件。
- 出站队列（O-25 中，真核心拉起真桥、假 NapCat，`queue.rs`；假 NapCat 的应答挪进 `support/answering.rs`，多一种发消息一律回失败的 `refusing`）：群里她的一句拆成两段，每段先记 `queued`（`{kind: reply, text, line, turn}`，`line` 是这个群的会话、`turn` 是这一轮）、再记 `venue.delivered`，入队的序号在送达前面；NapCat 回失败的记 `failed {queued, why: rejected, detail}`，`detail` 去掉空白、截到 200 个字符，不记送达，运行日志 `reply not sent … why=rejected`；她被禁言（禁 600 秒）记 `muted {until}`（本机此刻加 600 秒），禁言时主人 @ 她只记下（`record_only`、`muted`），主线经 `session.respond` 开的一轮她的两段排着（`reply waiting … why=muted`），解禁的通知来了照先后发、`unmuted` 的 `body` 是 `{}`，解禁以后主人 @ 照回；禁言改短成 1 秒（再推一次 `ban`），到了自己发、不另记 `unmuted`；别人被禁言、全员禁言、没带秒数的不认，禁她的才认；禁言时排着没发的也算这一回合说过的：差个标点的一句不入队，解禁以后照先后发那两句。
- 出站队列要等的（O-25 中，`queue_waits.rs`；测试的资源目录是一份拷贝，`queue_expire_seconds` 改成 1，`support/mod.rs` 的 `Home::spawning_tuned`）：禁言着排过了期限记 `failed {queued, why: expired}`，解禁以后不发；断着排过了期限同样，连上以后不发；断着时她的话排着（`reply waiting … why=disconnected`），连上了发，入队在送达前面；桥重启：断着时入队了的那一句不补发、不另记，群里来一条消息、桥从头订阅以后，她差个标点的一句照日志算重复、不发，最后一句照发；私聊（进程里的桥）断着时排着、重复的那一句不入队，连上了照先后发，`queued` 的 `line` 是私聊的会话。
- 提示、回执入队（O-25 中，`called_limits.rs`、`receipts.rs`）：限流的提示记 `queued {kind: notice, reason: rate_limited, text}`；群里的回执、被拒的那一句、私聊的回执都记 `queued {kind: receipt, text}`。进程里的桥私聊来回一次，问核心的是 `venue.session`、`subscribe`、`session.send`、`events.append`（`private.rs`）。
- 出站队列的纯逻辑（O-25 中，`src/core/route/queue/tests.rs`，钟停住）：门开着的照入队的先后交、关着的留着、交完了不再排；入队时刻加期限不晚于此刻的作废（门关着也是），差一毫秒不算；过期的先交出来、没过期的照交；会话各排各的；该醒的时刻是最早的过期、禁言到期的时刻，没排着的不醒；禁言到此刻加几秒，超出范围的没有；回应算成什么：成了带 `message_id`（整数、写成整数的字），回了失败是 `rejected`、带去掉空白截到 200 个字符的原因（空的、没有的不带），等不到 `timeout`，断了 `disconnected`。
- 投影（O-25 中，`src/core/route/projection/outbound_tests.rs`、`projection/muted_tests.rs`）：这一轮发出去的照入队的算，只有回执的不算，换了回合就清、晚到的上一轮的不算；桥先算上的，日志推来同一段不重复算；禁言到 `until` 为止（到了不算），解禁了不算，再禁一次、改短了照最后一条，读不出 `until` 的不改；桥重启补来的（`upto` 以内）照样。
- 禁言的通知（O-25 中，`frames.rs`）：禁她的认成禁言（号、秒数写成整数的字也认），`lift_ban`、禁 0 秒认成解禁；禁别人、全员禁言、没带秒数、负的、别的种类不认。
- 退信（O-25 下，真核心拉起真桥、假 NapCat，`undelivered.rs`；她照台词说的替身记下每一次请求）：NapCat 回失败的她的话，记了 `failed` 以后会话里多一条 `context.injected`（`kind` 是 `undelivered`，命令编号是 `<入队那一条的序号>/note`，写着 `why`、`detail` 和那一段的开头），带这一轮的回合编号，她这一轮的下一次请求里有它；一段一块，同一段只退一次；群里的回执失败了不退；私聊的也退。排过了期的（`queue_expire_seconds` 改成 1）写 `expired`、`detail` 是空的，她下一轮开头看到；那一段长的只带头 30 个字符。
- 退信的模板（O-25 下，`undelivered.rs`）：出厂的读得出、字段是 `why`、`detail`、`text`；写坏了、要了别的字段的起不来，问题的文件是 `facts/undelivered.txt`。
- 贴表情（O-25 下，真核心拉起真桥、假 NapCat，`reactions.rs`；假 NapCat 把 `set_msg_emoji_like` 另放一处，不占发出去的编号）：主人 @ 她，那一条先贴上（`{message_id: "<编号>", emoji_id: "289", set: true}`），她回了第一段（这一轮还没完）摘掉，这一轮完了不再摘；续聊她的贴；顶替接过去的，前一条当场摘、贴到新的那一条；判官在判时补的一句（两条一起判、判官说回）贴在后一条上，在判的时候不贴；她一直不回（`reaction_seconds` 改成 1），到时候摘，后来回了不再摘；判官说不回的、抽样、刚说过话判下来要回的不贴；运行日志 `reaction set`、`reaction removed`。
- 贴着的几条（O-25 下，`src/core/route/reaction/tests.rs`，钟停住）：推来的 `turn.started`、`turn.joined` 记下进了哪一轮，那一轮的 `venue.delivered`、`turn.ended` 发摘的信号，别的轮的、别的会话的（序号、回合编号各数各的）不发，没进哪一轮的不发，同一条只发一次；贴、等、摘的任务：信号来了或者到了时候摘一次，经那时的连接，贴不上的不摘，信号的一头放下了（桥在停）不摘。

**施工时定的**（O-8）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 1 | 只要两项配置：端口、令牌；机器人的号照 NapCat 报的 | 握手时 NapCat 自己报，现在用不上第三项 | 先声明 `onebot.self_id` |
| 2 | 默认端口 8301 | 网页软件是 8300，挨着好记；旧版 QQ 和网页共用 8300，NapCat 的端口要跟着网页变（18 第三节） | 和网页共用 |
| 3 | NapCat 必须带令牌；没设时桥照样起来，NapCat 的端口照开、连进来一律 401，设了不用重启（O-16 补，2026-10-07；补二，2026-10-08 项目主人定） | 18 第三节「默认只接本机，并且要带访问令牌」「还没配好就 `start`：桥只开 WebUI，不连 NapCat，等配好」：第一次用在 WebUI 里生成令牌，不用先上命令行；端口先开着，令牌一设 NapCat 下一次连就通（第二条「施工时定的」第 18 条） | 本机连进来的可以不带；令牌没设就不起来（O-8 原来的做法：WebUI 也打不开，和 18 第三节对不上）；令牌没设不开 NapCat 的端口（O-16 补的做法：设了要重启才开） |
| 4 | 订阅不写 `after` | 桥重启不重发旧回复；桥停着时她说的也不补发（O-25 中，「出站队列」第 6 条：可能已经发出去了） | 记住看到哪条、重启接着推 |
| 5 | 核心断了桥就退（O-18 做了：标准输入读到头就停，崩了由核心退避重启，第 21 条） | 9-4 以后核心拉起，崩了照退避重起；桥里不另写一套重连 | 桥自己重连核心 |
| 6 | **已去掉（O-20，2026-10-09）**：桥不读系统配置和密钥文件，用核心交的：握手回应的 `config`、推送 `extension.config`（`extensions.md`「配置」，核心那一头 9-4 下下），只在核心亲手拉起、走标准输入输出的连接上给，密钥是真值（`config.md` 第九条的例外）；不再依赖 `miyu-core`、`miyu-endpoint`（18 第一节「不依赖核心的 crate」）。原来（O-8 到 O-19）是权宜：桥用核心那一份读配置的代码（`Config::load`，照核心登记的全部清单）自己读，只读，令牌对不上时重读（O-16 补二）；系统配置里写了软件包的键时 `onebot.log` 记一条不认识的键的 `WARN`，随它一起没了 | 密钥不经头的协议交出去，桥要用令牌的值；握手正好在开监听以前，回应里带上配置，起来时就有 | 经 `config.get` 读：拿不到密钥的值；桥自己读盘（原来的权宜） |
| 7 | 两项放在高级页的「QQ 桥」组（`onebot`），排在「运行日志」后面 | 页的先后不变；一个桥一组，以后的几项跟着进来 | 放进权限页「平台账号」组：端口不是权限 |
| 8 | 会话不在了：`session_not_found`、`session_stopped` 都忘掉、再问一次 `venue.session`、再发一次，只重来一次；推来 `resync` 的再订阅一次（不写 `after`） | 会话被删、停了都是这个场所以后还要说话；只重来一次不会打转 | 只认 `session_not_found`：会话停了以后这个私聊就一直发不进去 |
| 9 | 发进去的字照原样；去掉首尾空白是空的才不送 | `session.send` 照原样成一块文字 | 先去掉首尾空白再发 |
| 10 | 回话照她说的先后放进写队列，等 NapCat 回应的那一步交给别的任务 | 一句话的回应要等，几句话的先后不能乱 | 每句回话一个任务从头发：先后靠调度 |
| 11 | 查询参数里的令牌照原样比，不做百分号解码 | NapCat 用头出示；朴素 | 照 URL 解码 |
| 12 | 自己编的命令编号（`venue.session`、`subscribe`）带一段随机前缀 | 同一个编号核心交回上一次的结果，桥重启以后从 1 数起会撞上 | 从 1 数起 |
| 13 | 握手报 `head.kind = "onebot"`、`caps.input = false` | 场所会话没人能确认（`venues.md`） | 照终端报能输入 |
| 14 | 跟核心的那一头崩了、发回话的任务崩了（是 bug）：说一句带原话、退出码 1 | 桥这时已经不能干活（发回话的任务只等回应、记日志，它崩了说明那一头的代码有错），照实报，不装作还在跑 | 记一行接着跑；当成核心断了退（原因说错了，排查走错路） |
| 15 | 桥自己的数（路径、调用时限、两个队列、接不了连接歇多久）放在 `resources/software/onebot/bridge.json`，照 `web.json` 读；登记簿门禁只豁免这一份文件 | 数是数据，住在代码之外；不进配置清单：没有人要改它们，进了清单就要给人看的名字、说明和设置页的位置。只豁免一份文件：这个包以后要放给模型看的字（群聊），整个包豁免会漏查 | 写成代码里的常量；进 `onebot.*` 配置；照 mermaid、net 豁免整个 `software/onebot/` |
| 16 | 平台的名字 `qq` 只写一处（`onebot.rs` 的 `PLATFORM`），场所、平台上的人、命令编号都经三个小函数拼（前两个经群聊内核拼，第 20 条） | 散在几处，改一处漏一处，拼出来的场所和找会话用的对不上 | 每处自己 `format!` |
| 17 | 给人看的字放 `resources/software/onebot/human/{zh,en,ja}.json`，照核心的格式和 `Human::load` 读；`ja.json` 照英文写；找不到资源目录、字读不懂时印原话 | 文字是数据；照核心的格式，门禁（`human_languages.rs`）一起查三种语言的键、字段对得上；日文没有专门写的，和核心拒绝时的话一样照英文。资源目录、字本身出问题时没有字可用，只能印原话 | 留在 `texts.rs` 里；不写 `ja.json` 让 `Human::load` 退到英文（门禁要求三份都在） |
| 18 | 握手的回应没带 `language`：照连不上核心退，退出码 1 | `protocol.md`「握手」说回应一定带；没带是协议不对，照实说 | 当成英文接着跑 |
| 19 | 算出来的 `Sec-WebSocket-Accept` 放不进头：回 400、不升级 | 回 101 却不带它，NapCat 握手失败，这一头却当升级成了 | 照样回 101 |
| 20 | 场所、平台上的人经群聊内核拼（`Venue::new`、`miyu_chat::person`），拼不出来的记一行 `WARN`、这条不送；命令编号末尾加事件的 `time`，没带、读不出的照 `0` 拼（照 `chat.md` 第七条第 1 条改，2026-10-07） | 编号的规矩只有一份，桥拼的和群聊内核解的对得上。加时刻：NapCat 重置本地库以后消息编号会重号，只拼编号的话新消息被当成重发吞掉（O 线自查第 17 条）。没带 `time` 照 `0`：OneBot v11 每个事件都带 `time`，没带是实现不合规；照 `0` 拼消息照样送，去重退回只看消息编号，和加时刻以前一样，不比不收差 | 没带的不收：主人的话被吞了还看不出来；照桥收到时的本机时刻拼：同一条重发过来时刻不同，去重失效 |

**施工时定的**（O-18，2026-10-08；表头的五条照施工单的推荐定：`serve` 只说标准输入输出、`status` 读状态文件、开关调 `extension.*`、`logs` 两份都印、测试照核心的办法）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 21 | 标准输入读到头、标准输出写不进：好好停下，退出码 0，不说话，运行日志一行 | 读到头是核心请它退出（`extensions.md`「怎么走」第 4 条），不是出错；核心停它时不看退出码，核心自己没了也没人看。说话会进 `onebot.stderr`，每停一次多一句 | 照原来的「核心断了」说一句、退出码 1 |
| 22 | 握手等回应有期限 `hello_seconds`（出厂 10 秒，放 `bridge.json`），等不到说 `failure/not-spawned`（`serve` 只由核心拉起、用 `miyu onebot start`），退出码 1 | 从终端跑起来的人看得懂该怎么做，不是对着一行 JSON 干等；核心的握手当场就回，10 秒等不到就是没有核心 | 一直等到标准输入读到头；照 `call_timeout_seconds`（那是等 NapCat 的数） |
| 23 | 状态文件照数据根算 `state/packages/onebot/status.json`，带桥的进程号；`status` 只在 `extension.status` 说在跑、进程号对得上时用它；变了才写，先写临时文件再改名；桥退出不删 | 核心给的工作目录就是这里，照数据根算的和它一样，在进程里跑的测试也不会写进源码树；进程号对得上，才不会把上一个进程留下的当成这一个的（刚拉起、还没写的那一下）；不删：崩了的删不了，删不删都靠进程号判 | 照工作目录写相对路径；不带进程号、照文件的时刻判 |
| 24 | `start`、`stop`、`restart`、`status` 连核心照 `miyu-onebot web` 的样子：没在跑就拉起 | 开关在核心那边，核心不在开不了也关不了；`status` 拉起核心时照开关拉起桥，和核心下一次起来一样 | 核心没在跑就说没在跑、什么都不做 |
| 25 | `start`、`restart` 说一句再照回应说它这时的样子（多半是「正在起来」），不等它握手 | 施工单「照回应说一句」；要看起来没有，`status` 一问就知道；等它握手要再定等多久 | 等到在跑或者停下再说 |
| 26 | `logs`：标准错误那一份有内容的先印，再印运行日志，两段各带一行标题；只有运行日志的不带标题、原样印；整份印、不截；`-f` 接着跟运行日志，文件变短了从头读 | `-f` 新写的行接在最后一段后面，看的人不会以为是标准错误；截多少由 `tail`、管道定 | 先印运行日志、再印标准错误；只印最后几行 |
| 27 | `-h`、`--help` 印用法到标准输出、退出码 0；用法改成 `miyu onebot …` 的写法，`serve` 只在括号里提一句 | `miyu help onebot` 转成 `--help`（9-2），帮助由包自己说；人敲的是 `miyu onebot …` | 照用法不对报退出码 2 |
| 28 | 测试：真核心（测试程序里的 `Core`）照出厂的清单拉起真的 `miyu-onebot`：程序硬链接到测试程序旁边（不拷：拷的时候开着写的句柄，同一个测试程序里别的测试这时起的子进程会带着它，接着拉起时 Linux 回 `ETXTBSY`）；在进程里跑的桥用内存里的管道，测试那一头把握手补上本机令牌、经本机套接字交给核心 | 和核心测扩展的办法一样，三个平台都成立；出厂的清单也跟着测到；核心「不看凭据」的入口不为测试公开，桥自己的代码不带凭据，由真进程的测试守着 | 测试里自己拉起、接管道；公开 `serve_spawned` |
| 29 | 核心拉起扩展时把 `MIYU_HOME`、`MIYU_RESOURCES` 设成核心正在用的数据根、资源目录（核心那边改，`extensions.md`「怎么走」第 1 条） | 测试里的核心跑在测试程序里，环境里没有临时的数据根，拉起的桥会去碰真的数据、也找不到资源目录；正式跑时也保证扩展和核心认同一份 | 测试里另起一个核心程序；测试程序带着环境变量把自己再跑一遍 |

**施工时定的**（O-19，2026-10-08；施工单「要定的」三条照推荐定：认不出的当普通的话交给她、命令和发消息同一个编号拼法、被拒的不交给她）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 30 | 斜杠命令写成第 7、8 条之间不占编号的一段（「斜杠命令」） | 后面第 8 到 12 条的编号在这一页别的地方、代码注释里引用着，插一条要跟着改一片，和这一步无关 | 插成第 8 条、后面的往后挪 |
| 31 | 被拒的那一句用核心拒绝的回应里的 `error.message`，桥的 `human/*.json` 不另配 | 核心拒绝的回应本来就照这个连接的语言写好（`protocol.md`「出错」：`message` 照握手时的语言），终端、网页说的也是这一句；桥另配一份，两边说法会不一样，核心加原因码桥还要跟着加 | 照原因码在 `human/*.json` 里配一句 |
| 32 | 认 `/` 照核心的认法：去掉开头的 Unicode 空白看头一个字；交过去的 `text` 照原样，命令名由核心认 | 桥只分「像不像命令」，认得哪些、别名、谁能用都在核心（`04-核心协议.md` P4）；加命令不用改桥 | 桥自己列一份命令名，只交认得的 |
| 33 | 只有 `unknown_command` 当普通的话；`bad_params` 照被拒发回核心那一句 | 施工单以为 `/` 后面是空白回 `bad_params`，核心其实回 `unknown_command`（`command.run` 第 1 条，核心的测试 `/ clear` 也是），施工单要的「`/ 你好` 照普通的话」照样成立。桥只交 `/` 开头的，核心对它回 `bad_params` 只剩一种：命令认出来了、后面的字不对（例如有记忆的会话里空的 `/remember`），这是给人的命令，该把「参数不对」告诉人，不该当话交给她。现在场所会话碰不到它，为它另写一个分支也测不到（施工时手写的变异照着这个分支改，没有测试逮得住） | 照施工单把 `bad_params` 也当普通的话 |
| 34 | 同一条命令重发：核心回的和头一次一样，回执照样再发一次 | 桥从回应分不出是不是重发；要分得记住发过的编号，多一份状态、桥重启又忘了；平台重发少见，多一句回执不碍事，命令本身只执行一次 | 桥记住发过的编号，重发的不回 |
| 35 | 回执、被拒的那一句和她的回话走同一条路（同一个机器人号的写队列），运行日志同样记 `reply sent`；成了、被拒记 `INFO`，不是命令记 `DEBUG` | 先后不乱：回执不会跑到她前一句回话前面；被拒（例如没东西可清）是人看得到的正常结果，不是桥的毛病 | 回执另开一条路发；被拒记 `WARN` |
| 36 | 交过核心、有了会话（成了、被拒、交了话都算）就记下这个会话的回话发给谁 | 回执、被拒的那一句要发回去；原来只在交了话以后记，第一条就是命令的私聊没处发 | 只在交了话以后记 |

**施工时定的**（测试的偶发红，2026-10-08，主会话派的，和 O-19 分开提交）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 37 | 真的程序起桥的测试（`spawned.rs`、`stdio.rs`、`no_token.rs`、`commands.rs` 记运行日志那一个）照 `support/ports.rs` 的 `on_free_ports` 跑：桥退出、标准错误说的正是挑的那两个端口被占了，这一组作废、换一组从头再来，最多 5 次；别的原因退出照旧当失败。等桥起来看状态文件的进程号是它的（两个端口都绑上才写），不看端口连不连得上。`/apply` 换到挑的空端口、回 409 说的正是它的，换一个再来；要「没人听的端口」的（`open.rs`）绑着不听、用完才放 | `free_port` 挑来马上放掉，过一阵真的程序才去绑；负载高时这个号被别的测试先拿走（进程里的桥照端口 0 让系统挑），桥说端口被占、退出码 1，测试偶发红。同时跑 6 份测试程序复现过（18 份里红 3 份，都是端口被占）。看端口连不连得上会把别人听着的当成桥 | 挑端口改成固定的号段：几份测试程序、别的会话同时跑照样撞；桥绑端口 0、测试读状态文件拿端口：`spawned.rs` 要测配置里写的端口 |

**施工时定的**（O-20，2026-10-09；施工单「要定的」三条照推荐定：握手回应里没有端口的照清单的默认值、`/apply` 施工时看（留着）、端口当场生效）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 38 | 握手回应里没有端口的（`null`、不是 0 到 65535 的整数的也算）照清单 `[settings]` 的默认值：起来时读资源目录里自己的清单（`miyu_store::packages` 出厂那一层的 `onebot`，`miyu_config::package` 读成样子），读不出来的起不来 | 核心交的是最终值，正常一定有；真没有就照默认值，不另起一套数，清单是唯一的那一份 | 起不来；在 `bridge.json` 里另写一份默认值 |
| 39 | `/apply` 留着，改成「照桥手里最新的配置换端口」：推送来了桥已经照它换了的，`/apply` 再照一次什么都不换，回实际听的两个端口；推送来时新端口被占、没换成的，`/apply` 再试一次，还被占回 409 和是哪个 | 页面存了端口要等换完的结果：WebUI 的端口换了要跳到新地址、被占了要说是哪个；只看 `/status`，页面得自己猜什么时候换完，页面这一步不改（`resources/software/onebot/web/` 不动）。页面的 `config.set` 回来以前，核心已经换上新的一份配置、叫醒了推送；回应还要经页面、浏览器再回到桥，推送先到。WebUI 的端口被推送换掉以后，页面这一次 `/apply` 走旧地址，回 403（Host 照新端口核对）或者连不上：页面照刚存的端口跳（第二条「施工时定的」第 37 条，无头浏览器里试出来的） | 去掉 `/apply`，页面照 `/status` 看换没换完（页面要改，还要猜等多久） |
| 40 | 两个端口的生效时机改成 `now`：推送来了当场换，照 `/apply` 的办法 | 桥收得到推送，不用再说「下次启动时」；命令行改的也当场换 | 照旧 `head_start` |
| 41 | `/status` 的 `token` 只剩 `set`、`none`：没写引用、引用取不到，核心都不交（`extensions.md`「配置」第 2 条），桥分不出 | 照核心交的说，不另要；页面原来照 `missing` 说「引用的取不到」，现在两种都说「没设」，`missing` 那一支和 `web/token/missing` 一起删掉（页面照 `config.get` 里写没写引用分得出，要分的时候再加） | 请核心把取不到的另交一个记号（核心的形状要改） |
| 42 | 握手以前不读配置、不说话：起不来的照系统的语言说一句；运行日志装不上的那一句等握手回了语言再说；握手回了语言就照它说，端口被占那一句也是（`run` 多一个参数：握手回了语言叫一声） | 核心拉起的桥，握手以前没有配置可读；O-19 时发现端口被占那一句照的是握手以前的语言（`main.rs` 只在「听上了」那一刻换） | 只在「听上了」那一刻换（原来的做法）；端口被占那一句自己带上语言 |
| 43 | `miyu-onebot web` 照状态文件的 `web` 找桥的网页，没有状态文件的照清单的默认值；`open/not-running` 去掉「改过 onebot.web 的，miyu onebot restart」 | 桥不再自己读配置；状态文件写的是桥实际听的端口（推送、`/apply` 换过的照换过的），比配置里写的准（新端口被占、没换成的，配置里是新的、桥还在旧的上）；端口改了当场换，不用 `restart` | 经核心的 `config.get` 问 `onebot.web`（要先连核心，桥没在跑也拉起核心；被占没换成时问到的不对） |
| 44 | `start`、`stop`、`restart`、`status`、`logs` 握手以前照系统的语言，不再读 `ui.language`；系统的语言照 `zh`、`ja` 开头的认，别的英文，和核心照 `locale` 算的一样，写在 `texts.rs` | 桥不读配置；`start` 这几样握手以后照核心回的语言说（照旧），握手以前只有连不上核心那一句；`logs` 只说标题和「还没有」那一句 | 经核心问语言（`logs` 不连核心） |
| 45 | 推来的 `extension.config` 由跟核心的那一头（`core/route.rs`）交给 `serve.rs`：令牌当场换上；两个端口变了另起一个任务照 `/apply` 的办法换 | 拿着跟核心的连接的只有那一个任务，它够不着监听；换端口和 `/apply` 是同一段代码（`web/apply.rs` 的 `latest`），同时来的几次锁着一个一个办，每次照这时手里最新的 | 跟核心的那一头自己换端口 |
| 46 | 两个端口的说明去掉「在 QQ 桥的网页上改、保存的，当场生效」，别的照核心那一份原样搬 | 生效时机改成 `now`，参考文件自己写「当场生效」；只说网页上的，像是命令行改的不当场生效 | 一字不改地搬过来 |
| 47 | 测试：测试里的核心照出厂的清单拼进包的配置项（`miyu-core` 的 `Packaged`，和真核心起来时一样；`miyu-core` 挪进开发依赖）；进程里跑的桥，测试那一头的转接在握手的回应里填 `config`，要改配置的照推送的样子写 `extension.config`；真核心拉起真桥的测试守着核心真的交、真的推 | 握手交配置、推送只给核心亲手拉起的连接，进程里跑的桥连的是本机套接字，核心不给；转接照样子填，桥的代码走的是同一条路 | 核心为测试公开「当成亲手拉起的」入口 |
| 48 | 依赖照 `cargo metadata` 查（测试 `dependencies.rs`） | 施工单验收第 1 条；分层门禁只管层，第 5 层依赖第 4 层是允许的，和门禁读同一份 | 改分层门禁（全仓的规则，不为一个包加） |

**施工时定的**（桥：陌生人的私聊照旧不接，2026-10-09 和核心的主会话定）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 49 | `venue.session` 回应带 `account`、等于桥自己的账号（握手回应的 `account`）的私聊是陌生人：照 `no_system_account` 不接，同一个人只记一行，会话编号不记进缓存、不订阅；没带 `account` 的照常接（第 7 条） | 核心 O-4 中（系统账号）以后，陌生人在 `venue.session` 不再被拒，照常造会话（属主是系统账号 `onebot`）；进站链要到接群那一步才接进桥，这期间私聊不能敞开，只接主人。核心等桥这一处进了 main 再合 O-4 中；没带 `account` 的照现在办，桥这一处不依赖核心先做 | 等接群那一步再管（这期间陌生人的话直接交给她）；记进缓存（下一条不再问，陌生人后来成了主人也认不出） |

**施工时定的**（O-21，2026-10-09；施工单「要定的」三条照推荐定：用的时候看一眼修改时刻、系统的写错了只丢坏的、系统的违规词表放 `system/modules/onebot/`）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 50 | 系统那一份读不成的（超过 1 MiB、不是 UTF-8、读不了）照空的用：规则文件照样替换同名的出厂那一份、自己没有规则；违规词表照空的；问题照样报 | 替换看的是同名的文件在不在，和群聊内核对 TOML 写法不对的办法一样（整份不用，出厂那份也不读）；配置起来时读不好的也照空的（`config.md` 的 `using-nothing`） | 读不成的当没有（出厂那份顶上来：同一个文件写法不对和读不成结果不一样）；照上一次读好的用（要一份一份记着，重读又是整份的） |
| 51 | 规则文件：`venues.d/` 里名字以 `.toml` 结尾、不以 `.` 开头的普通文件（跟着链接）；名字不是 UTF-8 的不认 | 编辑器的锁文件（`.#80-x.toml`）、隐藏文件不当规则；名字要按字节排、要印出来 | 收所有 `.toml` |
| 52 | 出厂的起来时读一次、单独查一次，放在内存里，跑着不再读、不看变没变；有问题握手以前说 `failure/factory` 和每一条问题、退出码 1 | 出厂的随包装好，跑着不会变（换包要重启桥）；系统同名替换以后出厂那份不读，单独查才查得全；和 `bridge.json` 一样是资源，握手以前读 | 出厂的也看修改时刻；出厂的问题只说一句「起不来」带第一条 |
| 53 | 变没变照文件列表、每一份的修改时刻和大小比（`venues.d/` 列不出的记目录本身）；隔多久看一次是 `bridge.json` 的 `rules_check_millis`（出厂 1000）；用的一方交进当时的时刻 | 大小顺手一起比，修改时刻粗的文件系统上同一秒里改了多半也看得出；数是数据；交进时刻，测试不用等一秒 | 只比修改时刻；写死 1 秒；里面自己取钟 |
| 54 | 起来时（握手以后）读一次系统的、问题记运行日志；桥里还没有别处用它 | 施工单要「桥把它们读进来」，写错的规则起来就看得见；用到它们的进站链、主动回复判断、出站随接群的几步 | 等用到的那一步再读 |
| 55 | `venue show`：只印规则设到的项，照键排，值照 TOML 写，接一句没列出的参数照 `defaults.toml`；问题印在标准输出上、在后面；有问题也是退出码 0；场所编号认不出退出码 2（用法不对）；出厂的有问题照起来时那样说、退出码 1；违规词表不印 | `Params` 交的是换算好的格（毫秒、字符），印不回原文；`Resolved` 照键排好了；问题是看的内容的一部分，像 `udevadm info`；出厂的有问题桥起不来，套出来的也不对；词表不分场所 | 参数三十几项都印；问题印在标准错误上、有问题退出码 1 |
| 56 | 问题说成话：在哪（出厂或系统、文件、第几条规则、第几行）加错在哪，一种原因码一句（`problem/…`），出厂参数缺了的另一句；不照配置的 `tell` 说 | `tell` 要配置清单的项才说得出期望什么，规则的属性、参数的声明不在配置清单里（群聊内核里是私有的）；原文 `got` 已经够人看出错在哪 | 照 `tell` 说（要群聊内核交出每一项的类型）；只印原因码 |

**施工时定的**（O-22，2026-10-09；施工单「要定的」三条照推荐定：QQ 的群主、管理员不算 `manager`，@ 的名字照群成员的缓存、拿不到写号，合并转发、卡片写占位）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 57 | 群消息、撤回写成第 12 条后面不占编号的两段（「群消息」「撤回」） | 同第 30 条：第 8 到 12 条的编号在这一页别处、代码注释里引用着 | 插成编号的几条 |
| 58 | `role` 只照场所规则的 `managers`；QQ 的群主、管理员不自动算 `manager` | `manager` 在核心里管的是谁能用 `/clear` 这类命令；群管理员能清她的上下文不对（施工单「要定的」第 1 条） | 群主、管理员自动算 |
| 59 | @ 的名字照群成员的缓存：每条群消息的发的人顺手记下，缺了的问 `get_group_member_info`（`no_cache: false`），记 `member_names_seconds`（出厂 600 秒，照旧版）；一条里调不成一次，后面的不再调、写号 | NapCat 的 `at` 段只有 `qq`（查过 NapCat 4.x 的代码，带名字那一行注释掉了），名字得另取；发过言的人最常被 @，顺手记下省一次调用；跟核心的那一头一条条照先后办，NapCat 一卡，一条 @ 十个人就要等十个时限 | 一律写号（施工单「要定的」第 2 条没选）；一律问 NapCat、不记 |
| 60 | @全体成员正文里不写，只记 `mentions_all` | 核心照这一格在 @ 那一行写 `@all`；正文里写就要一句「@全体成员」的字，又是一份要配语言的字，她也会看到两遍 | 正文里写 `@全体成员` |
| 61 | 合并转发、卡片的占位是 `[forward]`、`[card]`，照核心渲染带的东西的写法 `[种类]` | 是一行里的格式记号，和核心写在代码里的 `[image]`、`[msg=…]` 一样，不是说明的话，不进 26 第十节的登记簿；她看得出这里有一样东西、是什么种类（施工单「要定的」第 3 条） | 写中文的「[合并转发]」；整段不记 |
| 62 | CQ 字符串只读字（第 6 条的办法），@、引用、带的东西不认 | 施工单「字符串格式照现在私聊的办法也认」；NapCat 照这一页配成数组格式，字符串格式只是兜底 | 把 CQ 码也拆成段 |
| 63 | 场所的格照核心的写法洗：名字去掉控制字符、截到 64（带的东西的名字 200）个字符，空白的不写；引用、带的东西的编号不合（空的、超过 128 个字符、有控制字符）的那一格、那一样不记，消息照记 | 核心收到写错的格整条 `bad_params`、什么都不记（`venues.md`「场所的格」）：一个怪名字、一个长编号不能让整条消息丢了 | 原样交，让核心拒 |
| 64 | 群会话不订阅（O-23 改：从头订阅，第 71 条）；斜杠命令的回执、被拒的那一句直接经 `send_group_msg` 发回群 | 这一步群里只记旁听、不开回合，没有她的回话要发；她在群里说话随出站链（O-24），到时候再订阅 | 订阅了把她的回话直接发进群（绕过出站链） |
| 65 | `asleep` 照群聊内核的回合闸问：`gate()` 只交睡眠，闸说推迟就是睡着；此刻照桥的钟，时区照本机此刻的偏移（`jiff`，和核心造会话时钉下时区的办法一样） | 内核的 `Sleep` 只能从原文读，格不公开，判睡没睡的那一处在闸里；不为这一步改群聊内核。桥和核心在同一台机器上，核心钉下的时区就是这台机器的；`jiff` 本来就在依赖图里（`miyu-log` 用它） | 桥自己算一天里的第几分钟（第二份算法）；照事件的 `time` 算（可能没带、是 0） |
| 66 | 规则写错（人格、预设不存在、预设写错）：同一个群只记一行运行日志，桥起来以后只记一次，和私聊的陌生人同一张「记过的」（键从号换成场所编号） | 一个群每条消息记一行，日志会被刷满；改好以后消息照记，看得出来 | 每条都记；改好又改坏了再记一行（要多记一份状态，换来的只是少见的一行日志） |
| 67 | 只有带的东西、没有字的群消息照样交 `text: ""`：O-22 时核心回 `empty_message`、记一行 `WARN`，核心 O-13 补以后收了，记成没有内容块的 `message.user`；私聊的照旧不送 | 核心的 `session.send` 字是空的就没有内容块，内核拒空的消息，不看 `venue.media`；只有图的群消息是最常见的消息之一，要核心改（2026-10-09 主会话定：转给核心的会话，核心改了桥不用改）。私聊每条都开回合，只有图的私聊交给她要先能看图（平台工具那一步） | 桥塞一个空格（内容块里留一个空格，是 hack）；桥写「[图片]」（和核心渲染的 `[image]` 重复） |
| 68 | 私聊带上场所的格，不写 `ambient`、`asleep`；`venue.session` 的人格、预设、工作区只给群交 | 私聊每一条都开回合；施工单只给群写了 `persona`、`preset`、`cwd`，主人私聊的会话怎么造这一步不动 | 私聊也照规则交人格 |
| 69 | 机器人自己发的（`user_id` 等于 `self_id`）私聊、群消息都不看 | 她说过的话由出站记 `venue.delivered`（O-24）；NapCat 开了「上报自己发的消息」时会收到 | 只管群的 |
| 70 | 撤回的命令编号自己编（带随机前缀的那一种），撤的人照 `operator_id`；撤她自己的也记 | 平台不重发通知；谁撤的就是 `operator_id`，撤的是谁的由核心照 `msg` 找到那一条 | 照消息编号拼编号去重 |

**施工时定的**（O-23 上，2026-10-09；施工单「要定的」四条照推荐定：主人照核心记下的 `by` 认、只有主线、当没被禁言、要问判官的两条路只记判断）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 71 | 群会话找到了就订阅、写 `after: 0`；补来的、推来的走同一条路收进投影，序号不大于回应的 `upto` 的她的话不发 | 投影都从日志来（施工单），协议有现成的补发，不另要取历史的办法，桥也不读核心的存储（`01-架构.md` 第五节第 1 条）；补来的不发，桥重启不重发旧的回复 | 读会话目录里的日志文件；`session.page` 按轮翻（旁听的消息不成轮） |
| 72 | 判以前先把这个会话留着没办的推送收进投影：`Core` 交出留着的这个会话的事件推送，别的照旧留着 | 发的人是不是主人要看核心刚推来的那条 `message.user`；刚开的回合、刚记的提示、刚并进去的也都在里面，限流照这一刻的算。核心先推后回应，回应到了它们就在留着的里面 | 等主循环下一圈再判（判的时候投影还缺这一条）；另问核心这一条的 `by` |
| 73 | 自己人照 `onebot.trusted`：握手交来的读一份放在跟核心的那一头，推来的换；字的列表，别的当空的，列表里不是字的不要 | 施工单「要定的」第 1 条只管主人；`onebot.trusted` 是包的配置，握手交、推送换（O-20），和端口、令牌一样不读盘 | 桥自己读系统配置 |
| 74 | 不调 `dispatch`：开一轮、并进去、排队都是同一个 `session.respond`，核心照有没有在跑分 | 施工单「要定的」第 2 条：只有主线时三种结论桥做的是同一件事，调了不改结果，是用不到的代码 | 调了记进判断 |
| 75 | 不看 `discipline`、`parallel`：群一律照第三条的加值项和走路判（`discipline` 的那一半 O-23 下作废，第 87 条） | 这一步能回的只有主人冲她来，各种规程结果一样；别的规程、支线随用到它们的步子 | 现在就分规程 |
| 76 | `ext.onebot.chat.decided` 的格是「群里怎么叫她」第 7 条那张表：判的几条、发的人是谁、进站链的结果和原因、旗、条件和加分、走的路、结论；名字照群聊内核的类型写成蛇形的字 | 施工单第 7 条要的几样，加进站链的结果：限流、睡着、提示也是判断；`standing` 是走路的输入，自己人是当时的配置，事后算不回来。判官的回答、算分的每一项、用的模型、耗时随 O-23 下加 | 只记结论 |
| 77 | 记判断、开一轮、记提示的命令编号是这条消息的命令编号加 `/decided`、`/respond`、`/queued`；`venue.delivered` 的自己编 | 核心照命令编号去重：同一条消息至多一次判断、一次开回合、一次提示，看日志的 `cause` 也认得出是哪一条消息的；发出去的话没有平台给的编号可拼 | 都自己编 |
| 78 | 重发的（`session.send` 回的序号在收留着的推送以前就在投影里）不再判 | 第 77 条只在编号一样时管用；投影变了以后再判结论会不一样（头一次只记下，第二次开了一轮），同一条消息只判一次 | 交给核心照编号去重 |
| 79 | 只有表情：正文空白、带的东西都是表情；只有带的东西：正文空白、带了东西（图、文件、表情都算） | 照 `Facts` 两格的意思：表情不算字，刚说过话不算它；没有字的不抽样 | 只有图才算只有带的东西 |
| 80 | 限流的提示是 `human/*.json` 的 `group/rate-limited`，照握手回的语言说；那种语言的字读不懂的照系统的语言（`main.rs` 那一头也是照原来的说） | 给人看的字（施工单）；说话的人不是她，不用第一人称 | 写在代码里；写成她的口吻（人格侧的字） |
| 81 | NapCat 回的 `message_id` 当 `venue.delivered` 的 `msg`；回的里没有的不记，记一行 `WARN` | 核心照 `msg` 认「引用她」，`msg` 必写；编一个对不上平台的编号，引用她就认不出 | 自己编一个 |
| 82 | 掉了队照收到的最后一条再订阅，补来的照样收进投影，她的话不发 | 投影不缺；补来的回话不补发（O-25 中，「出站队列」第 6 条），桥连得好好的不会掉队（读的一头一直在读） | 从头再订阅；补来的照样发 |
| 83 | 投影不剪：每条人说的话记序号和发的人，每个回合、每轮回复、每个提示都记 | 都是从日志算得出的，桥重启重建；一条几十个字节，核心载入会话本来就整份读进内存；剪了以后很早的消息被当触发时认不出发的人 | 照时间窗剪（窗口是按场所改的参数，剪多了要再读） |
| 84 | 等 NapCat 回应的任务（回话、回执，私聊、群都算）的结果照放进写队列的先后交回来（`FuturesOrdered` 收着各个任务），`venue.delivered` 照发的先后记；任务的崩照旧交上去，放下跟核心的那一头时不掐它们（只等回应、记日志，连接断了、到了时限自己就完） | NapCat 并着办动作，回的先后不一定照发的先后；原来一个 `JoinSet`、谁先完先交，拆成两段的一句话 macOS、Windows 的 CI 上记反过（第二段先记），核心照日志的先后画她说的话，就画反了 | 一句话一个任务、里面几段照先后等（几句话之间照样会乱）；记的时候照编号重排（多一份状态） |

**施工时定的**（O-23 下，2026-10-09；施工单「要定的」三条照推荐定：放下在判的请求不真的取消、回来的回答丢掉，判官的全局并发和排队等多久放 `bridge.json`，违规时给她看的预检结论这一步不做）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 85 | 判官的说明算出厂数据：和场所规则、出厂参数、违规词表一起在 `Factory::load` 里读、查，写坏了是打包的错，桥起不来 | 施工单第 1 条；同 O-21 出厂数据的做法，有错一起报；`violations.txt` 写坏了不等到第一次问判官才发现 | 第一次问判官时读 |
| 86 | 跟核心的那一头加一个并着发的调用口（`Caller`）：同一条连接，问判官的任务各自发请求，回应照编号分（编号是跟核心的那一头的前缀加 `-side-` 和序号）；等不到了的（超时的、放下的）回应来了丢掉；读的一头停了，等着的都交回断开。核心原来在一条连接上一条条答 `model.call`，判官一次几十秒，这段时间桥的 `session.send` 都排在它后面；施工时报给主会话，核心改成在后台答（施工 8-20 补，2026-10-09，照 `link.preview` 那一路），一个连接上不设上限，全局 4 个照桥这边限 | 桥只有核心给的这一条连接，另开要本机令牌、身份就成了管理员；判官要并着问几个、不挡别的消息 | 桥另开连接；判官在跟核心的那一头里一条条等 |
| 87 | 线路规程照「群里怎么叫她」第 10 条那张表分开走，写在桥里（`discipline.rs`），不进群聊内核 | 施工单只改 `onebot.md`；四种里只有 `chatty` 用得上判官，别的只是留哪些条件、走哪条路；别的平台的桥要用时再挪进群聊内核 | 进群聊内核 |
| 88 | 没设 `discipline` 的群照 `chatty` | 出厂的群规则就是 `chatty`；O-23 上不看它时群一律照 `chatty` 判，没设的行为不变 | 照 `when-called`；报错 |
| 89 | `wake` 的「回复以后一小段时间内不用再叫」照续聊算（她刚回过的人、续聊的窗口），不照刚说过话（谁都算）；`wake` 不看顶替，`when-called` 看（18 第七节那张表只给 `when-called` 写了「同一个人接连发的几条」） | 叫醒她的是那个人；照刚说过话，热闹的群里她每说一句就给所有人开 30 秒的门，一句接一句停不下来 | 刚说过话 |
| 90 | 不是 `chatty` 的不抽样、不打分；`when-called`、`wake` 里只有违规旗的问判官只查违规，严重程度够了才回，和别的条件一起的照原来的走；`every-message` 不变，有字的照开一轮，违规旗只记进判断；都不带预检结论（2026-10-09 主会话审过以后改，原来是违规旗直接开一轮） | 18 第七节「违规审核」：关键词只能把判官拉起来，不能直接定违规（旧版 `OD` 一个词 7 天误报 447 次）；`every-message` 她回是因为每条都回，违规旗本来就没在定回不回，问了判官反而让带误报词的一句比普通的话更难被回；预检结论是给模型看的字，要实测、登记，另起一小步（施工单「要定的」第 3 条） | 违规旗直接开一轮；只记下；`every-message` 也问判官 |
| 91 | `Committed` 的口径是「判过要回、她还没回完」：从判断（回）记下起，到收了它的那一轮 `turn.ended` 为止，还没进哪一轮的照旧算 | 桥先记判断、紧接着 `session.respond`，核心当场记 `turn.started` 或 `turn.joined`；照「还没进哪一轮 `triggers`」，`Committed` 只存在一瞬，`Inherit` 走不到，「@ 她、3 秒后补一句没 @ 的」那一句她这一轮听不到（2026-10-09 主会话定，`chat.md` 第七条第 4 条同时改） | 照字面：还没进哪一轮 `triggers` |
| 92 | 几条一起判的，判官看的「这一条」是最后一条，前面几条在群聊记录里 | `venue.records` 只交一条的 `current`；前面几条就在它前面，判官看得到；要几条都当「这一条」得每条调一次再拼 | 每条调一次、拼起来 |
| 93 | `venue.records` 的 `count` 照 `judge.records` 原样交；群聊内核那一项的范围改成 1 到 100（`chat.md` 第八条，2026-10-09 主会话审过以后改，原来桥里把 0 照 1、超过 100 照 100 截） | 核心只收 1 到 100（`venues.md`「判官看的群聊记录」第 1 条）；范围对上了，参数写的就是判官看的，桥里不用另写一段截 | 参数照旧 0 到 1000、桥里截 |
| 94 | 重试：等不到、核心拒了（不分原因码）、读不出都再问；排队等不到、`venue.records` 被拒的不再问；记进判断的是最后一次的为什么 | 最朴素：判不了就再问，`retries` 出厂 1 次，多问一次花不了多少；排队等不到再排一次也一样，`venue.records` 被拒是这一条本身不对 | 照原因码挑着重试 |
| 95 | 放下的、等不到回答的判官请求，桥这边的任务照样等到回答（等不到的到时限）才放名额，回答丢掉 | 施工单「要定的」第 1 条；名额管的是同时在问模型的有几个，核心那边没取消，放了名额就多问了 | 放下就掐掉任务、放名额 |
| 96 | 耗时 `millis` 从交给判官（排队以前）算到有结果 | 排队也是在等判官；看日志的人要的是「这一条判了多久」 | 只算调 `model.call` 的 |
| 97 | `ext.onebot.chat.decided` 多四格：`discipline`、`supersede`、`judge`、`score`（「群里怎么叫她」第 7 条那张表）；`judge` 里不记判官回的原文 | 施工单第 7 条要的几样；读得出的已经拆成格，读不出的记为什么就够查，原文会撑大一条日志 | 记原文 |
| 98 | 额度满了：主人冲她来照回（不过判官），自己人冲她来只记下 | 18 第六节「超了的那段时间，线路规程不抽样、不调判官」；自己人不受限流是进站链放行，判官照样不调 | 自己人照回 |
| 99 | 判官在判的时候同一个人又说了一条、要顶替的：桥这边先拿掉在判的那一条，判断等新的一次判完才记，只记一笔（`msgs` 是几条，命令编号照最后一条拼） | 一条消息只判一次（第 8 条）；放下的那一次没有结论 | 放下的那一次也记一笔 |
| 100 | 算分的此刻是判官回来的那一刻；参数照交给判官时套的那一份 | 判官可能跑几十秒，冷静照她这时的近期发言量；参数中途改了，同一次判断照同一份 | 照交给判官的那一刻 |
| 101 | 核心为并进去的那几条接着开的一轮，`turn.started` 只有指向那条 `turn.joined` 的 `trigger`：投影照它找回 `triggers` 的发的人（2026-10-09 主会话审过以后加） | 那一轮回的就是并进去的人；不找回，`venue.delivered` 的 `to` 是空的，续聊认不出（真跑时看到的）。日志里本来就有，不用核心多记一格 | 要核心在那一轮另记 `triggers` |

**施工时定的**（O-23 补，2026-10-09；项目主人定：判官也带人格，给一个开关，默认开，经核心的主会话转达）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 102 | 人格照订阅回应的 `persona` 认；原文经 `persona.read {persona, prompt: "persona"}` 读现在文件里的那一份；`judge.persona` 关了的、无人格的不带、不读。量过：带样本人格 `engineer`（测试用的那一份，人设一句），判官请求多 15 个 token（`persona-open`、人设那一句、`persona-close` 接起来，2026-10-09 在开发端点的 `deepseek-v4.1-flash` 上量，两次一样）；真人格照它人设的长度多（旧版 Miyu 的人设 1403 个，`26-提示词.md` 第五节） | 施工单第 1、2 条：订阅回应就是会话实际用的人格，场所规则没写的照核心的默认人格，桥自己算不出；`persona.read` 是核心现成的。不加给模型看的字：标签 O-11 就登记了，人格原文是人格自己的 | 照场所规则的 `persona`；照会话快照里冻着的（要核心另开口） |
| 103 | 读到的原文记 60 秒（`bridge.json` 的 `judge_persona_seconds`），几个问判官的任务共用一份；读不到的不记 | 施工单第 4 条：同一个人格一分钟里读一次就够，热闹的群一分钟里能问判官好几次；人格改了最多晚一分钟。照群成员名字的缓存放 `bridge.json`（第 59 条）：是桥这个进程的，不按场所改。读不到的不记：人格恢复了下一次就带上；日志一次一行，判官的次数有限流、名额压着 | 不记、每次读；记到桥重启；读不到的也记一阵 |
| 104 | 读人格在问判官的任务里，拿到群聊记录以后 | 跟核心的那一头不等它（同第 86 条）；记录被拒的不用读 | 交给判官以前在跟核心的那一头读 |

**施工时定的**（测试工具：同时链桥的程序，2026-10-09，O-25 上顺手修，和 O-25 上分开提交）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 105 | 测试程序旁边链 `miyu-onebot`（第 28 条）：已经是同一个文件的不动；不是的先链到旁边一个只有这一次用的名字，再改名盖上；链、改名的结果都不看，最后是同一个文件就成了，不是的才失败 | 几份测试程序并着跑时同时去链：原来先删再链，后到的撞 `File exists`，或者把别人刚链好的删掉、那一刻没有这个文件，测试红（O-23 下撞过）。改名是原子的，那个名字一直在，谁最后盖上的都是同一个文件 | 加锁文件排队链；先删再链、撞了当成功（八个线程一起链，几十轮里还会删掉别人刚链好的） |

**施工时定的**（O-25 上，2026-10-09；施工单「要定的」三条照推荐定：一轮几条触发的引用最后一条，引用和 @ 只带在第一段，丢了的这一步只记运行日志）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 106 | 她回的那一条照投影：`turn.started` 的 `triggers` 的最后一条，`turn.joined` 并进来的换成它的最后一条；接着开的一轮照找回的那几条（第 101 条） | 施工单「要定的」第 1 条；和核心记 `tool.call` 的 `by` 一个取法（`providers.md`「是谁要的」），她这时看到的最新那一条才是她在回的；一条消息只能引用一条 | 引用第一条；不引用 |
| 107 | 丢了的不发、不入队、不另记事件，运行日志一行 `INFO reply dropped venue=… why=… chars=…`，不记原文 | 施工单「要定的」第 3 条；O-25 中入队的只有过了链要发的（「出站队列」第 1 条），丢了的没进过队，没有结局可记；正文不进运行日志（第 12 条） | 记 `ext.onebot.venues.*`（多一种状态，没人读） |
| 108 | 实际要带的引用、@ 只带在拆出来的第一段；@ 段后面跟一个只有空格的文字段 | 施工单「要定的」第 2 条：每段都带，群里刷屏；旧版也只带第一段。@ 段和后面的字挨着画，旧版发一个空格隔开，客户端不一定自己隔 | 每段都带；空格接在字前面（`venue.delivered` 记的字就和发的不一样了） |
| 109 | 引用的编号、@ 的号照原样写成字：编号是 `message.user` 的 `venue.msg`，号是发的人的平台身份 `qq:<号>` 的后一段 | 旧版的教训：引用的编号原样还回去，自作聪明换写法的，对端不声不响地丢掉引用；OneBot v11 段的数据本来写成字，NapCat 两种都收 | 写成整数 |
| 110 | `Since` 的三格：`others` 照序号数她回的那一条以后的人说的话，发它的人的不算；`elapsed` 是本机此刻减那一条的 `at`；`last_is_own` 照序号比她发出去的最后一段和最后一条人说的话，她的一段照她说它的 `message.assistant` 的序号。都照投影，纯逻辑 | 都是日志里有的，桥重启照样算得出；`at` 是核心记下的时刻，和本机的钟是同一台机器的；回执要等平台答，发出去到回执之间进来的人话在日志里排在回执前面，可群里她那一段在前，照回执比会多带一个引用（CI 的 macOS 上撞出来的，2026-10-09） | 照平台的消息编号比先后（QQ 的编号不保证递增）；桥自己数；照 `venue.delivered` 自己的序号比 |
| 111 | 她一轮里连着说的后一句（群里最后一条是她自己的）照出站链的规矩带引用，不另改 | 群聊内核的规矩（`chat.md` 第五条第 4 条，旧版同样）：她连着说话时靠引用分清在回谁；真跑看得到，测试照它写 | 一轮里只有第一句带 |
| 112 | 群里这一回合已经发出去的照投影里这一轮入队了的她的话（`ext.onebot.venues.queued` 里 `kind` 是 `reply` 的正文）：桥入队记成了就先算进投影，日志推来的同一段照正文认（这一轮已经有一样正文的不再加），桥重启照日志补。拆成几段的照段比，整句重复比不出来（出厂一段 3000 个字符，很少拆）。`superseded.rs` 里她连着说的后一句带不带引用不看（剧本说得快，前一句回执记没记下不一定，引用照回执算，那条测的是回给谁）。O-25 上照 `venue.delivered`、前一句 NapCat 还没回的不算，是过渡，O-25 中收掉 | 施工单 O-25 中「要定的」第 5 条：日志推来的入队事件可能比她下一句话晚到（她说得快），只靠日志会漏；先后照入队，没有「前一句 NapCat 还没回、下一句已经来了」的空当；正文一样的两段对去重是一回事 | 只照日志推来的（还有空当）；照 `venue.delivered`（O-25 上的过渡） |
| 113 | 私聊这一回合已经发出去的照桥这一轮自己入队了的那几段（`Spoken`：入队记成了就记上，内存里一个会话一份，换回合就清，桥重启就丢）；私聊也照 `plain`、`split`（这个私聊套出来的 `Params::split_chars`）拆段，不记 `venue.delivered`（2026-10-09 主会话定；O-25 中改成入队时记，施工单 O-25 中「要定的」第 4、5 条） | 私聊的订阅不补从前的（「怎么走」第 9 条），照日志算不了；私聊没有 `venue.delivered`，记了会改她私聊里看到的渲染 | 私聊不去重；私聊也从头订阅（多补一遍私聊的整个日志）；私聊也记 `venue.delivered` |
| 114 | 命令回执不过出站链；群里撤的是斜杠命令发回去的那一句（成了的回执、被拒的那一句），限流的提示不撤；撤是另起的任务（等几秒、撤），撤的时候照那个机器人号那时的连接；几秒放 `bridge.json` 的 `receipt_recall_seconds`（出厂 3）。O-25 中：回执和别的一样入队，等 NapCat 回应在照先后交回的那一串里，回了编号才另起撤的任务（第 119 条） | 回执是核心写的，不是她的话；被拒的那一句也是机器人的内部状态（18 第十节「群里不留机器人的内部状态」）；等几秒放进照先后交回的那一串，她后面的话记 `venue.delivered` 要跟着等 3 秒；中间断了重连的照新连接撤；秒数是桥这个进程的，不按场所改（和判官的并发一样，`chat.md` 第八条施工时定的第 5 条） | 被拒的不撤；放进 `sending`；按场所改 |
| 115 | 测试的模型替身另写一个（`support/speaking.rs`）：一次回复里能既说一句、又调一件不存在的工具（内核当场答「没有这件工具」、接着请求），能等测试放行再说 | 去重要「同一回合说两句」，剧本（`Script`）的一次回复要么只说话、要么只调工具；引用、@ 要她还没开口时群里先说几句、等够时候。核心的测试替身不动 | 改核心的剧本 |

**施工时定的**（O-25 中，2026-10-09；施工单「要定的」六条照推荐定：排着的 60 秒过期、放 `bridge.json`，全员禁言不认，桥重启时入队了没结局的不补发，私聊不记 `venue.delivered`，去重入队记成了就算上，禁言的复查随 quirk 那一步）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 116 | 出站队列分两块：排着、过期、禁言到期、回应算成什么是纯逻辑（`route/queue.rs`，钟由调的一方交进来，停住的钟测）；入队、交出去、记结局的（`route/sending.rs`）、记禁言的（`route/muted.rs`）跟核心、NapCat 打交道；`route.rs` 只登记、接线 | 一个模块一种职责，纯逻辑好测；`route.rs` 本来就快 400 行 | 写进 `route.rs`；各条小路（回话、提示、回执）各自排 |
| 117 | 禁言只记在投影里（照日志算），队不另记一份；门开没开当场照投影和连着的号算 | 一份账：桥重启照日志重建，不会两份对不上；进站链的 `Ctx.muted` 和出站照的是同一份 | 队里另记一份禁言到什么时候 |
| 118 | 时刻都照本机的钟（`Timestamp`）：入队的时刻、过期、禁言的 `until`；定时照最早的那一刻换算成时长睡 | `until` 要写进日志（绝对的时刻），和事件的 `at` 一种写法；一种钟好算，和 `asleep`、`elapsed` 一样照本机的钟 | 过期照单调钟（两种钟混着算） |
| 119 | 回执也在照先后交回的那一串里等回应，回了编号才另起撤回的任务（只等几秒、撤），第 114 条跟着改 | 回执也要记失败，结局走同一条路交回来；等回应是毫秒级，会卡住后面的是那几秒的等，那一截照旧另起 | 回执的等回应和撤回一起另起（失败要另一条路交回来记） |
| 120 | 失败的原因照调用的错分：回了失败是 `rejected`，`detail` 照回应的 `message` 去掉首尾空白（空的不写），截到 200 个字符；等不到是 `timeout`；写不进、断了是 `disconnected`。`CallError::Failed` 改成带回应原文（JSON），不再写成字 | 施工单「出站队列」第 4 条；`message` 是 NapCat 给人看的原因；带原文才取得出它 | 照 `retcode` 分；`detail` 写整个回应 |
| 121 | `group_ban` 只认禁的是她的；`duration` 读法同号（整数、写成整数的字），没带的、负的不认；0 当解禁 | 施工单「要定的」第 2 条；NapCat 解禁有的报 `lift_ban`、有的报 `ban` 带 0 | 没带 `duration` 的当一直禁着 |
| 122 | 禁言的 `until` 照收到通知时本机此刻加 `duration`，不照事件的 `time` | 判禁言、定时都照本机的钟（第 118 条）；平台给的时刻是另一台机器的，差几秒就早解禁或者多排几秒 | 照事件的 `time` 加 |
| 123 | 一个函数把所有排着的会话看一遍（入队以后、连上了、记了禁言或解禁、定时醒了都调它），不照号、照会话挑 | 排着的不多（过期 60 秒）；一处管先后、过期、门开没开，不会这里发了那里漏了 | 连上了只看这个号的会话；解禁只看这个群 |
| 124 | 排着的过期写 0 起不来（同队列写 0） | 0 的意思是排不住：入队时刻加 0 不晚于此刻，门开着的也来不及交，一条都发不出去；写 0 多半是写错 | 0 当不过期；0 当不排、发不出去的马上作废 |
| 125 | 门开着的也先入队、再照先后交（入队以后看一遍）：先把过期的记了，再交门开着的 | 排着的还没交时（刚连上、`until` 刚到、定时还没醒）新来的不插队；过期的先记，日志里一眼看得出哪几条没发 | 门开着的跳过队直接发 |
| 126 | 机器人号连上了经读出来的消息那一条队告诉跟核心的那一头（`Event::Connected`）；那一头不收了（桥在停）的，这条连接不再读 | 和消息同一条队，先后一致：连上以后来的消息排在它后面；不另开通道 | 另开一条通道；跟核心的那一头定时看连着的号 |
| 127 | 测试里要等过期的，抄一份资源目录、改小 `queue_expire_seconds`（真核心拉起的桥读核心的资源目录）；假 NapCat 多一种一律回失败的（`refusing`），应答挪进 `support/answering.rs`（`group.rs` 再加就过 500 行） | 出厂 60 秒，测试不能一跑几十秒；断言结果，不断言耗时 | 测试等 60 秒；桥另认一个改数的环境变量（为测试改产品） |
| 128 | 认 `group_ban` 并进 `onebot.rs`，和撤回挨着（`read` 里通知先认撤回、再认禁言），不另开 `onebot/notices.rs` | 施工单说看行数定：加上以后 300 来行；两种通知各一个小函数，分开反倒要多一处登记 | 另开 `onebot/notices.rs`，撤回一起挪过去 |

**施工时定的**（O-25 下，2026-10-09；施工单「要定的」五条照推荐定：一段一块，带那一段的开头，桥重启时贴着的不摘，贴在她要回的那一条，判下来要回才贴）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 129 | 退信接在 `sending.rs` 记 `failed` 的后面，不另开模块；模板随出厂数据起来时读（`rules/facts.rs`，`Factory`），查过字段 | 退信只是失败那一步多一个调用；模板照判官的说明一起读，写坏了起不来，是打包的错，不会跑到一半才发现 | 另开 `route/undelivered.rs`；用的时候读文件 |
| 130 | 那一段的开头取去掉首尾空白以后的头 30 个字符；`detail` 没有的填空的 | 施工单「要定的」第 2 条：认得出是哪一句就够，30 个字符是中文一两句的开头；一份模板写不了「有才写」，`detail=""` 比多一份给模型看的字省 | 整段带；截 100 个字符；带 `detail` 的另一份模板 |
| 131 | 贴表情放 `route/reaction.rs`：记着贴着的（会话、序号 → 进了哪一轮、摘的信号），推来的 `turn.started`、`turn.joined` 记下进了哪一轮，`venue.delivered`、`turn.ended` 发摘的信号；一条一个另起的任务：贴、等信号或者到时候、摘。任务和撤回执的放在同一个任务集里（`Route` 的 `chores`，原来叫 `recalls`），崩了照「施工时定的」第 14 条停下 | 三种摘法谁先到算谁，放在一个任务里自己就只摘一次，不用另记摘没摘过；`route.rs` 只接线 | 跟核心的那一头定时看到期的（`wake` 多一种）；照投影算进了哪一轮 |
| 132 | `set_msg_emoji_like` 照 NapCat 的源码（`SetMsgEmojiLike.ts`：`message_id`、`emoji_id` 数或字都收，`set` 布尔）写：`message_id` 原样写成字（同第 109 条），`emoji_id` 照 `reaction_emoji` 的字；不带旧版的 `emoji_type: "1"`（NapCat 不读） | 查得到 NapCat 认什么就照它；编号原样还回去，不换写法 | 照旧版把编号换成整数、带 `emoji_type` |
| 133 | `session.respond` 成了才贴（回 `already_answered`、`not_ambient` 的不贴）；先记下贴着的，再收推来的 `turn.started` | 核心不开、不并的没人回，贴了只能等到时候摘；核心先推、后回应，推送留在 `Core` 里等桥办完这一条才收，进了哪一轮认得出 | `respond` 以前就贴 |
| 134 | 顶替接过去的：前一条贴着的当场摘掉，贴到这一条上 | 施工单「要定的」第 4 条：旧版挪到新的那一条；她要回的是新的那一条（引用也换成它） | 两条都贴着，到她回了一起摘 |
| 135 | 贴的表情、多久摘放 `bridge.json`（`reaction_emoji` 写成字，`reaction_seconds`）；0 秒是贴了就摘，不查 | 是桥这个进程的数，不按场所改（同第 114 条）；NapCat 把表情编号当字收；0 不会弄坏什么 | 放进场所规则；写 0 起不来 |
| 136 | 测试：假 NapCat 把 `set_msg_emoji_like` 另放一处（同撤回，不插进别的测试等的动作里），回成了、不占发出去的编号；她照台词说的替身（`support/speaking.rs`）记下每一次请求 | 别的测试等的动作不变；退信要看「正在跑的那一轮她下一次请求」，剧本的一次回复要么只说话、要么只调工具（第 115 条） | 改核心的剧本 |

**施工时定的**（测试的偶发红，2026-10-09，O-25 下顺手修，和 O-25 下分开提交）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 137 | `disciplines.rs` 里她回过的人接着说那一句（续聊），等那个群记下 `venue.delivered` 再发 | 续聊照她的回复算（投影里的 `venue.delivered`），测试等到 NapCat 收到她的话就接着说，那时桥还没记送达：桥先办这一条还是先记送达不一定（`select!`），先办的算不上续聊、只记下，测试等不到她回。O-25 下多了贴、摘表情的调用和几份新测试以后，两份测试程序并着跑 74 遍红了 5 遍（O-25 下以前的那一版并着跑 80 遍没红过）；改了以后 60 遍里没有这一条红的 | 桥判以前先等在路上的送达（为测试改产品）；拉长测试的等待 |

### 二、WebUI（施工 O-16 起）

`miyu-onebot` 自己的配置页（`docs/designs/18-通讯平台.md` 第三节、Q18）：桥进程开一个只听本机的 HTTP 端口，给页面、把 `/ws` 原样转给核心。页面自己说核心协议：用户名、密码登录（核心验，`web-module.md` 第一条 W-8），改配置、存密钥调核心现成的 `config.set`、`secret.set`，校验只在核心那一处。只有桥自己知道的由桥另给：只读的 `/status`（NapCat 连没连上、是哪个实现、令牌设没设）、`/token`（令牌的值），和照桥手里最新的配置换端口的 `/apply`（O-16 补二，O-20 改）。终端界面、网页软件的设置页里只有一行「QQ：打开 miyu-onebot」，不画 QQ 的配置。

状态：图纸，2026-10-07 起草，项目主人过目（登录用 Miyu 网页的账号、先做两页，照推荐定）。O-16 做好了骨架和「连接」页（施工时补的见「施工时定的」第 7 到 13 条；没设令牌也起来、在「连接」页生成第一个，见第 14 到 16 条）。2026-10-08 项目主人在自己的 NapCat 上试过以后改了四处（补二，第 17 到 24 条）：令牌随时能看能复制、没令牌时页顶写清三步、改了令牌和端口当场生效不用重启、地址写短的 `/ws`。O-17 做好了「主人与自己人」页（施工时补的见「施工时定的」第 25 到 36 条）；场所和规则、平台工具、插件、日志几页等群接通以后再做。

**共用的底子**：给页面文件、核对 Host 和 Origin、`/ws` 原样转给核心、安全响应头、带一次性码打开浏览器，这几样网页软件（`web-ui.md` 第一条、第二条）已经写好了，从 `miyu-web` 抽进一个第 3 层的新 crate，`miyu-web` 和 `miyu-onebot` 都用它，`miyu-web` 的行为一个字节不变（18 第三节「抽成两边共用的库，不抄一份」）。这个 crate 叫 `miyu-webserve`，在第 3 层，交出什么、从哪搬来见 `webserve.md`（「施工时定的」第 1 条）。

**对外的样子**

| 什么 | 说明 |
|---|---|
| 配置 `onebot.web` | WebUI 的端口，整数 1024 到 65535，出厂 8302（网页软件 8300、NapCat 8301，挨着好记），只能写在系统配置；改了当场换：核心推过来，桥照 `/apply` 的办法换（O-20，原来命令行改的要等桥下次起来）。在清单的 `[settings]` 里（O-20，第一条「软件包清单」；原来照 `onebot.listen` 的先例由核心声明） |
| 配置 `onebot.trusted`（O-17） | 自己人：平台身份的列表（`["qq:20017"]`），元素是文字、最多 128 个字；没有默认值，不写的当没有自己人；只能写在系统配置；生效时机 `now`。在「主人与自己人」页上整张写回。现在桥还不读（桥接群、算「发的人是谁」时读，`chat.md`「发的人是谁」）。在清单的 `[settings]` 里（O-20；原来照 `onebot.listen` 的先例由核心声明） |
| `GET /` 和页面文件 | 页面在 `resources/software/onebot/web/`，规矩照网页软件（`..`、跑到目录外的、不是普通文件的 404；类型照扩展名；响应头带 `nosniff`、`no-referrer`、`no-cache`、内容安全策略） |
| `GET /ws` | 原样转给核心，和网页软件的 `/ws` 一样（一帧一行、Origin 要对、1 MiB 上限、核心断了关 1012） |
| `GET /status` | 只读，`Authorization: Bearer <登录令牌>`；桥拿这个令牌去和核心握手，核心认了才回，不认 401；连不上核心 502；`GET` 以外 405。照桥手里最新的配置答，不读盘（O-20）。回 `{"napcat": {"connected": 布尔, "implementation": 字, "version": 字, "self_id": 字}, "listen": 端口, "web": 端口, "token": "set" \| "none", "platform": "qq"}`，`Cache-Control: no-store`；没连着的 `napcat` 只有 `connected: false`；连着、还没问到是哪个实现的，没有 `implementation`、`version`。连着几个号的，说号最小的那一个。`listen`、`web` 是实际听的端口（换过的照换过的：推送来的、`/apply` 的）。`token`：桥手里有令牌的 `set`，没有的 `none`（O-20：没写引用、引用取不到，核心都不交，分不出，原来的 `missing` 没了，第一条「施工时定的」第 41 条；O-16 补二：去掉 `restart_needed`，`listen` 不再有 `null`，「施工时定的」第 6、14 条）。`platform` 是桥的平台名（第一条的 `PLATFORM`），「主人与自己人」页照它拼 `qq:<号>`（O-17，「施工时定的」第 26 条） |
| `GET /token` | 要登录，和 `/status` 一样（不带、带错 401，连不上核心 502，`GET` 以外 405）。回桥手里最新的令牌 `{"token": "<值>"}`（不读盘，O-20），没有的 `{"token": null}`；`Cache-Control: no-store`；运行日志只记一行 `INFO web token read`，不记值（O-16 补二，「施工时定的」第 17 条） |
| `POST /apply` | 要登录，同上；`POST` 以外 405。照桥手里最新的配置换端口（O-20，不读盘）：两个端口里和上一次照的（配置里写的那个）不一样的，先开新的，都开上了才关旧的、换上新的，回 `{"listen": 端口, "web": 端口}`（实际听的）。新的开不了的回 409 `{"in_use": 端口}`，两个端口都不换、旧的照旧开着。已经连着的（NapCat 的那一条、页面正在用的连接）不断（O-16 补二，「施工时定的」第 19 条）。推送来的端口变化桥自己照这个办法当场换（第一条第 1 条），换好了的，`/apply` 什么都不换、回实际听的；推送来时被占、没换成的，`/apply` 再试一次（第一条「施工时定的」第 39 条）。WebUI 的端口被推来的换掉以后，旧地址上来的请求 Host 对不上新端口，照 Host 的规矩回 403（页面照刚存的端口跳，「施工时定的」第 37 条） |
| `GET /human` | 登录以前页面要的字（O-16 施工时补，「施工时定的」第 7 条）：不要登录，`{"language": <桥的语言>, "said": {<编号>: <模板>}}`，只有 `software/onebot/web/` 开头的；`GET` 以外 405 |
| `miyu-onebot web [--print]` | 桥要在跑（`miyu onebot start`，「施工时定的」第 5 条），令牌设没设都一样：开浏览器到 WebUI；还没设过登录密码的，照 `miyu web` 带上一次性码（`web-ui.md` 第二条）；`--print`、交不给浏览器的印网址 |
| 给人看的字 | 页面里的字照 `human.get` 取（核心和软件包的 `human/*.json` 合成的一份，`web-module.md`），桥的字在 `software/onebot/human/` 里，编号前缀 `software/onebot/`；登录以前照 `/human` |

`bridge.json` 的 `web`（O-16）：

| 格 | 出厂 | 说明 |
|---|---|---|
| `csp` | `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'` | 页面的 `Content-Security-Policy`：只连自己、不许被框起来、不许内联的脚本和样式 |
| `types` | `html`、`js`、`css` 三种，带 `charset=utf-8` | 扩展名到媒体类型；表里没有的给 `application/octet-stream`（不猜） |
| `status_cache_seconds` | 60 | `/status`、`/token`、`/apply` 验过的登录令牌记几秒（「施工时定的」第 3 条） |

**样子**（宽屏；窄屏左栏收成抽屉。颜色、字体、按钮照网页软件，看着是一家的）

```
┌──────────────────────────────────────────────────────────────────┐
│ miyu-onebot   ● NapCat 已连上              admin ▾   [回到 Miyu] │
├────────────┬─────────────────────────────────────────────────────┤
│ 连接       │ 连接                                                │
│ 主人与自己人│                                                     │
│ ── 以后 ── │ NapCat   ● 已连上  NapCat 4.8.2 · 机器人 qq:12345   │
│ 场所       │                                                     │
│ 平台工具   │ NapCat 那边要填的                                   │
│ 插件       │   反向 WebSocket 地址  ws://127.0.0.1:8301/ws  [复制] │
│ 日志       │   访问令牌            已设 ········  [显示] [复制] [换一个]     │
│            │   消息格式            数组                          │
│            │                                                     │
│            │ 端口                                                │
│            │   NapCat 连进来  [ 8301 ]   WebUI  [ 8302 ]  [保存] │
└────────────┴─────────────────────────────────────────────────────┘
```

```
没设令牌时，「连接」页的上半截（O-16 补二）

┌ 先做这三步 ─────────────────────────────────────────────────┐
│ ① 生成令牌：按下面「访问令牌」那一行的「生成」。              │
│ ② 在 NapCat 的网络配置里加一个「WebSocket 客户端」，填上下面的 │
│    地址和令牌，消息格式选数组。                               │
│ ③ 等几秒，下面 NapCat 那一行变成「已连上」。                  │
└─────────────────────────────────────────────────────────────┘
NapCat   ● 没连上

NapCat 那边要填的
  反向 WebSocket 地址  ws://127.0.0.1:8301/ws  [复制]
  访问令牌            没设                    [生成]（主按钮）
```

```
按了「显示」、刚生成完（O-16 补二）

  访问令牌            3f9c…（64 位十六进制）  [收起] [复制] [换一个]
```

```
主人与自己人（O-17）

┌ 主人 ──────────────────────────────────────────────────┐
│ 这些号的私聊就是本机账号本人；在群里多管理命令的权限。     │
│  QQ 号            本机账号                               │
│  [ 10001     ]    [ admin ▾ ]   [删]                     │
│  [ 1000x     ]    [ admin ▾ ]   [删]  号只能是数字，不以 0 开头 │
│  [ 10001     ]    [ admin ▾ ]   [删]  这个号重复了        │
│  [加一个]                                       [保存]   │
│  没成：<核心的原话>                                      │
│  ▸ 主人的号就是你本人：先知道这三条代价（点开是 18 第三节的三条） │
└─────────────────────────────────────────────────────────┘
┌ 自己人 ────────────────────────────────────────────────┐
│ 私聊里能叫她，不限流，睡着时私聊也放行。                  │
│ QQ 桥接通群以后才照这张表认人，现在先记下。               │
│  [ 20017     ]   [删]                                    │
│  [ 10001     ]   [删]  已经是主人                        │
│  [加一个]                                       [保存]   │
└─────────────────────────────────────────────────────────┘
```

```
表空着时（O-17）

主人      还没有主人。按「加一个」，填上你自己的 QQ 号，再按「保存」：以后你私聊她，她就认得是你。
          [加一个]                                  [保存]（灰着）
自己人    还没有自己人。
          [加一个]                                  [保存]（灰着）
```

**怎么走**

1. **起来**：桥（`miyu-onebot serve`，O-18 起由核心拉起）起来时连 NapCat 的监听之外，再开 `127.0.0.1:<onebot.web>`；端口被占了照 `onebot.listen` 被占一样说清、退出码 1。令牌没设的两个也照开（第一条第 1 条，O-16 补、补二）：第一次用，人在这里生成令牌。Host、Origin 照网页软件核对。
2. **登录**：页面连 `/ws`，握手带用户名和密码（或者记住的登录令牌，30 天，照网页软件），核心验。没有「接平台」能力的账号（`06-多用户与身份.md` 第四节），页面说进不来、断开。还没设过密码的，页面说「在终端里运行 `miyu-onebot web`」。
3. **连接页**：
   - 没令牌时（`/status` 的 `token` 不是 `set`）页顶一段三步：① 生成令牌；② 把地址和令牌填进 NapCat 的「WebSocket 客户端」，消息格式选数组；③ 等几秒，NapCat 那一行变成「已连上」。出来了就留着（令牌设好了第一步打勾），NapCat 连上了才收起，这一页里不再出来（O-16 补二，「施工时定的」第 22 条）。
   - NapCat 的状态照 `/status`，每 5 秒取一次。
   - 地址照 `ws://127.0.0.1:<onebot.listen>/ws` 拼（短的那个，O-16 补二），「复制」把它放进剪贴板。
   - 令牌照 `/status` 的 `token`（O-16 补二，「施工时定的」第 16、17 条）：`set` 的那一行「已设 ········」（照环境变量的写「照环境变量 <名字>」），三个按钮：「显示」取 `/token` 把值显示在那一行（按钮变「收起」，再按收起）；「复制」取 `/token` 放进剪贴板；「换一个」先在页面里的对话框问一句（NapCat 那边也要跟着换；「取消」「换一个」，先停在「取消」上）再生成。`none` 的那一行说没设，只有一个主按钮「生成」，不先问（O-20 起没写引用、引用取不到都是 `none`，原来 `missing` 那一行「引用的取不到」随之去掉，第一条「施工时定的」第 41 条）。生成：页面生成 32 个随机字节（十六进制），`secret.set` 存成 `onebot`，`config.set` 把 `onebot.token` 写成 `{ secret = "onebot" }`，再取 `/token` 显示出来：核心换上新配置时就把新令牌推给桥（`extension.config`），桥当场照新的（第一条第 1、2 条，O-20）。
   - 端口：`config.set` 写 `onebot.listen`、`onebot.web`；写错的核心回问题，页面照原话说。写好了调 `/apply`（核心已经把新端口推给桥、桥多半已经换好，`/apply` 回实际听的，O-20）：NapCat 的端口换了，说一句去 NapCat 里改地址；WebUI 的端口换了，先说一句（登录记在浏览器里、按地址分，到新地址要重新登录一次），再跳到新地址；被占了（409）说哪个端口被占、桥照旧用原来的（O-16 补二，「施工时定的」第 21 条）。桥已经换到新的 WebUI 端口的，页面这一次 `/apply` 走的是旧地址，回 403（Host 对不上新端口）或者连不上：页面照刚存的端口说那一句、跳过去（O-20，「施工时定的」第 37 条）。
4. **主人与自己人页**（O-17）：
   - 进这一页时 `config.get` 读一遍（不写 `keys`：全部，键里有人起的名字的照真的键列；「连接」页也照它）。平台的名字照 `/status` 的 `platform`；还没取到 `/status` 的先说「正在连」，取到了再画（「施工时定的」第 26、33 条）。
   - **主人**：`external.bindings` 下键是 `<平台>:<号>` 的每一格，一行一个号对一个账号；别的平台的不画、不碰。号只收数字，平台前缀由页面照桥的平台名拼（`qq:<号>`）。账号的下拉：协议里没有列账号的方法，下拉里是握手回的账号（核心里固定是 `admin`），加上表里已经写着的别的账号（手写的照原样留着，「施工时定的」第 27 条）；新加的一行照握手回的账号。保存时照改动发一条 `config.set`（`layer: "system"`，几项一起）：加的、改了账号的写 `external.bindings."qq:<号>" = "<账号>"`，删的恢复默认（`unset: true`，去掉这一格）。只写系统配置。
   - **自己人**：`onebot.trusted`，平台身份的列表，一行一个号；保存时整张写回（`value` 是整张列表；别的平台的身份照原样留在前面，「施工时定的」第 29 条）。同一个号也在主人表里的（照主人表里现在的行，没存的也算），那一行旁边说一句「已经是主人」，照样能存：主人的权限包含自己人的，不拦，免得改表时卡住。
   - 号：去掉前后的空白；空着的行不算（保存时当没有这一行，也不标）；不是 1 到 20 位数字、以 0 开头的标出来；同一张表里号重复的都标出来；有标着的不让存（「施工时定的」第 28 条）。表里手写的 `qq:` 后面不是数字的照样画出来、标着，删掉或者改对了才能存。
   - 两张表各有一个「保存」，没改动、有标着的行时灰着。存好了在那一张表下面说「存好了」，照 `config.get` 重读，只重画这一张表，另一张没存的改动留着（第 31 条）。核心回问题（`config_invalid`、还没设好密码的 `setup_first` 这些）照原话说在那一张表下面（`web/failed`），表里的东西不丢。
   - 表空着时：主人表写一句先做什么（「加一个」、填上自己的号、「保存」），自己人表写「还没有自己人」。主人表下面折起来一段「号被盗的代价」（`<details>`，18 第三节的三条），给人看的字。
   - 生效：`external.bindings` 核心当场照新的认（`config.md` 那一行的 `now`）；`onebot.trusted` 现在桥还不读（桥接群、算「发的人是谁」时才读），生效时机写 `now`，到时照核心推来的当场用（O-20），自己人那张表下面先写一句「QQ 桥接通群以后才照这张表认人」（第 32 条），桥接群那一步去掉。页面都不提示重启。
   - 左栏的「主人与自己人」点得进，「连接」点得回；在哪一页只记在页面里，重新载入回到「连接」（第 34 条）。窄屏点了左栏的一页，抽屉收起。
5. **运行日志**：照桥的 `state/logs/onebot.log`，只记 WebUI 起来、登录成功或失败的次数、取过令牌（`/token`，O-16 补二）、换了哪个端口（推送来的、`/apply` 的），不记密码、令牌、一次性码。

**在哪**、**改了谁**：见第一条「在哪」标 O-16 的几行；共用的底子 `crates/miyu-webserve/`（`webserve.md`，第 3 层，从 `miyu-web` 原样搬来，搬家表在那一页）。

**出错**

| 情况 | 怎么办 |
|---|---|
| WebUI 的端口被占（`serve`） | `failure/web-port-in-use`：哪个端口被占了、改 `onebot.web`；退出码 1 |
| Host 不对 | 403，运行日志 `WARN web rejected host=… origin=…` |
| `/ws` 的 Origin 不对、连不上核心 | 照网页软件：403；`web.error` 通知再关（`web-ui.md` 第一条第 7 款） |
| `/status`、`/token`、`/apply` 没带、带错登录令牌 | 401，运行日志 `WARN web login refused`（带对的、要和核心握手的记 `INFO web login accepted`）；连不上核心 502，`WARN core unreachable` |
| `/apply`：新端口开不了 | 409 `{"in_use": 端口}`，两个端口都不换、旧的照旧开着；`WARN apply port in use`（O-16 补二） |
| 推送来的端口开不了（O-20） | 同上：两个端口都不换、旧的照旧开着，`WARN apply port in use`；页面接着调的 `/apply` 再试一次、回 409 |
| `/human` 字读不懂 | 500，`WARN human not read` |
| `miyu-onebot web`：WebUI 的端口上连不上（端口照状态文件，没有状态文件的照清单的默认值，O-20） | `open/not-running`，退出码 1 |
| `miyu-onebot web`：连不上核心、要不到一次性码 | `failure/core`，退出码 1 |
| 令牌没设（`serve`、`web`） | 不算错（O-16 补、补二）：`serve` 照常开两个端口，说 `notice/no-token`，运行日志 `WARN no token yet`，NapCat 连进来 401；`web` 照常打开，人在「连接」页生成令牌，NapCat 下一次连就通 |

**给人看的字**：标准错误上的（`serve`、`web` 说的）放在 `said` 里，和第一条一样照 `Texts` 换；页面里的放在同一份文件里、编号 `web/` 开头，页面自己换字段（`{字段}`、`{{`、`}}`，照 `Human::say`）。`ja.json` 照英文写（第一条「施工时定的」第 17 条）。

| 编号 | 什么时候 | 中文 | 英文 |
|---|---|---|---|
| `notice/web` | 起来了 | QQ 桥的网页在 http://127.0.0.1:{port}，用 miyu-onebot web 打开。 | The QQ bridge web page is at http://127.0.0.1:{port}; open it with miyu-onebot web. |
| `failure/web-port-in-use` | WebUI 的端口被占 | 端口 {port} 被占了。换一个：miyu config set --system onebot.web <端口>。 | Port {port} is in use. Pick another: miyu config set --system onebot.web <port>. |
| `open/not-running` | 桥不在跑（O-18 改，O-20 改） | 127.0.0.1:{port} 上没有 QQ 桥的网页。先 miyu onebot start。 | No QQ bridge web page on 127.0.0.1:{port}. Turn it on with miyu onebot start. |
| `open/first` | 还没设过密码 | 还没设过网页的登录密码：带着一次性码打开网页，在网页上设用户名和密码。 | No web password yet: opening the web page with a one-time code to set a username and password. |
| `open/opened` | 交给了浏览器 | QQ 桥的网页开在 {url}，已经交给浏览器打开。 | The QQ bridge web page is at {url} and has been opened in your browser. |
| `open/print-hint` | 带了码 | 浏览器没打开的话，用 miyu-onebot web --print。 | If the browser did not open, use miyu-onebot web --print. |
| `open/open-this` | `--print` | 在浏览器里打开： | Open this in a browser: |
| `open/code-warning` | `--print` 带了码 | 这个链接 5 分钟内有效，只能用一次，别发给别人。 | This link works once within 5 minutes. Do not share it. |

页面的字（`web/…`，中文；英文见 `resources/software/onebot/human/en.json`）：登录（`web/login/*`：登录、用户名、密码、「还没设过密码的，在终端里运行 miyu-onebot web。」）、设密码（`web/setup/*`）、`web/loading`、`web/core-lost`、`web/retry`、左栏（`web/nav/*`、`web/later`、`web/not-yet`）、`web/menu`、`web/sign-out`、没令牌时的三步（`web/guide/*`，O-16 补二）、NapCat 的状态（`web/napcat/*`：已连上、没连上、看不到桥的状态，`{implementation} {version} · 机器人 {bot}`）、要填的（`web/fill/*`，地址的写法 `ws://127.0.0.1:{port}/ws` 也在这里）、令牌（`web/token/*`：已设、没设、照环境变量、显示、收起、换一个、生成、换之前问一句；「引用的取不到」`web/token/missing` O-20 去掉）、对话框的「取消」（`web/cancel`）、端口（`web/ports/*`：两格的名字，NapCat 的端口换了、WebUI 的端口换了要重新登录、被占了）、`web/copy`、`web/copied`、`web/save`、`web/saved`、`web/failed`、`web/brand`；「主人与自己人」页（`web/people/*`，O-17，中文照下表，存好了、没成照上面的 `web/saved`、`web/failed`）。补二去掉了 `web/napcat/closed`（没开端口）、`web/token/new`、`web/token/new-hint`（只显示这一次）、`web/restart`（重启以后生效）。

| 编号（`web/people/` 后面） | 中文 | 英文 |
|---|---|---|
| `owners/title` | 主人 | Owners |
| `owners/hint` | 这些号的私聊就是本机账号本人；在群里多管理命令的权限。 | Private chats from these accounts are the local account itself; in groups they also get the admin commands. |
| `owners/empty` | 还没有主人。按「加一个」，填上你自己的 QQ 号，再按「保存」：以后你私聊她，她就认得是你。 | No owners yet. Press Add, enter your own QQ number and press Save: from then on she knows it is you in private chat. |
| `number` | QQ 号 | QQ number |
| `account` | 本机账号 | Local account |
| `add` | 加一个 | Add |
| `remove` | 删 | Remove |
| `bad-number` | 号只能是数字，不以 0 开头 | Digits only, not starting with 0 |
| `duplicate` | 这个号重复了 | This number is listed twice |
| `already-owner` | 已经是主人 | Already an owner |
| `trusted/title` | 自己人 | Friends |
| `trusted/hint` | 私聊里能叫她，不限流，睡着时私聊也放行。 | They can call her in private chat, with no rate limit, even while she sleeps. |
| `trusted/later` | QQ 桥接通群以后才照这张表认人，现在先记下。 | The QQ bridge uses this list once groups are connected; it is kept until then. |
| `trusted/empty` | 还没有自己人。 | No friends yet. |
| `risk/title` | 主人的号就是你本人：先知道这三条代价 | An owner account is you: know these three costs |
| `risk/stolen` | QQ 号被盗，就等于这台机器在你的权限下被别人用。 | If the QQ account is stolen, someone else uses this machine with your permissions. |
| `risk/forged` | 「这是主人发的」只能听 NapCat 报上来的：NapCat 被人拿下、或者本身不怀好意，就能冒充你的私聊。 | "This is from the owner" rests on what NapCat reports: a compromised or malicious NapCat can fake your private chats. |
| `risk/injection` | 网页、文件里藏着的提示词注入，能让她在沙盒允许的范围里乱改工作区的文件；越过沙盒的操作在私聊里一律做不了。 | Prompt injection hidden in web pages or files can make her change workspace files within what the sandbox allows; nothing beyond the sandbox can be done from a private chat. |

**样子**的实际：宽屏顶栏「miyu-onebot ● NapCat 已连上 … admin ▾」，`admin ▾` 里是「退出登录」；左栏「连接」「主人与自己人」两页点得进（O-17），「以后」下面四页是灰的、点不了；「连接」页三张卡片（NapCat、NapCat 那边要填的、端口）。令牌没设的（O-16 补、补二）：页顶多一张「先做这三步」的卡片，令牌那一行「没设」、一个主按钮「生成」；生成以后令牌那一行变「已设」，值直接显示在那一行（按钮是「收起」「复制」「换一个」），三步的第一步打勾，NapCat 连上以后三步收起。窄屏（720px 以下）左栏收成抽屉，顶栏左边一个按钮开它。亮暗两色跟着系统（`prefers-color-scheme`），颜色照网页软件的「晨光」和「tokyonight」。图里的「回到 Miyu」O-16 没画（「施工时定的」第 12 条）。

**守着它的**（`crates/miyu-onebot/tests/`，O-16；核心是 `miyu-ipc` 的监听当替身，照网页软件）

- Host 只认三种写法，NapCat 的端口、别的端口都 403，`/status`、`/human`、`/ws` 也一样；页面不出页面目录（`..`、`%2e%2e`、链接、目录、`%zz`）；响应头一个不少、内容安全策略照 `bridge.json`、从不设 cookie；`HEAD`、别的方法 405；类型照表、表里没有的不猜。`/ws` 的 Origin 不对 403（没有、别的网站、`https`、`null`、NapCat 的端口）；两头一帧一行照转、一个字节不改（凭据、中文、空白、很长的一行）；核心断了 1012；连不上核心发 `web.error` 再关。WebUI 的端口被占说是哪个端口。`/human` 照桥的语言、只给 `web/` 开头的。（`web.rs`）
- `/status`：不带、带错（差一个字、多一个字、空的、别的方案）401；带对的回状态（两个实际的端口、`token`）；记着的不再握手，错的每次都握、都 401，过了时候再握；NapCat 连上以后说是哪个实现、哪个号，断了说没连着。（`status.rs`）
- 令牌没设（O-16 补、补二，O-20 改）：两个端口都开，`/status` 的 `listen` 是实际的端口、`token` 照推来的说 `none`、`set`；真的程序 `serve`、`web --print` 照常走（第一条「守着它的」，`no_token.rs`）。
- `/token`（O-16 补二）：不带、带错登录令牌 401，`GET` 以外 405；带对的拿到值，`no-store`；没有的是 `null`。（`token.rs`）
- `/apply`（O-16 补二，O-20 改）：不带、带错 401，`POST` 以外 405；推来新的 NapCat 端口，不用 `/apply` 就换：旧的连不上、新的连得上，`/status` 跟着说，再 `/apply` 回新端口、什么都不换；推来新的 WebUI 端口：新地址上页面、`/status` 照常，旧的连不上；推来的新端口被占，旧的照旧、两个都不换，`/apply` 回 409 和是哪个，端口空出来以后再 `/apply` 换得上；推来新令牌：旧的 401、新的进得来、已经连着的那一条还在；推来 `null`：一律 401。（`apply.rs`）
- 验过的登录令牌：记一阵、到点就忘、别的令牌不算；只记 SHA-256，没有原文。（`src/web/tests.rs`）
- `miyu-onebot web`：桥不在跑说先 `miyu onebot start`、退出码 1、不开浏览器；没设过密码的带 `#setup=`，设过的不带；`--print`、交不给浏览器的印网址和提醒；网页的端口照状态文件里桥实际听的，没有、读不懂的照清单的默认值（O-20）。（`open.rs`）
- 出厂的 `bridge.json` 的 `web`：三种类型、策略只连自己不许被框、60 秒；多一格少一格读不进来。（`tuning.rs`）握手没交 `onebot.web` 的照清单的默认值 8302。（`settings.rs`）新加的每一句三种语言都换得出来。（`texts.rs`）
- 「主人与自己人」页（O-17，`people.rs`；真的核心加真的桥，浏览器经 `/ws` 照页面的样子发）：还没设好密码的连接（一次性码）改系统配置回 `setup_first`、什么都没写；设好以后加一个主人（`external.bindings."qq:<号>"`）、删一个主人（恢复默认）、整张写回自己人，换一条连接 `config.get` 读得回，桥收到新主人的私聊、记成管理员本人；自己人写进个人设置、写成一个字、元素是空的字，`config_invalid`，什么都没变。`/status` 带 `platform`（`status.rs`）。`onebot.trusted` 读得出、不写是没有、只能写系统配置、写错的报问题（`crates/miyu-core/tests/settings.rs`，O-20 起照出厂清单的 `[settings]`、在「软件包」页这个包那一组）。
- 页面没有自动测试：`node --check` 查 `app.js`、`people.js` 的写法；无头浏览器截图（补二：没令牌时的三步、显示令牌、换令牌的确认，亮暗各一套；O-17：两张表空着、填了几行（号不对、重复、已经是主人）、核心回问题、窄屏，亮暗各一套）；项目主人在浏览器里试一次（O-16 施工单验收第 6 条、O-17 第 5 条）。O-20：真核心加真桥、无头浏览器在「连接」页改 NapCat 的端口（当场换、说去 NapCat 里改）、改 WebUI 的端口（先说一句、跳到新地址、要重新登录）、两个端口各改成被占的（说是哪个、桥照旧），截图看一眼（O-20 施工单「验收结果」）。

**施工时定的**（O-16，2026-10-07；补二，2026-10-08；O-17 第 25 到 36 条，2026-10-08）

| # | 定了什么 | 为什么 | 没选 |
|---|---|---|---|
| 1 | 共用的底子叫 `miyu-webserve`，第 3 层：给页面文件（路径、类型、安全响应头）、核对 Host 和 Origin、`/ws` 原样转给核心、带一次性码开浏览器；函数原样从 `miyu-web` 搬过去，不改名、不改行为 | 18 第三节「抽成两边共用的库，不抄一份」；名字说清它是给网页的那一层 | 叫 `miyu-webkit`（和浏览器引擎撞名）；抄一份进桥 |
| 2 | 前端照网页软件：原生 JS，`index.html`、`app.js`、`style.css` 三个文件，页面里的字照 `human.get` | 和网页软件一家，不加构建工具 | 引一套前端框架 |
| 3 | `/status` 验过的登录令牌记 60 秒（只记令牌的哈希） | 页面每 5 秒取一次，不用每次都和核心握手 | 每次都握手；不验 |
| 4 | 登录成功就算能进：核心现在只有管理员一个账号，「接平台」这项能力随多用户那一段 | 不为以后写代码；到时照能力判 | 现在就做能力检查 |
| 5 | `miyu-onebot web` 要桥已经在跑；不在跑的说先 `miyu onebot start`（O-18 改，原来说先 `miyu-onebot serve`） | 桥由核心照开关拉起，开不开是人的决定，打开网页不替人开 | 照 `miyu web` 在后台拉起 serve |
| 6 | `/status` 每次重读一次配置：`token` 照读到的说，桥手里的令牌跟着换上（补二改写，2026-10-08 项目主人定「改了当场生效」：去掉 `restart_needed`）。**O-20 改**：不读盘，照桥手里最新的配置（核心推来的）说 | 页面要照实说令牌设没设；读到了就用，令牌改了不等 NapCat 来碰 | 和起来时读到的比、说要不要重启（O-16 原来的 `restart_needed`）；订阅配置的推送 |
| 7 | 登录以前页面的字由桥的 `/human` 给：照桥的语言（握手回的）读 `software/onebot/human/<语言>.json`，只交 `web/` 开头的，样子和 `human.get` 的 `said` 一样；登录以后照 `human.get` 换一遍（施工时补，2026-10-07） | 登录以前页面还没和核心握手，调不了 `human.get`，登录表单的字没处来；字还是住在 `human/*.json` 里 | 登录表单的字写进 `index.html`（字不是数据）；登录以后也只用 `/human`（和图纸「照 `human.get`」两样） |
| 8 | `/status` 验登录令牌时握手报 `head.kind = "onebot"`，等回应照 `bridge.json` 的 `call_timeout_seconds`，握完就关；连不上核心回 502 | 不另开一个数；502 照网页软件 `/media` 连不上核心 | 每次留着连接复用（照 `/media`：页面 5 秒一次，又有 60 秒的记着，省不了几条） |
| 9 | 连着几个机器人号的，`/status` 说号最小的那一个；还没问到是哪个实现的，不写 `implementation`、`version` | 图纸的 `napcat` 是一个；不写比写空的字清楚 | 列出全部（图纸的样子要改） |
| 10 | `miyu-onebot web` 认 `--print`；判桥在不在跑，照 `onebot.web` 连一下 `127.0.0.1` 的端口 | snap 装的浏览器打不开时要能印网址（照 `miyu web`）；桥不写 `run/` 里的地址，连一下最朴素 | 不认 `--print`；桥像网页软件那样写 `run/onebot` |
| 11 | 页面：登录令牌记在浏览器的 `localStorage`（`miyu-onebot.login`，带核心给的过期时刻）；握手报 `head.kind = "onebot-web"`；`/status` 每 5 秒、断了每 5 秒重连，写在 `app.js` 顶上；NapCat 的地址照 `onebot.listen` 和 `web/fill/url` 那一句拼；端口照 `input` 交给核心读；退出登录调 `account.logout`（只作废这一个） | 图纸「记住的登录令牌，30 天，照网页软件」「每 5 秒」；页面的节奏是页面自己的，没有数据文件可放；地址的写法和配置说明里的那句一样是给人看的字 | 每 5 秒写进 `bridge.json` 再经 `/status` 交给页面（`/status` 的样子要改） |
| 12 | 图里的「回到 Miyu」O-16 不画 | 页面不知道网页软件在哪：它的端口在 `web.json`、地址在 `run/web`，都不归桥管 | 写死 8300（网页软件 `--port` 换了就错） |
| 13 | 新加 `Unready::BadWebPort`、`Failure::WebPortInUse`、`Notice::Web`，各一句话 | 端口被占要说改哪一项（`onebot.web`，不是 `onebot.listen`） | 共用 `listen` 那一句 |
| 14 | 令牌没设也开 NapCat 的端口，连进来一律 401；`/status` 的 `listen` 总是实际听的端口（补，2026-10-07；补二改写，2026-10-08 项目主人定） | 设了令牌不用重启就要能连，端口得先开着；令牌对不上本来就回 401 | 令牌没设不开 NapCat 的端口、`listen` 写 `null`（补的做法：设了要重启才开） |
| 15 | 令牌没设那一句从读配置的 `Unready::NoToken` 挪成起来以后说的 `Notice::NoToken`，编号 `unready/no-token` 改成 `notice/no-token`（补，2026-10-07）；补二：跟在 `Notice::Listening` 后面说（语言照 `Listening` 带的换，它自己不再带）；`Settings::token` 是 `Token`（取到了、没写引用、取不到三种）。**O-20 改**：`Settings::token` 是 `Option<Secret>`（核心不交取不到的，分不出，第一条「施工时定的」第 41 条）；`Notice::Listening` 不带语言，握手回了语言 `main.rs` 就换（第 42 条） | 没设已经不是起不来；这一句在握手以后说，照核心回的语言；`/status` 要分清没写引用和取不到 | 留在 `Unready`、读配置时就说（照系统的语言说，又和后面几句不是一种语言）；`Option<Secret>`（分不出两种没有） |
| 16 | 页面：令牌设没设照 `/status` 的 `token`：`set` 的（照环境变量的也是）「显示」「复制」「换一个」，换之前问；`none`、`missing` 只有一个主按钮「生成」，不问（补，2026-10-07；补二改写，2026-10-08）。**O-20 改**：`missing` 没了（第一条「施工时定的」第 41 条），页面那一支删掉 | 没有旧的要换掉，问「换吗」不对；环境变量设没设桥看得到，页面不用猜 | 一直叫「换一个」；页面照 `secret.list` 自己算（看不到环境变量，只能当它有） |
| 17 | 令牌随时能看、能复制：`/token` 由桥交给登录了的管理员；页面不再写「只显示这一次」（补二，2026-10-08 项目主人定，推翻 2026-10-07「只在刚生成时显示一次」） | 本机、要登录、只给管理员的页面，方便优先；令牌是桥自己的凭据，桥手里本来就有，由桥交给登录了的管理员，不经核心协议交出去，和 `config.md` 第九条（密钥不经协议交出去）不冲突 | 只在刚生成时显示一次（试的时候抄错了、关了页面，只能再换一个） |
| 18 | 令牌当场生效：NapCat 出示的对不上（或者桥手里还没有）时重读一次配置再比，节流 `reload_seconds`（出厂 1 秒）；WebUI 每次读配置（`/status`、`/token`、`/apply`）也照读到的换上；已经连着的那一条不断（补二，2026-10-08 项目主人定「不用重启」，做法照推荐）。**O-20 改**：核心有了推送（9-4 下下，值也交过来），去掉重读和 `reload_seconds`，推来了就换；已经连着的照旧不断 | 令牌经核心写进密钥文件和系统配置，桥没有推送可听；NapCat 几秒重连一次，对不上时读一下最省事；节流：乱连的不能把读配置变成负担；连着的那一条握手时出示的是当时对的，断了它 NapCat 那边还没改，白断 | 订阅核心的配置推送（多一条订阅，值还是要自己读）；每次连进来都读；定时读；换了令牌断开连着的 |
| 19 | `/apply`：两个端口照配置里写的比上一次照的（不是实际听的），变了的先开新的，都开上了才关旧的；有一个开不了回 409，两个都不换，令牌照样换上；同时来的几个 `/apply` 一个一个办（补二，2026-10-08）。**O-20 改**：不读盘，照桥手里最新的配置；推送来的端口变化也照这个办法换（第一条「施工时定的」第 39、45 条） | 开不了新端口时旧的照旧，桥不会落到哪个端口都不听；写 0 的（测试）每次都是 0，不当成变了；令牌和端口互不牵连 | 先关旧的再开新的；开上一个算一个；照实际听的比（写 0 的每次都当变了） |
| 20 | 页面上的地址写短的 `ws://127.0.0.1:<端口>/ws`（补二，2026-10-08 项目主人定） | 两个路径桥都认，短的好填 | 照 OneBot 的惯例写 `/onebot/v11/ws` |
| 21 | 端口保存以后调 `/apply`：WebUI 的端口换了，页面先说一句（`alert`）再跳到新地址，在新地址上重新登录；被占了（409）页面说清，不把配置改回去（补二，2026-10-08） | 登录令牌记在浏览器里、按地址分，新地址上没有；不在地址里带登录令牌（会进浏览历史）。被占时配置里已经是新端口，说清被占、桥照旧用原来的，人换一个再存就够；改回去要多一次 `config.set` | 地址里带着登录令牌跳过去；409 时页面把配置改回原来的 |
| 22 | 没令牌时页顶的三步：`/status` 的 `token` 不是 `set` 时出来，出来了就留到 NapCat 连上（令牌设好了第一步打勾）；令牌早就设好、只是没连上的不出来（补二，2026-10-08） | 生成以后还有两步要做，这时收起，人不知道下一步；设好过的人不用再看一遍 | 只看 `token`（生成完就收起）；只看连没连上（设好过、NapCat 暂时断了也出来） |
| 23 | 换令牌之前问的那一句用页面里的对话框（`<dialog>`），不用浏览器的 `confirm`（补二，2026-10-08） | 跟着页面的亮暗色、字体，和页面是一家；截图看得到（浏览器自己的对话框不进页面的截图） | 浏览器的 `confirm`（O-16 原来的做法） |
| 24 | 配置清单里 `onebot.token` 的生效时机改 `now`，说明写「改了以后，NapCat 下一次连进来就照新的」；`onebot.listen`、`onebot.web` 留 `head_start`，说明补「在 QQ 桥的网页上改、保存的，当场生效」；地址的说法写 `/ws`；没设令牌那一句改成「QQ 桥照样起来，NapCat 连进来会被拒」（补二，2026-10-08 主会话定；`OnebotSettings` 是 O-8 权宜放在核心里的，核心同意过由桥这边改）。**O-20 改**：四项挪进清单的 `[settings]`，两个端口也改 `now`，说明里网页那一句去掉（第一条「施工时定的」第 40、46 条） | 令牌改了确实当场生效（第 18 条），说「下次启动时」会让人白重启；命令行改的端口还是要重启或在网页上按保存，`head_start` 照实说，网页上的另补一句 | 令牌留 `head_start`（说得不对）；端口改 `now`（命令行改的不会当场换） |
| 25 | 页面拆成模块：`app.js`（登录、骨架、「连接」页）、`people.js`（「主人与自己人」页，`app.js` 引进来）；`index.html` 照 `type="module"` 载 `app.js`（O-17） | 一页一个文件，不出包揽一切的大文件；原生的模块不用构建工具，内容安全策略 `script-src 'self'` 照样放行，`.js` 的类型表里本来就有 | 都写在 `app.js` 里（八百多行）；两个普通脚本经全局变量共用 |
| 26 | 平台的名字照 `/status` 多的一格 `platform`（第一条的 `PLATFORM`）；页面拼 `<平台>:<号>`，认的也只认这个平台的（O-17） | 平台的名字只写一处（第一条「施工时定的」第 16 条）；页面本来每 5 秒取 `/status` | 页面里再写一份 `qq` |
| 27 | 账号的下拉：握手回的账号，加上表里已经写着的别的账号；新加的一行照握手回的（O-17） | 协议里没有列账号的方法，核心现在只有 `admin`（第 4 条），照实；手写的别的账号不能因为页面不认就被改掉 | 请核心加 `account.list`（多用户那一段的事）；写死 `admin` |
| 28 | 号去掉前后空白，照 `1` 到 `9` 开头、一共 1 到 20 位数字认；空着的行不算、不标；同一张表里重复的标出来；有标着的不让存（O-17） | 桥照整数拼 `qq:<号>`，`0123` 写进去永远对不上；刚按「加一个」的空行就标红扰人 | 收任意字；空行也标；以 0 开头的也收 |
| 29 | 只画、只改这个平台的：主人表里别的平台的键不画、不碰；自己人整张写回时，别的平台的身份照原样留在前面。手写的 `qq:` 后面不是数字的照样画出来、标着（O-17） | 页面只管这个桥的平台，不把别处写的冲掉；坏的那一格让人看见、改对或者删掉 | 整张写回时只写页面上的（别的平台的被冲掉）；坏的不画（存的时候被悄悄删掉） |
| 30 | 「已经是主人」照主人表里现在的行（没存的也算）（O-17） | 人改着两张表时当场看得到 | 只照存好的 |
| 31 | 存好了照 `config.get` 重读，只重画存的那一张表；另一张没存的改动留着（O-17） | 两张表各存各的 | 整页重读（另一张没存的改动丢了） |
| 32 | 自己人那张表下面写一句「QQ 桥接通群以后才照这张表认人，现在先记下」，桥接群那一步去掉（O-17） | 现在桥还不读 `onebot.trusted`（O-8 只接主人的私聊），不说的话人加了自己人、私聊没反应，以为坏了 | 不说 |
| 33 | 读配置的 `load` 不写 `keys`（全部），两页共用一份（O-17，「连接」页原来只要三项） | 主人表有哪些号事先不知道，只能全要；一份 `items` 两页都用，不各读各的 | 两页各读各的；主人表照 `config.schema` 先问有哪些键（没有这种问法） |
| 34 | 在哪一页只记在页面里，重新载入回到「连接」（O-17） | 朴素；地址栏的 `#` 给一次性码用 | 照地址栏的 `#people` 记 |
| 35 | 「不是管理员的账号改系统配置被拒」照实测：核心现在只有管理员一个账号（第 4 条），没有这种账号；测的是还没设好密码的连接（一次性码）改系统配置回 `setup_first`、什么都没写，页面照原话说在表下面（O-17） | 照核心现在的样子测，不为测造一个核心里没有的账号；多用户那一段有了别的账号再补 | 造一个假的账号（核心里没有） |
| 36 | `onebot.trusted` 的元素是文字，最多 128 个字（和主人表的 `<external>` 一样长）；没有默认值（`none`），不写的页面当空表（O-17） | 元素是平台身份，照 `<external>` 的长度；宏里只有密钥的列表认 `[]` 当默认值，要 `[]` 得改 `miyu-config` | 默认值 `[]`；照号只收数字（规矩在页面，平台无关的规矩在桥那边，施工单「要定的」第 4 条） |
| 37 | 端口存好以后，WebUI 的端口换了的，`/apply` 回了的照回的跳；旧地址上问不到（403、连不上）的照页面刚存的端口跳；被占的照 409 说（O-20，2026-10-09，主会话定） | 核心推来的多半比页面的 `/apply` 先到，桥已经换到新端口：页面这一次 `/apply` 走的是旧地址上留着的连接，桥照新端口核对 Host 回 403（无头浏览器里试出来的，页面卡在旧地址说「没成：/apply 403」）；刚存的端口页面自己知道，不用非等 `/apply`。被占的推送换不成，桥还在旧地址上，`/apply` 回 409 照旧说 | 只照 `/apply` 回的跳（卡在旧地址）；存好了不问 `/apply` 直接跳（新端口被占时跳到没人听的地址） |
