// @ts-check
//! 待办那一块（蓝图 `web.md`「待办」，照 `tui.md`「后台命令、子代理和侧边栏」第 4、7 条）：有待办时常驻在输入框上面，
//! 占着地方（照 TUI 窄屏的样子，正文往上让），在运行状态行上面；命令列表开着时被它盖住。
//!
//! 真的待办照核心推的（`setReal`，核心 D-3）：全做完了停 `hold_ms` 让人看到最后一项打勾，再收掉；读回来就已经全做完的不出，
//! 收掉以后清单再变了再出来。跟着会话走：换了会话藏起来，换回来接着看。
//! 收成几行照 `model.js`，点这一块展开、再点收起。出来、收掉照「动效」：高度从 0 长出来、收回去（`unfold`），收的时候字留着。
//! 数和字都是这个包的（`config`、`t`），停用时 `destroy` 停掉计时。

import { h, replace } from '../../src/lib/dom.js';
import { unfold } from '../../src/lib/motion.js';
import { progress, allDone, fold, fromCore } from './model.js';

/** @typedef {{todos: import('./model.js').Todo[], full: boolean, timer: number, hidden: boolean, sig: string}} Real 一个会话真的待办（`hidden`：全做完了、停够了收掉的） */

export class TodoDock {
  /**
   * @param {{rows: number, marks: Record<string, string>, hold_ms: number}} config 这个包的设置项
   * @param {(path: string, fields?: any) => string} t 这个包的字
   */
  constructor(config, t) {
    this.config = config;
    this.t = t;
    /** 会话编号 → 核心推来的真的待办 */
    this.real = /** @type {Map<string|null, Real>} */ (new Map());
    /** 正在看的会话。 */
    this.key = /** @type {string|null} */ (null);
    /** 头一行和一项一行：收的时候留着，收的动画里还看得到 */
    this.body = h('div.todo-body');
    this.el = h('div.dock-todo.unfold', { role: 'button', title: t('toggle'), onclick: () => this.toggle() }, h('div.unfold-inner', this.body));
    unfold(this.el, false);
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

  /** 正在看的会话这一块画谁：它的清单（全做完、停够了收掉的不画）。 */
  current() {
    const real = this.real.get(this.key);
    return real && !real.hidden ? real : null;
  }

  /** 看另一个会话：它的清单露出来，别的藏起来。 */
  show(key) {
    if (key === this.key) return;
    this.key = key;
    this.draw();
  }

  toggle() {
    const shown = this.current();
    if (!shown) return;
    shown.full = !shown.full;
    this.draw();
  }

  draw() {
    const list = this.current();
    unfold(this.el, !!list);
    if (!list) return;
    const { done, total } = progress(list.todos);
    const t = this.t;
    const marks = this.config.marks;
    const row = (cls, mark, text) => h(`div.todo-row.${cls}`, mark == null ? null : h('span.todo-mark', mark), h('span.todo-text', text));
    const rows = fold(list.todos, this.config.rows, list.full).map((r) => {
      if (r.kind === 'folded') return row('is-folded', marks.done, t('folded', { count: r.count }));
      if (r.kind === 'more') return row('is-more', null, t('more', { count: r.count }));
      return row(`is-${r.todo.state}`, marks[r.todo.state], r.todo.text);
    });
    this.el.classList.toggle('is-full', list.full);
    this.el.setAttribute('aria-expanded', String(list.full));
    replace(this.body, h('div.todo-head', t('title', { done, total })), rows);
  }

  /** 停用了：停掉每个会话的计时。 */
  destroy() {
    for (const real of this.real.values()) clearTimeout(real.timer);
    this.real.clear();
  }
}
