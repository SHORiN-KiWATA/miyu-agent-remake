// @ts-check
//! 后台任务的按钮和浮层（软件包 `jobs`，蓝图 `web.md`「后台任务」第 2–8 条）：按钮挂在框下面那一行的中间，有在跑的才露（连嵌套的
//! 一起数）；浮层在输入框上面、和命令列表同一个位置：头一行「后台任务」和几个在跑、几个结束了；在跑的、停在半路的排在上面，能停，
//! 子代理下面在跑的缩进一层、文件树的线接上（怎么分段在 `sections.js`）；「已结束 N 个」一行，点了展开、收起（和时间线点开一步同一个
//! 动效，展开时列表跟着往下滚，往下滚时这一行钉在顶上）。一行一个、一行字：左边照种类的图标，标题后面接编号，右边状态的记号接一段
//! 字。子代理那一行点了进它的会话；后台命令那一行点了在下面展开预览（`preview.js`）。在跑的不止一个时头一行有「全部停掉」。
//!
//! 开着时每 `tick_ms` 只改用时那几个字、读一次开着的预览；任务的样子变了才整个重画（不然每秒重画一次，展开收起的动画、悬停
//! 都被打断）。

import { h, icon, replace } from '../../src/lib/dom.js';
import { show, hide } from '../../src/lib/motion.js';
import { clock, seconds } from '../../src/lib/format.js';
import { sections } from './sections.js';
import { Preview } from './preview.js';

/**
 * @typedef {import('./sections.js').Node} Node 一个任务：谁派的（`owner`，停它时照它），它是子代理的，下面挂着它的会话派的
 */

/** 一棵树里在跑的有几个（停在半路的不算） */
export const countRunning = (/** @type {Node[]} */ tree) => tree.reduce((n, x) => n + (x.state === 'running' ? 1 : 0) + countRunning(x.kids), 0);
/** 一棵树里在跑的全部（全部停掉用）：停一个子代理连它派的一起停，它下面的不再单停 */
const liveOf = (/** @type {Node[]} */ tree) => tree.flatMap((x) => (x.state === 'running' || x.state === 'paused' ? [x] : liveOf(x.kids)));
/** 一个任务的键：谁派的加编号 */
const keyOf = (/** @type {Node} */ x) => `${x.owner}:${x.job}`;

export class JobsPanel {
  /**
   * @param {(path: string, fields?: any) => string} t 这个包的字
   * @param {{tick_ms: number, fold_ms: number}} config
   * @param {{stop: (owner: string, job: string) => Promise<void>, enter: (session: string) => void,
   *   read: (owner: string, job: string) => Promise<any>}} on 停掉一个、进一个子代理的会话、读一条后台命令的输出
   */
  constructor(t, config, on) {
    this.t = t;
    this.config = config;
    this.on = on;
    /** 展开了「已结束 N 个」（这个标签页里记着） */
    this.doneOpen = false;
    /** @type {Node[]} */
    this.tasks = [];
    /** 点过停止、还没等到回报的 */
    this.stopping = new Set();
    /** 开着预览的后台命令：键 → 预览 */
    /** @type {Map<string, Preview>} */
    this.previews = new Map();
    /** 上一次整个画的是什么样：没变的只改用时 */
    this.drawn = '';
    this.timer = 0;
    this.label = h('span.jobs-pill-text');
    this.button = h('button.jobs-pill', { type: 'button', hidden: true, title: t('open'), onclick: () => this.toggle() }, h('span.jobs-dot'), this.label);
    this.headCount = h('span.jobs-head-count');
    this.stopAll = h('button.jobs-stop-all', { type: 'button', hidden: true, onclick: () => this.stopEverything() }, t('stop_all'));
    this.head = h('div.jobs-head', h('strong.jobs-head-title', t('title')), this.headCount, this.stopAll);
    this.list = h('div.jobs-list');
    this.el = h('div.jobs-panel.dock-float', { hidden: true, role: 'dialog', 'aria-label': t('open'), style: `--jobs-fold: ${config.fold_ms}ms` }, this.head, this.list);
    this.onKey = (/** @type {KeyboardEvent} */ e) => {
      if (e.key !== 'Escape') return;
      e.stopPropagation();
      this.close();
    };
    this.onDown = (/** @type {PointerEvent} */ e) => {
      const at = /** @type {any} */ (e.target);
      if (!this.el.contains(at) && !this.button.contains(at)) this.close();
    };
  }

  /** @param {Node[]} tasks 这个会话的任务，连嵌套的 */
  update(tasks) {
    this.tasks = tasks;
    // 点过停止的：还在跑的（哪一层的都算）留着灰，结束了的放掉
    const all = (/** @type {Node[]} */ xs) => xs.flatMap((x) => [x, ...all(x.kids)]);
    const live = new Set(all(tasks).filter((x) => x.state === 'running' || x.state === 'paused').map(keyOf));
    for (const key of [...this.stopping]) if (!live.has(key)) this.stopping.delete(key);
    const n = countRunning(tasks);
    this.button.hidden = n === 0;
    this.label.textContent = this.t('pill', { count: n });
    if (this.isOpen()) this.draw();
  }

