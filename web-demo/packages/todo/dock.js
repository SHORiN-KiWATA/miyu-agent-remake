// @ts-check
//! 待办那一块（蓝图 `web.md`「待办」，照 `tui.md`「后台命令、子代理和侧边栏」第 4、7 条）：有待办时常驻在输入框上面，
//! 占着地方（照 TUI 窄屏的样子，正文往上让），在运行状态行上面；命令列表开着时被它盖住。
//!
//! 真的待办照核心推的（`setReal`，核心 D-3）：全做完了停 `hold_ms` 让人看到最后一项打勾，再收掉；读回来就已经全做完的不出，
//! 收掉以后清单再变了再出来。`/demo-todo` 推一份演示的（设置项 `demo_items`），每 `every_ms` 推进一项，只在这一页里；这个会话
//! 有真的待办时不出。都跟着会话走：换了会话藏起来，换回来接着看。
//! 收成几行照 `model.js`，点这一块展开、再点收起。出来、收掉照「动效」：高度从 0 长出来、收回去（`unfold`），收的时候字留着。
//! 数和字都是这个包的（`config`、`t`），停用时 `destroy` 停掉计时。

import { h, replace } from '../../src/lib/dom.js';
import { unfold } from '../../src/lib/motion.js';
import { startTodos, advance, progress, allDone, fold, fromCore } from './model.js';

/** @typedef {{todos: import('./model.js').Todo[], full: boolean, timer: number}} Demo 一个会话的演示 */
/** @typedef {{todos: import('./model.js').Todo[], full: boolean, timer: number, hidden: boolean, sig: string}} Real 一个会话真的待办（`hidden`：全做完了、停够了收掉的） */

export class TodoDock {
  /**
   * @param {{rows: number, marks: Record<string, string>, hold_ms: number, demo_items: string[], every_ms: number}} config 这个包的设置项
   * @param {(path: string, fields?: any) => string} t 这个包的字
   */
  constructor(config, t) {
    this.config = config;
    this.t = t;
    /** 会话编号（还没开的新会话是 `null`）→ 它的演示。 */
    this.demos = /** @type {Map<string|null, Demo>} */ (new Map());
    /** 会话编号 → 核心推来的真的待办 */
    this.real = /** @type {Map<string|null, Real>} */ (new Map());
    /** 正在看的会话。 */
    this.key = /** @type {string|null} */ (null);
    /** 头一行和一项一行：收的时候留着，收的动画里还看得到 */
    this.body = h('div.todo-body');
    this.el = h('div.dock-todo.unfold', { role: 'button', title: t('toggle'), onclick: () => this.toggle() }, h('div.unfold-inner', this.body));
    unfold(this.el, false);
  }

  /** 在 `key` 这个会话里推一份演示的待办；已经有一份的从头来。 */
  start(key) {
    this.drop(key);
    const { demo_items: items, every_ms: every } = this.config;
    /** @type {Demo} */
    const demo = { todos: startTodos(items), full: false, timer: 0 };
    const step = () => {
      demo.todos = advance(demo.todos);
      demo.timer = allDone(demo.todos)
        ? setTimeout(() => this.forget(demo), this.config.hold_ms)
        : setTimeout(step, every);
      this.touched(demo);
    };
    demo.timer = setTimeout(step, every);
    this.demos.set(key, demo);
    this.touched(demo);
  }

  /**
   * 核心推来的这个会话的清单（对话区每画一次都给，一样的不动）：空的去掉；全做完了停 `hold_ms` 再收掉；这一页里头一次见到就已经全做完的
   * （刷新、换回这个会话读回来的）直接不出。因为全做完了清空的（`done` 是清空前那一份，核心 D-3 补）照它画成全打勾，停一下再收。
   * @param {string|null} key @param {{content: string, status: string}[]} list @param {{content: string, status: string}[]|null} [done]
   */
  setReal(key, list, done = null) {
    const sig = JSON.stringify([list ?? [], done ?? null]);
    const had = this.real.get(key);
    if (had?.sig === sig) return;
    if (had) clearTimeout(had.timer);
    // 全做完了清空的：照清空前那一份当成还在、全打勾，下面照「全做完了」停一下再收（这一页里看着的时候才停，读回来的不出）
    const shown = !(list ?? []).length && done?.length && had && !had.hidden ? done.map((x) => ({ ...x, status: 'completed' })) : list;
    const todos = fromCore(shown);
    if (!todos.length) {
      this.real.delete(key);
    } else {
      /** @type {Real} */
      const entry = { todos, full: had?.full ?? false, timer: 0, hidden: false, sig };
      if (allDone(todos)) {
        if (!had) entry.hidden = true;
        else entry.timer = setTimeout(() => { entry.hidden = true; if (this.key === key) this.draw(); }, this.config.hold_ms);
      }
      this.real.set(key, entry);
    }
    if (this.key === key) this.draw();
  }

  /** 正在看的会话这一块画谁：真的（露着的）优先，没有的才是演示。 */
  current() {
    const real = this.real.get(this.key);
    if (real) return real.hidden ? null : real;
    return this.demos.get(this.key) ?? null;
  }

  /** 看另一个会话：它的演示露出来，别的藏起来。 */
  show(key) {
    if (key === this.key) return;
    this.key = key;
    this.draw();
  }

  /** 新会话第一句话发出去、会话开了：演示跟过去（还是同一段对话）。 */
  rename(from, to) {
    const demo = this.demos.get(from);
    if (!demo) return;
    this.demos.delete(from);
    this.demos.set(to, demo);
    if (this.key === from) this.key = to;
  }

  /** 去掉 `key` 这个会话的演示（开了一个新会话，它不带着别人的待办）。 */
  drop(key) {
    const demo = this.demos.get(key);
    if (demo) this.forget(demo);
  }

  forget(demo) {
    clearTimeout(demo.timer);
    const shown = this.demos.get(this.key) === demo;
    for (const [k, d] of this.demos) if (d === demo) this.demos.delete(k);
    if (shown) this.draw();
  }

  toggle() {
    const shown = this.current();
    if (!shown) return;
    shown.full = !shown.full;
    this.draw();
  }

  /** 一份演示变了：是正在看的才重画，别的会话的照走、不画。 */
  touched(demo) {
    if (this.demos.get(this.key) === demo) this.draw();
  }

  draw() {
    const demo = this.current();
    unfold(this.el, !!demo);
    if (!demo) return;
    const { done, total } = progress(demo.todos);
    const t = this.t;
    const marks = this.config.marks;
    const row = (cls, mark, text) => h(`div.todo-row.${cls}`, mark == null ? null : h('span.todo-mark', mark), h('span.todo-text', text));
    const rows = fold(demo.todos, this.config.rows, demo.full).map((r) => {
      if (r.kind === 'folded') return row('is-folded', marks.done, t('folded', { count: r.count }));
      if (r.kind === 'more') return row('is-more', null, t('more', { count: r.count }));
      return row(`is-${r.todo.state}`, marks[r.todo.state], r.todo.text);
    });
    this.el.classList.toggle('is-full', demo.full);
    this.el.setAttribute('aria-expanded', String(demo.full));
    replace(this.body, h('div.todo-head', t('title', { done, total })), rows);
  }

  /** 停用了：停掉每个会话的计时。 */
  destroy() {
    for (const demo of this.demos.values()) clearTimeout(demo.timer);
    for (const real of this.real.values()) clearTimeout(real.timer);
    this.demos.clear();
    this.real.clear();
  }
}
