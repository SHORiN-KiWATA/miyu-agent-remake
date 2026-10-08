//! 随机测试里改标题、置顶（施工 3-8 三补）：另用一串随机数，每一例最后送一次，原来那串输入不跟着错开。夹在中间的话，
//! 多出来的事件、编号让原来那串输入走的路跟着变，难得走到的几条路就走不到了；停在回合中间的种子多，回合进行中的也就
//! 喂得到。什么时候来都收，回合进行中的带上回合；照看守的规矩，一样查每个命令恰好回应一次。

use super::*;
use crate::event::{VenueMessage, VenueRecalled};
use crate::id::ExternalId;
use crate::session::{Appended, ExtEvent};

/// 改一次：标题改成一个、去掉、不改，置顶、取消、不改，随便配；两格都不改的也有（接受，什么都不记）。
/// 随机数照种子 `seed` 另起一串。
pub(super) fn some_meta(seed: u64, next_id: &mut u64) -> Input {
    let mut rng = Rng(seed ^ 0x3E7A_0000);
    let title = [None, Some(""), Some("发版")][rng.below(3) as usize];
    let pinned = [None, Some(true), Some(false)][rng.below(3) as usize];
    Input::Command(Received {
        id: id(next_command(next_id)),
        by: alice(),
        at: at(30),
        command: Command::SetMeta {
            title: title.map(str::to_string),
            pinned,
        },
    })
}

/// 换工作区（施工 9-7 上）：照改标题的办法，每一例最后另送一次，另用一串随机数；目录换一个、不换，加进来的目录换成一个、
/// 清空、不换，随便配（都不换的接受，什么都不记）。什么时候来都收，回合进行中的带上回合。
pub(super) fn some_workspace(seed: u64, next_id: &mut u64) -> Input {
    let mut rng = Rng(seed ^ 0x9A70_0000);
    let cwd = ["~/src/miyu", "/work"][rng.below(2) as usize];
    let dirs: [Option<&[&str]>; 3] = [None, Some(&[]), Some(&["~/notes"])];
    let dirs = dirs[rng.below(3) as usize];
    Input::Command(Received {
        id: id(next_command(next_id)),
        by: alice(),
        at: at(32),
        command: Command::SetWorkspace {
            cwd: cwd.to_string(),
            dirs: dirs.map(|dirs| dirs.iter().map(|dir| (*dir).to_string()).collect()),
        },
    })
}

/// 记下用了一个斜杠命令（施工 O-6）：照改标题的办法，每一例最后另送一次；什么时候来都收。
pub(super) fn some_ran(next_id: &mut u64) -> Input {
    Input::Command(Received {
        id: id(next_command(next_id)),
        by: alice(),
        at: at(31),
        command: Command::Ran {
            text: "/reset".to_string(),
            command: "clear".to_string(),
        },
    })
}

/// 场所里旁听的一句（施工 O-13 上）：照改标题的办法，每一例最后另送一次。只记下，不开回合，回合进行中也不排进这一轮。
pub(super) fn some_overheard(next_id: &mut u64) -> Input {
    Input::Command(Received {
        id: id(next_command(next_id)),
        by: alice(),
        at: at(33),
        command: Command::Send {
            blocks: vec![Block::Text(Text {
                text: "今天谁值班".to_string(),
            })],
            urgent: false,
            venue: Some(VenueMessage {
                msg: "8810".to_string(),
                ambient: true,
                ..VenueMessage::default()
            }),
        },
    })
}

/// 桥记的一条（施工 O-13 上）：照改标题的办法，每一例最后另送一次；扩展自己的、撤回，随便挑。什么时候来都收，不带回合。
pub(super) fn some_appended(seed: u64, next_id: &mut u64) -> Input {
    let mut rng = Rng(seed ^ 0x0A13_0000);
    let event = match rng.below(2) {
        0 => Appended::Ext(
            ExtEvent::new("ext.onebot.chat.decided", serde_json::json!({"to": [1]}))
                .expect("ext. 开头的"),
        ),
        _ => Appended::Recalled(VenueRecalled {
            msg: "8810".to_string(),
            by: ExternalId::parse("qq:20017").expect("合写法"),
        }),
    };
    Input::Command(Received {
        id: id(next_command(next_id)),
        by: alice(),
        at: at(34),
        command: Command::Append { event },
    })
}