  isOpen() {
    return !this.el.hidden && !this.el.classList.contains('is-leaving');
  }

  toggle() {
    if (this.isOpen()) this.close();
    else this.open();
  }

  open() {
    if (this.isOpen()) return;
    this.drawn = '';
    this.draw();
    show(this.el);
    clearInterval(this.timer);
    this.timer = window.setInterval(() => this.tick(), this.config.tick_ms);
    document.addEventListener('keydown', this.onKey, true);
    document.addEventListener('pointerdown', this.onDown, true);
  }

  close() {
    clearInterval(this.timer);
    document.removeEventListener('keydown', this.onKey, true);
    document.removeEventListener('pointerdown', this.onDown, true);
    if (this.isOpen()) hide(this.el);
  }

  /** 停掉一个：记着在停，等回报。 */
  halt(task) {
    this.stopping.add(keyOf(task));
    this.draw();
    return this.on.stop(task.owner, task.job).finally(() => this.draw());
  }

  /** 全部停掉：照先后一个个停（停一个子代理连它派的一起停）。 */
  async stopEverything() {
    for (const task of liveOf(this.tasks)) if (!this.stopping.has(keyOf(task))) await this.halt(task);
  }

  /** 每 `tick_ms`：只改在跑的用时，读一次开着的、还在跑的预览。 */
  tick() {
    const now = Date.now();
    for (const el of this.list.querySelectorAll('.jobs-row.is-running')) {
      const text = el.querySelector('.jobs-state-text');
      const since = Number(/** @type {HTMLElement} */ (el).dataset.since);
      // 派它的那条还在更早、没读的页里的（核心 9-6 上的种子）不知道几时开始，不走表
      if (text && Number.isFinite(since)) text.textContent = this.t('states.running', { elapsed: clock((now - since) / 1000) });
    }
    for (const p of this.previews.values()) p.refresh();
  }

  /**
   * 头一行「后台任务」和几个在跑、几个结束了；下面在跑的、停在半路的一个一行，「已结束 N 个」一行（点了展开）；什么都没有的写
   * 一句。任务的样子没变的不重画（蓝图「后台任务」第 3 条）。
   */
  draw() {
    const t = this.t;
    const s = sections(this.tasks);
    const sig = JSON.stringify([s.live, s.done].map((part) => part.map((l) => [keyOf(l.task), l.task.state, l.task.title, l.depth, l.spawned, l.last, l.guides, l.kids, l.orphan])).concat([[...this.stopping]]));
    const running = s.live.filter((l) => l.task.state === 'running').length;
    const paused = s.live.length - running;
    const counts = [running && t('counts.running', { count: running }), paused && t('counts.paused', { count: paused }), s.counts.done && t('counts.done', { count: s.counts.done })];
    this.headCount.textContent = counts.filter(Boolean).join(' · ');
    this.stopAll.hidden = liveOf(this.tasks).length < 2;
    if (sig === this.drawn) return;
    this.drawn = sig;
    const now = Date.now();
    // 没了的任务的预览放掉
    const alive = new Set([...s.live, ...s.done].map((l) => keyOf(l.task)));
    for (const key of [...this.previews.keys()]) if (!alive.has(key)) this.previews.delete(key);
    const doneFold = h(`div.jobs-fold.jobs-done-fold${this.doneOpen ? '.is-open' : ''}`, h('div.jobs-fold-inner', s.done.flatMap((l) => this.row(l, now))));
    const doneButton = h(`button.jobs-done${this.doneOpen ? '.is-open' : ''}`, {
      type: 'button', 'aria-expanded': String(this.doneOpen),
      onclick: () => this.foldDone(doneButton, doneFold),
    }, h('span', t('sections.done', { count: s.done.length })), icon('chevron-right'));
    replace(this.list, [
      s.live.flatMap((l) => this.row(l, now)),
      s.done.length ? [doneButton, doneFold] : [],
      s.live.length || s.done.length ? [] : [h('div.jobs-empty', t('empty'))],
    ].flat());
  }

  /** 展开、收起「已结束 N 个」：只换样子（动画照 CSS 走）；展开时列表跟着往下滚，让展开的露出来。 */
  foldDone(button, fold) {
    this.doneOpen = !this.doneOpen;
    button.classList.toggle('is-open', this.doneOpen);
    button.setAttribute('aria-expanded', String(this.doneOpen));
    fold.classList.toggle('is-open', this.doneOpen);
    if (this.doneOpen) this.follow(fold);
  }

