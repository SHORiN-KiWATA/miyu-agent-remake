//! 卸掉一个包以后忘掉它的配置项（施工 F-8 中下补，设计 `31-软件包.md` 第三节第 3 条、定了的 H）：系统配置、个人设置里这个包
//! 的几项照原文删掉（`miyu_config::edit` 的 `Unset`），走 `config.set` 那一条写盘、记日志、推 `config.changed` 的路，改的人记成
//! 这台机器的管理员。项目目录里的 `.miyu/config.toml` 在别人的仓库里，不碰；密钥文件不碰（一条密钥可能别处也在引用）。
//! 删不掉的那一项（写成了别的样子）跳过；写不进的记一行，不算卸失败。

use miyu_config::Layer;
use miyu_config::edit::{self, Change};
use miyu_kernel::origin::{By, Person};
use miyu_store::config_file::{self, ConfigText, WriteError};

use super::TARGET;
use super::observe::observe;
use super::push::Via;
use super::set::{TRIES, done};
use crate::Core;

/// 系统配置、个人设置里删掉 `keys`（真的键，例如 `onebot.listen`）。
pub(crate) fn forget(core: &Core, keys: &[String]) {
    if keys.is_empty() {
        return;
    }
    let by = By::Person(Person::new(core.config().places.account.clone()));
    for layer in [Layer::System, Layer::Personal] {
        let mut config = core.config();
        for _ in 0..TRIES {
            observe(core, &mut config, layer);
            let file = config.file(layer);
            if file.version.is_none() {
                break;
            }
            let text = keys.iter().fold(file.text.clone(), |text, key| {
                edit::apply(&text, Change::Unset(key)).unwrap_or(text)
            });
            if text == file.text {
                break;
            }
            let bytes = config_file::bytes(&text, file.bom);
            match config_file::write(&file.path, &bytes, file.version.as_deref()) {
                Ok(()) => {
                    let written = ConfigText {
                        version: config_file::version(&bytes),
                        text,
                        bom: file.bom,
                    };
                    done(
                        core,
                        &mut config,
                        layer,
                        written,
                        (Via::Set, by.clone()),
                        None,
                    );
                    break;
                }
                Err(WriteError::Changed) => {}
                Err(WriteError::Io(error)) => {
                    tracing::warn!(target: TARGET, file = %file.shown, error = %error, "config not written");
                    break;
                }
            }
        }
    }
}
