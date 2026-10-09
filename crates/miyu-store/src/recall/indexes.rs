//! 回合库的登记（施工 R-2 上，`docs/blueprint/memory.md`「怎么走」第一条第 4、7 款）：一个账号、一个人格一份，
//! `home/<账号>/index/recall/turns-<人格>.db`。开过的留着、一直开着：一个库一个进程只开一个连接（`docs/designs/07-存储.md`
//! 第六节）。

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use miyu_kernel::id::{AccountId, SessionId};
use miyu_recall::Source;

use crate::root::DataRoot;

use super::room::{DB as SUFFIX, TURNS_PREFIX as PREFIX, recall_dir};
use super::{DbError, Opened, RecallIndex, Room};

/// 一份回合库这一回第一次开时叫的（施工 R-2 下）：哪一间、开得怎么样。会话那一层照它记运行日志、补齐旧会话。在开库的那个
/// 线程里叫，登记的锁已经放开（它可以再来开库）。
pub type Opener = Box<dyn Fn(&Room, &Opened) + Send + Sync>;

/// 回合库的登记：照房间开、留着。核心里一份，会话表交给每个会话（施工 R-2 上）。
pub struct RecallIndexes {
    /// 数据根：库在账号的 `index/recall/` 下。
    root: DataRoot,
    /// 开过的。
    open: Mutex<BTreeMap<Room, Arc<RecallIndex>>>,
    /// 第一次开时叫的（[`RecallIndexes::when_opened`]）；没登记的不叫。
    opener: OnceLock<Opener>,
}

impl std::fmt::Debug for RecallIndexes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecallIndexes")
            .field("root", &self.root)
            .field("open", &self.open)
            .finish_non_exhaustive()
    }
}

impl RecallIndexes {
    /// 一份空的登记，用到哪一份才开哪一份。
    pub fn new(root: &DataRoot) -> RecallIndexes {
        RecallIndexes {
            root: root.clone(),
            open: Mutex::new(BTreeMap::new()),
            opener: OnceLock::new(),
        }
    }

    /// 登记第一次开一份库时叫谁（施工 R-2 下）。只认第一次登记的：交回 `false` 的是已经有了，这一个没登记上。
    pub fn when_opened(&self, opener: Opener) -> bool {
        self.opener.set(opener).is_ok()
    }

    /// 房间 `room` 的回合库（施工 R-3 下照房间开：跟着人格的、只在会话里的）。这一回第一次用、刚开的，另交回开库的情形
    /// （[`Opened`]），也叫登记过的 [`Opener`]（施工 R-2 下）；开过的交回同一份，情形是空的。用不了的照样交回一份：它什么都找不到、写什么都
    /// 不写。
    pub fn turns(&self, room: &Room) -> (Arc<RecallIndex>, Option<Opened>) {
        let (index, opened) = {
            let mut open = self.lock();
            if let Some(index) = open.get(room) {
                return (Arc::clone(index), None);
            }
            let (index, opened) = RecallIndex::open(&room.turns(&self.root));
            let index = Arc::new(index);
            open.insert(room.clone(), Arc::clone(&index));
            (index, opened)
        };
        if let Some(opener) = self.opener.get() {
            opener(room, &opened);
        }
        (index, Some(opened))
    }

    /// 删会话（进回收处）时：拿掉这个账号每一份回合库里会话 `session` 的全部和它照到了哪，埋一块整个会话的墓碑
    /// `会话/`（施工 R-3 上：记忆的出处在它里面的都算死了）。磁盘上有的都算，没开过的这时开（核心重启以后，别的人格的
    /// 会话也删得掉）。
    ///
    /// # Errors
    ///
    /// 列不出目录；有一份写不进（其余的照样拿掉，报最后一个错）。
    pub fn forget_session(&self, account: &AccountId, session: &SessionId) -> Result<(), DbError> {
        let mut personas: Vec<String> = Vec::new();
        match fs::read_dir(recall_dir(&self.root, account)) {
            Ok(entries) => {
                for entry in entries {
                    let name = entry?.file_name();
                    let persona = name
                        .to_str()
                        .and_then(|name| name.strip_prefix(PREFIX))
                        .and_then(|name| name.strip_suffix(SUFFIX));
                    if let Some(persona) = persona {
                        personas.push(persona.to_string());
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let source = session.to_string();
        let mut failed = None;
        let tomb = format!("{source}/");
        for persona in personas {
            let (index, _) = self.turns(&Room::persona(account, &persona));
            // 先埋再拿掉（施工 R-2 下）：埋了以后补进来的一批写不进（`apply`），拿掉以后不会又多出来。
            if let Err(error) = index.bury(&tomb).and_then(|()| index.forget(&source)) {
                failed = Some(error);
            }
        }
        failed.map_or(Ok(()), Err)
    }

    /// 记忆的一处出处 `source` 还活着（施工 R-3 上，`memory.md` 第二条第 4 款）：那个会话的目录还在（没删进回收处、没清出回收处，
    /// 施工 R-3 三补），房间 `room` 的回合库里那一轮没埋（没撤销）。
    ///
    /// 会话照目录判、不照删会话时埋的那块墓碑：墓碑是派生的，回合库坏了删掉重建就没了，回收处里的会话没人再读出来补；目录是
    /// 真相，从回收处恢复的也自然又算活的。先看这一间的账号名下（平常就在这里），没有再找别的账号（系统账号的会话、记忆归管理员）。
    ///
    /// # Errors
    ///
    /// 回合库读不了；读不了数据根的 `home/`。
    pub fn alive(&self, room: &Room, source: &Source) -> Result<bool, DbError> {
        let account = match room {
            Room::Persona { account, .. } | Room::Session { account, .. } => account,
        };
        let here = self.root.session_dir(account, &source.session).is_dir();
        if !here && self.root.owner_of(&source.session)?.is_none() {
            return Ok(false);
        }
        let (index, _) = self.turns(room);
        let turn = format!("{}/{}", source.session, source.turn.started().get());
        Ok(!index.is_buried(&turn)?)
    }

    /// 拿锁。别的线程拿着锁崩了，登记还是好的（只是一张表），照常用。
    fn lock(&self) -> MutexGuard<'_, BTreeMap<Room, Arc<RecallIndex>>> {
        self.open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
