// @ts-check
//! 命令列表和执行（蓝图 `web.md`「斜杠命令」「命令列表」，照 `tui.md`「斜杠命令列表」，样子是网页的）。
//!
//! 列表浮在输入框上面，左边和框对齐、和框一样宽，不推正文；开不开、筛出哪几条、选中哪一条由 `model/commands.js` 的
//! `Menu` 定，这里只画、接键和鼠标。最多露 `layout.json` 的 `command_rows` 条，多的在里面滚，选中的滚进视野。
//!
//! 执行照清单里的 `run`，一种一个处理（`RUNS`）：加命令只登记（`resources/commands.json`），要新的一种才动这里。
//! 命令不进正文、不发给她；核心拒绝的照原因码写一句短的（`text/zh.json` 的 `refusals`），查不到的写核心的原话，
//! 都在提示那个小框里。

import { h, replace } from './dom.js';
import { show, hide } from '../lib/motion.js';
import { res, t } from '../util/res.js';
import { Menu, revertedSaid } from '../model/commands.js';
import { project } from '../model/transcript.js';
import { copy } from '../markdown/build.js';
import { Refusal } from '../core/connection.js';
import { openPackages } from './packages.js';

/** @typedef {import('../model/commands.js').Spec} Spec */
/** @typedef {import('./app.js').App} App */

/**
 * 斜杠命令的清单（蓝图 `web/architecture.md`「上下文」的 `ctx.commands`）：出厂的一份（`resources/commands.json`，做法在 `RUNS`）
 * 加上软件包登记的（做法跟着规格一起交进来：`spec.do(words)`）；包停用了它登记的跟着没。当服务 `commands` 给别的包用。
 */
export class Commands {
  /** @param {Spec[]} base 出厂的一份 */
  constructor(base) {
    this.base = base;
    /** @type {Spec[]} 软件包登记的 */
    this.extra = [];
  }

  /** 现在的全部：出厂的在前，登记的照登记的先后。 */
  list() { return [...this.base, ...this.extra]; }

  /** 登记一条：`run` 是做法，拿到命令名后面的字；交回怎么拿掉。 */
  register(spec, run) {
    const row = { ...spec, run: 'package', do: run };
    this.extra.push(row);
    return () => {
      const i = this.extra.indexOf(row);
      if (i >= 0) this.extra.splice(i, 1);
    };
  }

  /** 按包绑一份：登记的跟着那个包撤回。 */
  bind(ctx) {
    return { register: (spec, run) => ctx.effect(() => this.register(spec, run)), list: () => this.list() };
  }
}

/** 浮在输入框上面的命令列表。 */
export class CommandList {
  /**
   * @param {{run: (spec: Spec) => void, fill: (text: string) => void}} on 执行选中的；把名字填进框里
   * @param {() => Spec[]} specs 现在的全部命令（出厂的加软件包登记的）
   */
  constructor(on, specs) {
    this.on = on;
    this.specs = specs;
    this.menu = new Menu();
    /** @type {Spec[]} */
    this.matches = [];
    this.value = '';
    /** 上一次鼠标在哪：列表在鼠标底下滚动时浏览器也发 `mousemove`，位置没变的不算悬停。 */
    this.pointer = '';
    this.head = h('div.commands-head');
    this.list = h('div.commands-list', { role: 'listbox', style: `--rows: ${res.layout.command_rows}` });
    this.el = h('div.dock-commands.dock-float', { hidden: true }, this.head, this.list);
  }

  get open() { return this.matches.length > 0; }

  /** 框里的字变了（或者要照它重看一遍）：定开不开、筛出哪几条。 */
  update(value) {
    this.value = value;
    this.matches = this.menu.sync(value, this.specs());
    // 开：从下面升上来；关：往下沉、淡出（蓝图「动效」）
    if (!this.open) {
      hide(this.el);
      return;
    }
    show(this.el);
    this.head.textContent = t('commands.header', { count: this.matches.length });
    replace(this.list, this.matches.map((spec, i) => this.row(spec, i)));
    this.mark(true);
  }

  row(spec, i) {
    const aliases = spec.aliases?.length ? h('span.command-alias', ` (${spec.aliases.join(', ')})`) : null;
    return h('div.command-row', {
      role: 'option',
      // 焦点留在输入框里
      onmousedown: (/** @type {MouseEvent} */ ev) => ev.preventDefault(),
      onmousemove: (/** @type {MouseEvent} */ ev) => this.hover(ev, i),
      onclick: () => this.on.run(spec),
    }, h('span.command-name', `/${spec.name}`, aliases), h('span.command-summary', spec.summary));
  }

