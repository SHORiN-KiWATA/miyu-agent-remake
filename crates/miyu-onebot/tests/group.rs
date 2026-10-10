//! 群消息记进场所会话（施工 O-22，`onebot.md` 第一条「群消息」「撤回」）：真核心照开关拉起真的桥，假 NapCat 发群消息。群里的
//! 每一条都记进 `qq:group:<群号>` 那个会话（属主是系统账号 `onebot`），一律旁听、不开回合；名字、@、引用、@全体、带的东西
//! 各自记进场所的格；`show_ids`、`managers`、睡觉时间照场所规则；机器人自己发的不记；规则写了不存在的人格的群不记、只记一行
//! 运行日志；群里的斜杠命令照私聊的办法交，回执发回群里；撤回记 `venue.recalled`；只有一张图、只有一个表情的也记得进（核心
//! O-13 补以后）。挑的空端口在桥起来以前被别人占了的，换一个从头再来（`support/ports.rs`）。

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::support::group::*;
use crate::support::*;

/// 规则里 `show_ids`、`managers` 都设了的群。
const GROUP: i64 = 555;

/// 规则里睡觉时间盖住此刻的群。
const SLEEPY: i64 = 666;

/// 规则里写了不存在的人格的群。
const BROKEN: i64 = 777;

/// 规则里 `managers` 有的人。
const MANAGER: i64 = 40004;

/// 群里发过言的人、没发过言的人、NapCat 也问不到的人。
const LIN: i64 = 20002;
const JIE: i64 = 20003;
const WANG: i64 = 20005;
const NOBODY: i64 = 20009;
const ZHOU: i64 = 20010;

/// 假 NapCat 认得的群成员：她自己（机器人的号）、老王、小周。
const MEMBERS: &[Member] = &[
    (BOT, "米尤", "miyu"),
    (WANG, "", "老王"),
    (ZHOU, "小周", "z"),
];

/// 核心照中文写的几句（`resources/core/human/zh.json`、`protocol.md`「给人看的字」）。
const STOPPED: &str = "已全部停止。";
const NOT_ALLOWED: &str = "仅终端管理员和群管理员可用。";

/// 系统的场所规则：[`GROUP`] 写 `show_ids`、`managers`，[`SLEEPY`] 的睡觉时间盖住此刻（前后各一个小时，照本机的时区），
/// [`BROKEN`] 写一个不存在的人格。
fn rules() -> String {
    let now = jiff::Zoned::now();
    let minute = i64::from(now.hour()) * 60 + i64::from(now.minute());
    let clock = |minute: i64| {
        let minute = minute.rem_euclid(24 * 60);
        format!("{:02}:{:02}", minute / 60, minute % 60)
    };
    format!(
        "[[rule]]\nmatch = {{ kind = \"group\", group = [{GROUP}] }}\nshow_ids = true\nmanagers = [\"qq:{MANAGER}\"]\n\n\
         [[rule]]\nmatch = {{ kind = \"group\", group = [{SLEEPY}] }}\nsleep = \"{}-{}\"\n\n\
         [[rule]]\nmatch = {{ kind = \"group\", group = [{BROKEN}] }}\npersona = \"nobody-here\"\n",
        clock(minute - 60),
        clock(minute + 60),
    )
}