  /** 展开的那一块往下长的这一会儿，列表一帧一帧地滚，让它的底露出来。 */
  follow(node) {
    const until = performance.now() + this.config.fold_ms;
    const list = this.list;
    const step = () => {
      const over = node.getBoundingClientRect().bottom - list.getBoundingClientRect().bottom;
      if (over > 0) list.scrollTop += over;
      if (performance.now() < until) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }

  /**
   * 一行（后台命令的连着下面那块预览）：左边照种类的图标，中间一行字（标题、编号、派了几个），右边状态的记号接一段字、停止
   * 按钮；嵌套的前面文件树的线。子代理那一行点了进它的会话，后台命令那一行点了展开、收起预览。
   * @param {import('./sections.js').Line} line
   * @param {number} now
   * @returns {HTMLElement[]}
   */
  row(line, now) {
    const t = this.t;
    const task = line.task;
    const agent = task.what === 'agent';
    const key = keyOf(task);
    const stoppable = task.state === 'running' || task.state === 'paused';
    const stop = stoppable
      ? h('button.jobs-stop', {
        type: 'button', title: t('stop'), 'aria-label': t('stop'), disabled: this.stopping.has(key),
        onclick: () => { this.halt(task); },
      }, h('span.stop-mark'))
      : null;
    const preview = agent ? null : h(`div.jobs-fold.jobs-preview-fold${this.previews.has(key) ? '.is-open' : ''}`, { style: `--depth: ${line.depth}` },
      h('div.jobs-fold-inner', this.previews.get(key)?.el ?? null));
    const click = (/** @type {MouseEvent} */ e) => {
      if (/** @type {Element} */ (e.target).closest('.jobs-stop')) return;
      if (agent && task.session) {
        this.close();
        this.on.enter(task.session);
      } else if (preview) this.togglePreview(task, row, preview);
    };

    const cls = `.is-${task.state}${agent ? '.is-agent' : '.is-command'}${line.kids ? '.has-kids' : ''}${this.previews.has(key) ? '.is-previewing' : ''}`;
    const row = h(`div.jobs-row${cls}`, {
      role: 'button', tabindex: '0', title: agent ? t('enter') : t('preview.open'), style: `--depth: ${line.depth}`,
      dataset: { since: task.since == null ? '' : String(task.since) }, onclick: click,
    },
    treeLines(line),
    h('span.jobs-mark', icon(agent ? 'bot' : 'square-terminal')),
    h('span.jobs-text', h('span.jobs-title', { title: task.title }, task.title), h('span.jobs-id', task.job),
      line.spawned ? h('span.jobs-spawned', t('spawned', { count: line.spawned })) : null),
    h('span.jobs-state', h('span.jobs-state-mark', stateMark(task.state)), h('span.jobs-state-text', this.stateText(task, now))),
    stop);
    return preview ? [row, preview] : [row];
  }

  /** 展开、收起一条后台命令的预览：展开时读一次（跑着的之后每秒再读），列表跟着往下滚。 */
  togglePreview(task, row, fold) {
    const key = keyOf(task);
    const open = !this.previews.has(key);
    if (open) {
      const p = new Preview(task, this.t, this.on.read);
      this.previews.set(key, p);
      replace(/** @type {HTMLElement} */ (fold.firstChild), p.el);
      p.refresh();
      this.follow(fold);
    } else this.previews.delete(key);
    row.classList.toggle('is-previewing', open);
    fold.classList.toggle('is-open', open);
  }

  /** 状态的字：在跑的走表，停在半路的写等你说话，结束的照回报（和正文里回报那一行一个说法）。 */
  stateText(task, now) {
    const t = this.t;
    if (task.state === 'running') return task.since == null ? t('states.running_plain') : t('states.running', { elapsed: clock((now - task.since) / 1000) });
    if (task.state === 'done') {
      const ms = task.duration ?? (task.ended != null && task.since != null ? task.ended - task.since : null);
      return ms != null ? t('states.done', { elapsed: seconds(ms) }) : t('states.done_plain');
    }
    if (task.state === 'failed') return task.signal != null ? t('states.signal', { signal: task.signal }) : t('states.failed', { code: task.code });
    const known = t(`states.${task.state}`);
    return known === `states.${task.state}` ? task.state : known;
  }

  /** 停用了：关上、停掉计时、拿掉节点。 */
  destroy() {
    this.close();
    this.button.remove();
    this.el.remove();
  }
}

/** 状态的记号：在跑的转圈，停在半路的暂停，完成的勾，失败的叉，停了、撤销、中断的一个小方块。 */
function stateMark(state) {
  if (state === 'running') return icon('loader-circle');
  if (state === 'paused') return icon('circle-pause');
  if (state === 'done') return icon('check');
  if (state === 'failed') return icon('x');
  return h('i.jobs-square');
}

/** 文件树那样的线（和左栏一样）：上面几层还没完的竖线穿过去，自己这一层一段竖线接到上一行、一小段横线接到记号。父子代理结束了的不画。 */
function treeLines(line) {
  if (!line.depth || line.orphan) return null;
  const guides = (line.guides ?? []).map((on, i) => (on ? h('i.jobs-guide', { style: `--level: ${i + 1}` }) : null));
  return h('span.jobs-lines', { 'aria-hidden': 'true' }, ...guides, h(`i.jobs-elbow${line.last ? '.is-last' : ''}`, { style: `--level: ${line.depth}` }));
}
