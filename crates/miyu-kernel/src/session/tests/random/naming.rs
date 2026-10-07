//! 随机测试里改标题、置顶（施工 3-8 三补）：另用一串随机数，每一例最后送一次，原来那串输入不跟着错开。夹在中间的话，
//! 多出来的事件、编号让原来那串输入走的路跟着变，难得走到的几条路就走不到了；停在回合中间的种子多，回合进行中的也就
//! 喂得到。什么时候来都收，回合进行中的带上回合；照看守的规矩，一样查每个命令恰好回应一次。

use super::*;

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
