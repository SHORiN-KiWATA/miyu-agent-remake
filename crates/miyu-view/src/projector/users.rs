//! 消息和排着的话（终端蓝图 `tui.md`「运行状态行和排队的消息」第 5 条）：回答进行中来的话先排着；哪一次请求看到了它，
//! 它就被听到了：前面那段收起，它挪到末尾，接着的步另起一段。被退回的不画。一次听到的几句照先后挨着排，一句一条。

use miyu_kernel::block::Block;
use miyu_kernel::event::{Event, MessageUser, MessageWithdrawn, Permission};
use miyu_kernel::id::Seq;
use miyu_kernel::origin::By;

use super::Projector;
use crate::entry::{Attachment, Body, Entry, EntryId, User};

/// 权限级别写成字：`workspace`、`full`，只读的是 `read_only`。
pub(super) fn level(permission: &Permission) -> String {
    match permission.read_only {
        true => "read_only".to_string(),
        false => permission.level.as_str().to_string(),
    }
}

impl Projector {
    /// 一条消息落了盘。
    pub(super) fn message(&mut self, event: &Event, message: &MessageUser) {
        let id = EntryId::message(event.seq);
        let ambient = message.venue.as_ref().is_some_and(|venue| venue.ambient);
        // 回答进行中来的、不是旁听的：排着，等哪一次请求看到它。
        let queued = self.turn.is_some() && !ambient;
        let (text, attachments) = content(&message.blocks);
        let user = User {
            text,
            attachments,
            from: from(&event.by),
            level: self.level.clone(),
            queued,
            withdrawn: false,
        };
        if queued {
            self.queued.insert(event.seq, id.clone());
        }
        self.users.insert(event.seq, id.clone());
        // 排着的不收起在进行的那一段：被听到时才收起、挪到末尾。
        if !queued {
            self.close_group();
        }
        self.add(Entry {
            id,
            body: Body::User(user),
            turn: None,
            hidden: false,
            at: event.at,
        });
    }

    /// 排着的话被退回了：不画。
    pub(super) fn withdrawn(&mut self, withdrawn: &MessageWithdrawn) {
        for seq in &withdrawn.messages {
            self.queued.remove(seq);
            if let Some(id) = self.users.get(seq).cloned() {
                self.touch(&id, |entry| {
                    if let Body::User(user) = &mut entry.body {
                        user.queued = false;
                        user.withdrawn = true;
                    }
                });
            }
        }
    }

    /// 这一轮的一次请求看到了第 `seen` 条为止：排着的话序号不超过它的被听到了。
    pub(super) fn heard(&mut self, seen: Seq) {
        let picked: Vec<Seq> = self.queued.range(..=seen).map(|(seq, _)| *seq).collect();
        self.bring_in(&picked);
    }

    /// 这几条归到在跑的这一轮：排着的收起前面那段、挪到末尾；本来不排着的只记上这一轮。
    pub(super) fn bring_in(&mut self, seqs: &[Seq]) {
        let turn = self.turn.as_ref().map(|t| t.id);
        let mut closed = false;
        for seq in seqs {
            let Some(id) = self.users.get(seq).cloned() else {
                continue;
            };
            match self.queued.remove(seq) {
                Some(_) => {
                    if !closed {
                        self.close_blocks();
                        self.close_group();
                        closed = true;
                    }
                    self.move_to_end(&id, |entry| {
                        entry.turn = turn;
                        if let Body::User(user) = &mut entry.body {
                            user.queued = false;
                        }
                    });
                }
                None => self.touch(&id, |entry| entry.turn = turn),
            }
        }
    }

    /// 用了斜杠命令、没在回答：留着的排队话不再接着发，挪到末尾照你说的画，不归到哪一轮（`/stop`）。
    pub(super) fn settle(&mut self) {
        if self.turn.is_some() {
            return;
        }
        let left: Vec<(Seq, EntryId)> = std::mem::take(&mut self.queued).into_iter().collect();
        for (_, id) in left {
            self.move_to_end(&id, |entry| {
                if let Body::User(user) = &mut entry.body {
                    user.queued = false;
                }
            });
        }
    }
}

/// 几块文字照先后接起来；图、文件成附件。
fn content(blocks: &[Block]) -> (String, Vec<Attachment>) {
    let mut text = String::new();
    let mut attachments = Vec::new();
    for block in blocks {
        match block {
            Block::Text(piece) => text.push_str(&piece.text),
            Block::Image(image) => attachments.push(Attachment {
                kind: "image",
                name: image.name.as_ref().map(|n| n.as_str().to_string()),
                media_type: image.media_type.as_str().to_string(),
                blob: image.blob.clone(),
                width: Some(image.width),
                height: Some(image.height),
                path: image.path.as_ref().map(|p| p.as_str().to_string()),
            }),
            Block::File(file) => attachments.push(Attachment {
                kind: "file",
                name: Some(file.name.as_str().to_string()),
                media_type: file.media_type.as_str().to_string(),
                blob: file.blob.clone(),
                width: None,
                height: None,
                path: file.path.as_ref().map(|p| p.as_str().to_string()),
            }),
            _ => {}
        }
    }
    (text, attachments)
}

/// 别处来的话的来处：人自己说的没有。
fn from(by: &By) -> Option<By> {
    match by {
        By::Person(person) if person.via.is_none() => None,
        other => Some(other.clone()),
    }
}
