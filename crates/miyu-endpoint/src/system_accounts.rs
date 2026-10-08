//! 系统账号（施工 O-4 下，`docs/construction/O-4-系统账号（下）.md`，`06-多用户与身份.md` U14）：声明了系统账号的包各一个，
//! 账号名就是包的编号。照核心起来时读到的清单算，不落盘：`home/<编号>/` 就是它在磁盘上的样子。核心拉起的这个包的扩展以它的
//! 身份连进来；它的东西管理员能看能管，它没有自己的记忆。

use std::collections::BTreeMap;
use std::sync::Arc;

use miyu_kernel::id::AccountId;

use crate::Core;

/// 运行日志的目标。
const TARGET: &str = "miyu::endpoint";

impl Core {
    /// 这次起来认的系统账号，照编号排。
    pub(crate) fn system_accounts(&self) -> Vec<AccountId> {
        miyu_store::packages::system_accounts(&self.packages)
    }

    /// `account` 是不是这次起来认的系统账号。
    pub(crate) fn is_system(&self, account: &AccountId) -> bool {
        self.system_accounts().contains(account)
    }

    /// 核心认的账号：管理员、系统账号。会话列表的索引、用量、记忆只给它们的会话（别的账号随多用户）。
    pub(crate) fn knows(&self, account: &AccountId) -> bool {
        *account == self.admin || self.is_system(account)
    }

    /// 管理员和系统账号：用量照它们补。
    pub(crate) fn accounts(&self) -> Vec<AccountId> {
        std::iter::once(self.admin.clone())
            .chain(self.system_accounts())
            .collect()
    }

    /// 连接是谁：核心拉起的包 `package` 声明了系统账号的是它，别的是管理员（本机连上来的都是他）。
    pub(crate) fn account_of(&self, package: Option<&str>) -> AccountId {
        package
            .and_then(|package| AccountId::parse(package).ok())
            .filter(|account| self.is_system(account))
            .unwrap_or_else(|| self.admin.clone())
    }

    /// 属主是 `owner` 的会话，出厂、系统区的人格的记忆照谁算（`personas.md`「怎么走」第 5 条）：系统账号的归管理员。
    pub(crate) fn memory_owner(&self, owner: &AccountId) -> AccountId {
        match self.is_system(owner) {
            true => self.admin.clone(),
            false => owner.clone(),
        }
    }
}

/// 起来时建系统账号的家目录和工作区（已经有的不动），开它们各自的会话列表的索引。建不成的记一行，账号照样算有：用到时再报。
/// 只开一次：再调的什么都不做。
pub(crate) fn prepare(core: &Core) {
    if core.system_indexes.get().is_some() {
        return;
    }
    let mut indexes = BTreeMap::new();
    for account in core.system_accounts() {
        if let Err(error) = core.root.prepare_home(&account) {
            tracing::warn!(target: TARGET, account = account.as_str(), kind = ?error.kind(), "system account not prepared");
        }
        let index = Arc::new(crate::list::open_index(&core.root, &account));
        indexes.insert(account, index);
    }
    if core.system_indexes.set(indexes).is_err() {
        // 同时有别人开好了：用它的。
    }
}
