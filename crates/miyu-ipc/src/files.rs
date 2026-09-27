//! `run/` 下的两个小文件（`docs/designs/07-存储.md` 第二节）：本机令牌 `run/token`，套接字的实际
//! 位置 `run/socket`。
//!
//! 核心每次起来都重写：先写临时文件再改名，读的人读不到半个。不同步：断电丢了也不要紧，下次起来
//! 再写一遍。Unix 上权限 0600，只有自己能读。

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use miyu_store::root::DataRoot;

use crate::sys;

/// 本机令牌，在 `run/` 里。
const TOKEN: &str = "token";

/// 套接字的实际位置，在 `run/` 里。
const LOCATION: &str = "socket";

/// 换一个本机令牌：32 个随机字节，写成 64 位十六进制。旧的就作废了。
///
/// # Errors
///
/// 取不到随机数；写不进。
pub(crate) fn renew_token(root: &DataRoot) -> io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)?;
    let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    replace(&root.run().join(TOKEN), format!("{token}\n").as_bytes())?;
    Ok(token)
}

/// 现读本机令牌。
///
/// # Errors
///
/// 读不到。
pub(crate) fn read_token(root: &DataRoot) -> io::Result<String> {
    let text = fs::read_to_string(root.run().join(TOKEN))?;
    Ok(text.trim_end().to_string())
}

/// 记下套接字的实际位置：一行，套接字的路径。
///
/// # Errors
///
/// 写不进。
pub(crate) fn write_location(root: &DataRoot, socket: &Path) -> io::Result<()> {
    let mut line = socket.as_os_str().as_encoded_bytes().to_vec();
    line.push(b'\n');
    replace(&root.run().join(LOCATION), &line)
}

/// 读套接字的实际位置。
///
/// # Errors
///
/// 读不到；写的不是绝对路径。
pub(crate) fn read_location(root: &DataRoot) -> io::Result<PathBuf> {
    let mut line = fs::read(root.run().join(LOCATION))?;
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    let path = sys::path_from_bytes(line)?;
    match path.is_absolute() {
        true => Ok(path),
        false => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("run/socket 写的不是绝对路径：{}", path.display()),
        )),
    }
}

/// 换掉 `path` 的内容：先写旁边的临时文件，再改名盖过去。上次留下的临时文件先删掉，拿着锁的核心
/// 才写，不会有别人在写它。
fn replace(path: &Path, content: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    match fs::remove_file(&temporary) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut file = sys::create_private(&temporary)?;
    file.write_all(content)?;
    drop(file);
    fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests;
