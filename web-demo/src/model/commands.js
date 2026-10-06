// @ts-check
//! 斜杠命令怎么认、怎么筛（蓝图 `web.md`「斜杠命令」「命令列表」，规矩照 `tui.md`「斜杠命令列表」，做法照 TUI 演示的
//! `commands.rs`、`menu.rs`）。纯的；清单在 `resources/commands.json`，加命令只登记。
//!
//! - 像不像命令：`/` 开头，后面像个命令名（英文字母打头，只有英文字母、数字、`-`、`_`；刚打一个 `/` 也算）。
//!   `/你吃了吗`、路径 `/home/…` 不算，照普通的话发。
//! - 列表：没有空白时开；边打边筛，名字（或别名）开头的排前面、含着的排后面；`Esc` 关掉以后字变了才再开。
//! - 回车：`/名字` 是命令；带参数的命令名字后面空一格接着的字是参数；没有这个命令的弹「命令不存在」；
//!   别的后面跟了字的是一句话。

import { text, attachments } from './session.js';

/**
 * @typedef {{name: string, aliases?: string[], summary: string, run: string, args?: boolean}} Spec
 *   一条命令：名字（不带 `/`）、别名、列表里的说明、做什么（`ui/commands.js` 照它找处理）、名字后面能不能接参数
 */
/** @typedef {{kind: 'command', spec: Spec, words: string|null}|{kind: 'unknown'}|{kind: 'talk'}} Line 回车时框里那一行是什么 */

/** 框里的字像不像命令：像的交回命令名（刚打一个 `/` 的是空的），不像的是 `null`。名字要紧挨着 `/`。 */
export function typed(value) {
  if (!value.startsWith('/')) return null;
  const name = value.slice(1).split(/\s/u, 1)[0];
  return /^([A-Za-z][A-Za-z0-9_-]*)?$/.test(name) ? name : null;
}

/** 列表要不要开、照什么筛：像命令名，名字后面还没打空白（打了空白是在写参数，名字已经打全了）。 */
export function menuTyped(value) {
  const name = typed(value);
  if (name == null) return null;
  return value.length === 1 + name.length ? name : null;
}

/** 照打的字筛：名字或别名以它开头的排前面，含着它的排后面，各照清单的先后；不分大小写。 */
export function filter(list, name) {
  const want = name.toLowerCase();
  const starts = [];
  const contains = [];
  for (const spec of list) {
    const names = [spec.name, ...(spec.aliases ?? [])];
    if (names.some((n) => n.startsWith(want))) starts.push(spec);
    else if (names.some((n) => n.includes(want))) contains.push(spec);
  }
  return [...starts, ...contains];
}

/** 名字或别名正好是 `name` 的那一条；没有是 `null`。 */
export function find(list, name) {
  return list.find((s) => s.name === name || (s.aliases ?? []).includes(name)) ?? null;
}

/**
 * 回车时这一行是什么。
 * @param {Spec[]} list
 * @param {string} value 框里的字
 * @returns {Line}
 */
export function read(list, value) {
  const name = typed(value);
  if (name == null) return { kind: 'talk' };
  const words = value.slice(1 + name.length).trim();
  const spec = find(list, name);
  if (spec && !words) return { kind: 'command', spec, words: null };
  if (spec?.args) return { kind: 'command', spec, words };
  if (!spec && !words) return { kind: 'unknown' };
  return { kind: 'talk' };
}

/** 命令列表的状态：开没开、选中哪一条（照 TUI 演示的 `menu.rs`）。 */
export class Menu {
  constructor() {
    /** 选中的是筛出来的第几条。 */
    this.selected = 0;
    /** 上一次照什么名字筛的：变了，选中回到第一条。 */
    this.typed = /** @type {string|null} */ (null);
    /** 按 `Esc` 时框里的字；字没变就一直关着。 */
    this.dismissed = /** @type {string|null} */ (null);
  }

  /**
   * 照框里的字定开不开：开着的交回筛出来的几条，关着的（不像命令、`Esc` 关了、一条都没有）交回空的。
   * @param {string} value
   * @param {Spec[]} list
   */
  sync(value, list) {
    const name = menuTyped(value);
    if (name == null) {
      // 不是命令了（`/` 删掉了、打了空白）：Esc 关掉的那一次也算过去了
      this.typed = null;
      this.dismissed = null;
      return [];
    }
    if (this.typed !== name) {
      this.typed = name;
      this.selected = 0;
    }
    if (this.dismissed !== value) this.dismissed = null;
    const matches = filter(list, name);
    this.selected = Math.min(this.selected, Math.max(0, matches.length - 1));
    return this.dismissed == null ? matches : [];
  }

  /** 往下、往上挪一条，到头就停。 */
  step(down, count) {
    this.selected = down ? Math.min(this.selected + 1, Math.max(0, count - 1)) : Math.max(0, this.selected - 1);
  }

  /** 关掉，直到框里的字变了。 */
  dismiss(value) { this.dismissed = value; }
}

/**
 * 撤销成了，撤掉的那一轮里你说的话（整段，放回输入框，`tui.md`「输入框」第 7 条）：照回应里 `turn.reverted` 的序号找到
 * 撤掉的第一轮，再找引起它的那一条 `message.user`。那一条还没收到、那一轮不是人开的，是 `null`（由回应的 `said` 顶，
 * 它只有第一行，`protocol/undo.md`）。
 * @param {any[]} events 这个会话的持久事件
 * @param {number|undefined} seq 回应的 `events` 的第一条
 */
export function revertedSaid(events, seq) {
  const said = revertedMessage(events, seq);
  const words = said ? text(said) : '';
  return words.trim() ? words : null;
}

/**
 * 撤掉的第一轮是哪一条 `message.user` 开的（字和附件都从它取）；那一条还没收到、那一轮不是人开的是 `null`。
 * @param {any[]} events @param {number|undefined} seq 回应的 `events` 的第一条
 */
export function revertedMessage(events, seq) {
  const bySeq = new Map(events.map((e) => [e.seq, e]));
  const reverted = bySeq.get(seq);
  if (reverted?.kind !== 'turn.reverted' || !reverted.body.turns?.length) return null;
  const started = bySeq.get(Math.min(...reverted.body.turns));
  const said = bySeq.get(started?.body?.trigger);
  return said?.kind === 'message.user' && said.by?.kind === 'person' ? said : null;
}

/**
 * 一条 `message.user` 带的附件，换成附件包放回框里的样子（`{blob, name, media_type, size}`，核心存好的那一份）：大小照这台设备的
 * 输入历史（发的时候记下的），查不到的是 `null`；没名字的图片照媒体类型起一个（`image.png`）。
 * @param {any} said
 * @param {{parts?: Record<string, any>}[]} history 输入历史的条目
 */
export function keptAttachments(said, history) {
  const sizes = new Map();
  for (const item of history ?? []) for (const k of item.parts?.attachments ?? []) if (k?.blob) sizes.set(k.blob, k.size);
  return attachments(said).map((a) => ({
    blob: a.blob,
    name: a.name ?? `${a.kind}.${(a.media_type.split('/')[1] ?? 'bin').split('+')[0]}`,
    media_type: a.media_type,
    size: sizes.get(a.blob) ?? null,
  }));
}
