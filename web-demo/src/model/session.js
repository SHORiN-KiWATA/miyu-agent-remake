// @ts-check
//! 左栏的一项，从这个会话的事件推出来：标题、在不在跑、什么时候开的（蓝图 `web.md`「会话表的一项」）。
//!
//! 核心的 `session.list` 现在只给编号和是不是一次性的，会话列表流还没有（`04-核心协议.md` 第五节、
//! 蓝图 `protocol.md`）；先在头里从日志推，核心有了列表流只换数据源。

/**
 * @param {string} id 会话编号
 * @param {any[]} events 这个会话的持久事件，照序号
 * @returns {{session: string, title: string|null, running: boolean, created: string|null}}
 */
export function summarize(id, events) {
  let title = null;
  let first = null;
  let running = false;
  for (const e of events) {
    // 去掉标题写成空的（`kernel/events-bodies.md`）：回到照第一句话写
    if (e.kind === 'session.meta_changed' && typeof e.body.title === 'string') title = e.body.title || null;
    if (e.kind === 'message.user' && first == null) first = text(e);
    if (e.kind === 'turn.started') running = true;
    if (e.kind === 'turn.ended') running = false;
  }
  // 没起名字的拿第一句话顶：换行换成空格，一行里放不下由界面截掉
  const fallback = first ? first.replace(/\s+/g, ' ').trim() : '';
  return { session: id, title: title ?? (fallback || null), running, created: events[0]?.at ?? null };
}

/** 一条 `message.user` 里的字：文字块接起来（现在经协议发来的只有一块文字，蓝图 `protocol.md`）。 */
export function text(e) {
  return (e.body.blocks ?? []).filter((b) => b.type === 'text').map((b) => b.text).join('\n');
}

/**
 * 一条消息里的附件（图片、文件块，`kernel/blocks.md`），照先后；格都写上，没有的是 `null`（蓝图 `web.md`「附件」第 6 条）。
 * @returns {{kind: 'image'|'file', blob: string, media_type: string, width: number|null, height: number|null, name: string|null}[]}
 */
export function attachments(e) {
  return (e.body.blocks ?? []).filter((b) => b.type === 'image' || b.type === 'file').map((b) => ({
    kind: b.type,
    blob: b.blob,
    media_type: b.media_type,
    width: b.width ?? null,
    height: b.height ?? null,
    name: b.name ?? null,
  }));
}