  /** 画选中的那一条；`scroll` 时把它滚进视野（只滚列表自己，不带着整页动）。 */
  mark(scroll) {
    const rows = /** @type {HTMLElement[]} */ ([...this.list.children]);
    rows.forEach((r, i) => {
      r.classList.toggle('is-selected', i === this.menu.selected);
      r.setAttribute('aria-selected', String(i === this.menu.selected));
    });
    const row = rows[this.menu.selected];
    if (!scroll || !row) return;
    const top = row.offsetTop;
    const bottom = top + row.offsetHeight;
    if (top < this.list.scrollTop) this.list.scrollTop = top;
    else if (bottom > this.list.scrollTop + this.list.clientHeight) this.list.scrollTop = bottom - this.list.clientHeight;
  }

  hover(ev, i) {
    const at = `${ev.clientX},${ev.clientY}`;
    if (at === this.pointer || i === this.menu.selected) {
      this.pointer = at;
      return;
    }
    this.pointer = at;
    this.menu.selected = i;
    this.mark(false);
  }

  /** 开着时 `↑` `↓` 选、`Tab` 补全、`Enter` 执行、`Esc` 关掉（字变了才再开）。接了的交回 `true`。 */
  key(ev) {
    if (!this.open || ev.ctrlKey || ev.metaKey || ev.altKey) return false;
    const spec = this.matches[this.menu.selected];
    if (ev.key === 'ArrowDown' || ev.key === 'ArrowUp') {
      this.menu.step(ev.key === 'ArrowDown', this.matches.length);
      this.mark(true);
    } else if (ev.key === 'Tab' && !ev.shiftKey) {
      this.on.fill(`/${spec.name}${spec.args ? ' ' : ''}`);
    } else if (ev.key === 'Enter' && !ev.shiftKey) {
      this.on.run(spec);
    } else if (ev.key === 'Escape') {
      this.menu.dismiss(this.value);
      this.update(this.value);
    } else {
      return false;
    }
    ev.preventDefault();
    return true;
  }
}

/**
 * 执行一条命令。`words` 是名字后面的字（带参数的命令才有）。核心拒绝的、出了错的都写进提示，不往外抛。
 * @param {App} app
 * @param {Spec} spec
 * @param {string|null} words
 */
export async function runCommand(app, spec, words) {
  try {
    // 软件包登记的：做法跟着规格来
    if (spec.do) {
      await spec.do(words);
      return;
    }
    const run = RUNS[spec.run];
    if (!run) throw new Error(`commands.json 里 /${spec.name} 的 run 没有对应的做法：${spec.run}`);
    await run(app, spec, words);
  } catch (err) {
    if (!(err instanceof Refusal)) console.error(err);
    app.composer.say(refusalText(err));
  }
}

/** 核心拒绝的写一句短的：照原因码查 `refusals`，查不到的写核心的原话；别的错写它自己的话。 */
export function refusalText(err) {
  return err instanceof Refusal ? res.text.refusals[err.reason ?? ''] ?? err.message : err.message;
}

/** @type {Record<string, (app: App, spec: Spec, words: string|null) => unknown>} */
const RUNS = {
  revert: undo,
  unrevert: restore,
  compact: async (app, spec, words) => {
    const session = opened(app);
    if (session) await app.store.conn.request('session.compact', words ? { session, instructions: words } : { session });
  },
  // 清空上下文（`session.clear`，照 TUI）：还没开的新会话当场说「上下文为空」，不去开会话
  clear: async (app) => {
    if (!app.current) {
      app.composer.say(res.text.refusals.nothing_to_clear);
      return;
    }
    await app.store.conn.request('session.clear', { session: app.current });
  },
  // 回顾（蓝图「回顾」）：提示「正在回顾…」；推来的 `session.recapped` 照日志画，`cached` 的照回应在正文末尾再画一次；
  // 还没开的新会话没什么可回顾，不去开会话
  recap: async (app) => {
    const session = app.current;
    if (!session) {
      app.composer.say(res.text.refusals.nothing_to_recap);
      return;
    }
    app.composer.say(t('commands.recap_working'));
    const got = await app.store.conn.request('session.recap', { session });
    if (got?.cached) app.recapAgain(session, got.text);
    if (app.current === session) requestAnimationFrame(() => app.chat.reveal());
  },
  theme: async (app) => {
    const next = await app.ctx.theme?.next();
    if (next) app.composer.say(t('commands.theme_changed', { name: next.name }));
  },
  // 界面语言（蓝图「界面语言」第 3 条）：不带参数的开一个浮层选；写了 auto 或表里的一种直接换，写别的提示能写哪些
  language: (app, spec, words) => {
    const lang = app.ctx.language;
    const want = (words ?? '').trim();
    if (!want) {
      const { items, current } = lang.options();
      app.composer.picker.show({
        title: t('language.title'),
        hint: t('language.hint'),
        note: t('language.current'),
        items: items.map((x) => ({ label: x.auto ? t('language.auto', { name: x.name }) : x.name })),
        current,
        // 选的就是现在写的那一样：只关掉
        choose: (i) => { if (i !== current) setLanguage(app, items[i].value); },
      });
      return;
    }
    if (!lang.choices.includes(want)) {
      app.composer.say(t('commands.language_unknown', { choices: lang.choices.join(t('list_sep')) }));
      return;
    }
    setLanguage(app, want);
  },
  // 重做、编辑最新一轮（蓝图「斜杠命令」，照 TUI）：在回答时、最新一轮不是你开的都不做，提示一句，不找核心
  redo: (app) => {
    if (latestTurn(app)) redo(app, null);
  },
  edit: (app) => {
    if (latestTurn(app)) app.chat.editLatest();
  },
  // 全部会话那一页（蓝图「全部会话」）
  sessions: (app) => app.sessionsPage.open(),
  new: (app) => {
    app.open(null);
    app.composer.focus();
  },
  copy: copyReply,
  packages: (app) => openPackages(app.ctx.packages, (text) => app.composer.say(text)),
  fake: (app, spec) => app.composer.say(t('commands.fake', { name: spec.name })),
};