#[tokio::test]
async fn group_messages_are_recorded_as_ambient_with_their_venue_fields() {
    let script = Script::new([]);
    let (home, napcat, _) = started(&script, &rules(), "", MEMBERS).await;
    let venue = format!("qq:group:{GROUP}");
    // 1、2：名字取群名片，空白的取昵称。
    napcat.send(group_frame(
        GROUP,
        LIN,
        1,
        json!([plain("大家好")]),
        ("小林", "lin"),
    ));
    napcat.send(group_frame(
        GROUP,
        JIE,
        2,
        json!([plain("我在")]),
        ("  ", "阿杰"),
    ));
    // 3：@ 她、@ 发过言的阿杰（照缓存）、@ 没发过言的老王（问 NapCat）、@ NapCat 也问不到的人（写号）；问不到一次，后面的小周
    // 不再问，也写号，阿杰照缓存。
    let mentioning = json!([
        at(BOT),
        plain(" "),
        at(JIE),
        plain(" "),
        at(WANG),
        plain(" "),
        at(NOBODY),
        plain(" 看这个"),
        at(JIE),
        at(ZHOU),
    ]);
    napcat.send(group_frame(GROUP, LIN, 3, mentioning, ("小林", "lin")));
    // 4：引用、@全体。
    let quoting = json!([{"type": "reply", "data": {"id": "1"}}, at("all"), plain(" 开会了")]);
    napcat.send(group_frame(GROUP, JIE, 4, quoting, ("", "阿杰")));
    // 5：带的东西各一样，合并转发、卡片写占位，戳一戳不记。
    let media = json!([
        plain("看图"),
        {"type": "image", "data": {"file": "a.jpg", "sub_type": 0, "summary": "", "url": "https://x/a"}},
        {"type": "image", "data": {"file": "b.gif", "sub_type": 1, "summary": "[动画表情]"}},
        {"type": "face", "data": {"id": "14", "raw": {"faceIndex": 14, "faceText": "/微笑"}}},
        {"type": "image", "data": {"file": "x.gif", "emoji_id": "e1", "emoji_package_id": 9, "summary": "[吃瓜]"}},
        {"type": "mface", "data": {"emoji_id": "m1", "emoji_package_id": 9, "key": "k", "summary": "[比心]"}},
        {"type": "record", "data": {"file": "v.amr"}},
        {"type": "video", "data": {"file": "c.mp4", "file_id": 77}},
        {"type": "file", "data": {"file": "排班.pdf", "file_id": "/f-1", "file_size": "12"}},
        {"type": "forward", "data": {"id": "fw1"}},
        {"type": "json", "data": {"data": "{}"}},
        {"type": "poke", "data": {"type": "1", "id": "1"}},
    ]);
    napcat.send(group_frame(GROUP, LIN, 5, media, ("小林", "lin")));
    // 6：机器人自己发的不记。
    napcat.send(group_frame(
        GROUP,
        BOT,
        6,
        json!([plain("我说的")]),
        ("米尤", "miyu"),
    ));
    // 7：规则里的管理的人；8：群里的终端管理员。
    napcat.send(group_frame(
        GROUP,
        MANAGER,
        7,
        json!([plain("收到")]),
        ("管理", "m"),
    ));
    napcat.send(group_frame(
        GROUP,
        ADMIN,
        8,
        json!([plain("辛苦了")]),
        ("终端管理员", "o"),
    ));
    // 睡着的群；规则写错的群连发两条。
    napcat.send(group_frame(
        SLEEPY,
        LIN,
        9,
        json!([plain("睡了吗")]),
        ("小林", "lin"),
    ));
    napcat.send(group_frame(
        BROKEN,
        LIN,
        10,
        json!([plain("一")]),
        ("小林", "lin"),
    ));
    napcat.send(group_frame(
        BROKEN,
        LIN,
        11,
        json!([plain("二")]),
        ("小林", "lin"),
    ));
    // 撤回：小林的第 1 条，管理的人撤的。一条条照先后办，它记下了，前面的就都办完了。
    napcat.send(group_recall(GROUP, LIN, MANAGER, 1));
    let events = until_event(&home.root, &venue, |event| {
        event["kind"] == "venue.recalled"
    })
    .await;

    let said = said(&events);
    let texts: Vec<&str> = said.iter().map(words).collect();
    assert_eq!(
        texts,
        [
            "大家好",
            "我在",
            "@米尤 @阿杰 @老王 @20009 看这个@阿杰@20010",
            " 开会了",
            "看图[forward][card]",
            "收到",
            "辛苦了",
        ],
        "机器人自己发的不记"
    );
    let fields: Vec<&Value> = said.iter().map(|said| &said["body"]["venue"]).collect();
    assert_eq!(
        *fields[0],
        json!({"msg": "1", "name": "小林", "ambient": true, "show_ids": true})
    );
    assert_eq!(fields[1]["name"], "阿杰", "群名片空白的取昵称");
    assert_eq!(
        *fields[2],
        json!({
            "msg": "3", "name": "小林", "mentions": ["qq:20003", "qq:20005", "qq:20009", "qq:20010"], "mentions_me": true,
            "ambient": true, "show_ids": true,
        }),
        "@ 她的不进 mentions，@ 同一个人两次只进一次"
    );
    assert_eq!(
        napcat.asked(),
        [BOT, WANG, NOBODY],
        "发过言的照缓存，问不到一次后面的不再问"
    );
    assert_eq!(
        *fields[3],
        json!({"msg": "4", "name": "阿杰", "reply_to": "1", "mentions_all": true, "ambient": true, "show_ids": true})
    );
    assert_eq!(
        fields[4]["media"],
        json!([
            {"kind": "image", "id": "a.jpg"},
            {"kind": "sticker", "id": "b.gif"},
            {"kind": "sticker", "id": "14", "name": "/微笑"},
            {"kind": "sticker", "id": "x.gif", "name": "[吃瓜]"},
            {"kind": "sticker", "id": "m1", "name": "[比心]"},
            {"kind": "voice", "id": "v.amr"},
            {"kind": "video", "id": "77"},
            {"kind": "file", "id": "/f-1", "name": "排班.pdf"},
        ])
    );
    for one in &said {
        assert_eq!(one["by"]["kind"], "external", "{one}");
        assert_eq!(one["by"]["venue"], venue.as_str(), "{one}");
        assert!(one["body"]["venue"].get("asleep").is_none(), "{one}");
    }
    assert_eq!(said[0]["by"]["id"], "qq:20002");
    assert_eq!(said[0]["by"]["role"], "member");
    assert_eq!(
        said[0]["cause"],
        format!("qq:{BOT}:1:{TIME}"),
        "命令编号同私聊的拼法"
    );
    assert_eq!(said[5]["by"]["role"], "manager", "规则的 managers");
    assert_eq!(
        said[6]["by"]["account"], "admin",
        "群里的终端管理员带 account"
    );
    assert!(
        events.iter().all(|event| event["kind"] != "turn.started"),
        "旁听的不开回合：{events:#?}"
    );
    assert!(script.requests().is_empty(), "不请求模型");
    let recalled = events
        .iter()
        .find(|event| event["kind"] == "venue.recalled")
        .expect("有撤回");
    assert_eq!(recalled["body"], json!({"msg": "1", "by": "qq:40004"}));

    let sleepy = venue_events(&home.root, &format!("qq:group:{SLEEPY}"));
    let sleepy = self::said(&sleepy);
    assert_eq!(sleepy.len(), 1, "{sleepy:?}");
    assert_eq!(
        sleepy[0]["body"]["venue"]["asleep"], true,
        "睡觉时间里的记 asleep"
    );
    assert!(
        venue_events(&home.root, &format!("qq:group:{BROKEN}")).is_empty(),
        "规则写了不存在的人格的群什么都不记"
    );
    let log = run_log(&home.root);
    let broken: Vec<&str> = log
        .lines()
        .filter(|line| line.contains("group not recorded"))
        .collect();
    assert_eq!(broken.len(), 1, "同一个群只记一行：{log}");
    assert!(broken[0].contains("venue=qq:group:777"), "{log}");
    assert!(broken[0].contains("reason=unknown_persona"), "{log}");
    assert!(!log.contains("大家好"), "原文不进运行日志");
    stopped(home).await;
}

