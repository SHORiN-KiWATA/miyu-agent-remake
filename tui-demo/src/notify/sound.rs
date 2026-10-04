//! 提示音（蓝图 `tui.md`「系统通知」第 5 条）：内置的 `wake.wav`（旧版合成的木琴音），第一次用时写到机器缓存
//! 目录的 `tui/sounds/wake.wav`，大小对不上就重写；照清单依次试放音的程序，哪个拉得起来用哪个。

use std::fs;
use std::path::{Path, PathBuf};

use super::system::spawn;

/// 内置的提示音。
const WAKE: &[u8] = include_bytes!("../../resources/sounds/wake.wav");

/// 把提示音写进 `dir`（已经在、大小对的不重写），交回文件；写不进去的是 `None`。
pub fn materialize(dir: &Path) -> Option<PathBuf> {
    let file = dir.join("wake.wav");
    let fresh = fs::metadata(&file).is_ok_and(|m| m.len() == WAKE.len() as u64);
    if !fresh {
        fs::create_dir_all(dir).ok()?;
        fs::write(&file, WAKE).ok()?;
    }
    Some(file)
}

/// 放一声：照 `players` 的先后，拉得起来就算；一个都拉不起来的什么都不做。
pub fn play(players: &[Vec<String>], file: &Path) {
    for player in players {
        let Some((program, args)) = player.split_first() else {
            continue;
        };
        let mut args = args.to_vec();
        args.push(file.display().to_string());
        if spawn(program, &args) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WAKE, materialize};

    #[test]
    fn the_sound_is_written_once_and_rewritten_when_it_goes_wrong() {
        let dir = std::env::temp_dir().join(format!("miyu-sound-{}", std::process::id()));
        let file = materialize(&dir).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), WAKE);
        std::fs::write(&file, b"broken").unwrap();
        materialize(&dir).unwrap();
        assert_eq!(
            std::fs::read(&file).unwrap().len(),
            WAKE.len(),
            "大小不对就重写"
        );
        std::fs::remove_dir_all(&dir).unwrap_or_default();
    }
}