/** `/redo`、`/edit` 能不能做：在回答时提示「回答进行中」，最新一轮不是你开的（或一轮都没有）提示「无法重做」。 */
function latestTurn(app) {
  if (app.composer.running) app.composer.say(res.text.refusals.turn_running);
  else if (!app.chat.hasLatest()) app.composer.say(res.text.refusals.not_redoable);
  else return true;
  return false;
}

/** 改界面语言：界面的字、收起那一行的写法变了内核重新载入页面；都没变的提示一句。 */
function setLanguage(app, value) {
  const done = app.ctx.language.set(value);
  if (!done.reload) app.composer.say(t('commands.language_changed', { name: done.language.name }));
}

/** 撤销、恢复、压缩要一个开了的会话；还没开的新会话提示一句，交回 `null`。 */
function opened(app) {
  if (!app.current) app.composer.say(t('commands.not_open'));
  return app.current;
}

/**
 * `/undo`：撤掉最近一轮，那一轮里你说的话放回输入框（框里有字的不覆盖），提示「已撤销」。放回的字照日志找整段，
 * 那一条推送还没到的用回应的 `said`（只有第一行）。等回应的时候换了会话的不放回。
 */
async function undo(app) {
  const session = opened(app);
  if (!session) return;
  const reply = await app.store.conn.request('session.revert', { session });
  const events = app.store.sessions.get(session)?.events ?? [];
  const said = revertedSaid(events, reply?.events?.[0]) ?? reply?.said ?? null;
  if (said && app.current === session) app.composer.putBack(said);
  // 撤掉的那几轮派出去、还在跑的任务一起停了（施工 7-8，回应的 `jobs`）：说一句
  const stopped = reply?.jobs?.length ?? 0;
  app.composer.say(stopped ? t('commands.undone_jobs', { count: stopped }) : t('commands.undone'));
}

/** `/restore`：恢复刚才撤销的；撤销时放回框里、还没动过的那句一并收回，免得再发一遍。 */
async function restore(app) {
  const session = opened(app);
  if (!session) return;
  await app.store.conn.request('session.unrevert', { session });
  if (app.current === session) app.composer.takeBack();
}

/**
 * 重做最新的一轮（蓝图「对话区」的「编辑」「她的一轮的按钮」）：`text` 是改过的话，`null` 是照原话重来。核心的
 * `session.redo` 撤掉和重发在同一批里做完；拒绝的写进提示，改过的字放回输入框，不往外抛。
 * @param {App} app
 * @param {string|null} text
 */
export async function redo(app, text) {
  const session = app.current;
  if (!session) return;
  // 画面不动：记住这一句的位置，重发的落回原处（蓝图「编辑」）
  app.chat.keepLatest();
  try {
    await app.store.conn.request('session.redo', text == null ? { session } : { session, text });
  } catch (err) {
    if (!(err instanceof Refusal)) console.error(err);
    app.chat.scroll.forget();
    if (text != null && app.current === session) app.composer.putBack(text);
    app.composer.say(refusalText(err));
  }
}

/** 复制她这一轮说的全部正文（原文，段与段之间空一行）。 */
export async function copyTurn(app, turn) {
  const s = app.current ? app.store.sessions.get(app.current) : null;
  const items = project(s?.events ?? [], s?.live ?? null, s?.marks).items;
  const text = items.filter((it) => it.type === 'reply' && it.turn === turn).map((it) => it.text.trim()).join('\n\n');
  if (!text) {
    app.composer.say(t('commands.nothing_to_copy'));
    return;
  }
  await copy(text, (words, good) => app.composer.say(words, good));
}

/** `/copy`：她的上一段回答（原文，写完了的）；还没有的提示一句。 */
async function copyReply(app) {
  const s = app.current ? app.store.sessions.get(app.current) : null;
  const items = project(s?.events ?? [], s?.live ?? null, s?.marks).items;
  const reply = items.filter((it) => it.type === 'reply' && !it.streaming).at(-1);
  if (!reply) {
    app.composer.say(t('commands.nothing_to_copy'));
    return;
  }
  await copy(reply.text, (text, good) => app.composer.say(text, good));
}
