// @ts-check
//! 待办那一块（蓝图 `web.md`「待办」，照 `tui.md`「后台命令、子代理和侧边栏」第 4、7 条）：有待办时常驻在输入框上面，
//! 占着地方（照 TUI 窄屏的样子，正文往上让），在运行状态行上面；命令列表开着时被它盖住。
//!
//! 核心还没有待办：`/demo-todo` 推一份演示的（设置项 `demo_items`），每 `every_ms` 推进一项；全做完了停 `hold_ms` 让人看到
//! 最后一项打勾，再收掉。演示跟着开它的那个会话走：换了会话藏起来，换回来接着看（计时照走）；数据只在这一页里，刷新就没了。
//! 收成几行照 `model.js`，点这一块展开、再点收起。出来、收掉照「动效」：高度从 0 长出来、收回去（`unfold`），收的时候字留着。
//! 数和字都是这个包的（`config`、`t`），停用时 `destroy` 停掉计时。

import { h, replace } from '../../src/lib/dom.js';
import { unfold } from '../../src/lib/motion.js';
import { startTodos, advance, progress, allDone, fold } from './model.js';

/** @typedef {{todos: import('./model.js').Todo[], full: boolean, timer: number}} Demo 一个会话的演示 */

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
    const demo = this.demos.get(this.key);
    if (!demo) return;
    demo.full = !demo.full;
    this.touched(demo);
  }

  /** 一份演示变了：是正在看的才重画，别的会话的照走、不画。 */
  touched(demo) {
    if (this.demos.get(this.key) === demo) this.draw();
  }

  draw() {
    const demo = this.demos.get(this.key);
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
    this.demos.clear();
  }
}