#[tokio::test]
async fn slash_commands_in_a_group_answer_in_the_group() {
    let script = Script::new([Play::Says("不会说的")]);
    let (home, mut napcat, _) = started(&script, &rules(), "", MEMBERS).await;
    let venue = format!("qq:group:{GROUP}");
    napcat.send(group_frame(
        GROUP,
        LIN,
        1,
        json!([plain("大家好")]),
        ("小林", "lin"),
    ));
    // 终端管理员、管理的人能用；别人照核心那一句被拒，什么都不记；认不出的记成旁听。
    napcat.send(group_frame(
        GROUP,
        ADMIN,
        2,
        json!([plain("/stop")]),
        ("终端管理员", "o"),
    ));
    assert_eq!(napcat.group_reply(GROUP).await, STOPPED);
    napcat.send(group_frame(
        GROUP,
        LIN,
        3,
        json!([plain("  /clear")]),
        ("小林", "lin"),
    ));
    assert_eq!(napcat.group_reply(GROUP).await, NOT_ALLOWED);
    napcat.send(group_frame(
        GROUP,
        MANAGER,
        4,
        json!([plain("/clear")]),
        ("管理", "m"),
    ));
    let cleared = napcat.group_reply(GROUP).await;
    napcat.send(group_frame(
        GROUP,
        LIN,
        5,
        json!([plain("/xxx")]),
        ("小林", "lin"),
    ));
    let events = until_event(&home.root, &venue, |event| {
        event["kind"] == "message.user" && words(event) == "/xxx"
    })
    .await;
    let ran: Vec<(String, String)> = events
        .iter()
        .filter(|event| event["kind"] == "command.ran")
        .map(|event| {
            (
                event["body"]["command"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                event["cause"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        ran,
        [
            ("stop".to_string(), format!("qq:{BOT}:2:{TIME}/ran")),
            ("clear".to_string(), format!("qq:{BOT}:4:{TIME}/ran")),
        ],
        "回执：{cleared}"
    );
    assert_eq!(
        said(&events).iter().map(words).collect::<Vec<_>>(),
        ["大家好", "/xxx"]
    );
    assert!(napcat.pending().is_none(), "认不出的不回");
    assert!(script.requests().is_empty(), "不请求模型");
    stopped(home).await;
}

#[tokio::test]
async fn a_message_with_only_an_image_or_a_sticker_is_recorded() {
    let script = Script::new([]);
    let (home, napcat, _) = started(&script, &rules(), "", MEMBERS).await;
    let venue = format!("qq:group:{GROUP}");
    let image =
        json!([{"type": "image", "data": {"file": "only.jpg", "sub_type": 0, "summary": ""}}]);
    napcat.send(group_frame(GROUP, LIN, 1, image, ("小林", "lin")));
    let sticker = json!([{"type": "face", "data": {"id": "14", "raw": {"faceText": "/微笑"}}}]);
    napcat.send(group_frame(GROUP, JIE, 2, sticker, ("阿杰", "jie")));
    // 等桥办完第 2 条（记了判断）再看运行日志：「message sent in」那一行在核心的回应到了以后才写，核心记下第 2 条的时候它
    // 可能还没写；判断记在它后面，运行日志是同步写进文件的。
    let decided = format!("qq:{BOT}:2:{TIME}/decided");
    let events = until_event(&home.root, &venue, |event| event["cause"] == decided).await;
    let said = said(&events);
    assert_eq!(said.len(), 2, "{events:#?}");
    assert_eq!(said[0]["body"]["blocks"], json!([]), "没有字就没有内容块");
    assert_eq!(
        said[0]["body"]["venue"]["media"],
        json!([{"kind": "image", "id": "only.jpg"}])
    );
    assert_eq!(said[1]["body"]["blocks"], json!([]));
    assert_eq!(
        said[1]["body"]["venue"]["media"],
        json!([{"kind": "sticker", "id": "14", "name": "/微笑"}])
    );
    let log = run_log(&home.root);
    assert!(
        !log.contains("message refused"),
        "核心收了，没有 WARN：{log}"
    );
    assert_eq!(log.matches("message sent in").count(), 2, "{log}");
    stopped(home).await;
}
