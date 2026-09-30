// @ts-check
//! 压缩的进度那一行（蓝图 `web.md`「压缩的进度」，方案 A：一行加细条）：正文末尾，一直是同一个节点（条在追、点在轮换，重画会
//! 从头来）。「正在压缩上下文」流光、点轮换、右边写了多少字；下面一根细条一顿一顿地追真实的字数（`model/compaction.js`），
//! 最多 95%；过了一会儿还没压好，字和条一起呼吸。压好了：条走满、停一下、淡出，交给 `onFinished`（仓库收掉这一行，落了盘的
//! 结果那一行露出来）。没压成、这一轮先结束了的：仓库那边已经收掉了，这里当场拿掉。

import { h } from './dom.js';
import { res, t } from '../util/res.js';
import { leave } from '../lib/motion.js';
import { realCells, chase, percent } from '../model/compaction.js';

const reduced = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

export class CompactingRow {
  /** @param {(session: string|null) => void} onFinished 压好了、走满了、淡出了：哪个会话的 */
  constructor(onFinished) {
    this.session = /** @type {string|null} */ (null);
    this.onFinished = onFinished;
    this.el = h('div.compacting', { hidden: true, 'aria-live': 'polite' });
    /** @type {import('../core/store.js').Compacting|null} */
    this.state = null;
    this.shown = 0;
    this.finishing = false;
    /** @type {number[]} */
    this.timers = [];
  }

  /**
   * 照仓库里这个会话的压缩画：`null` 是没在压（没压成、这一轮结束了），当场拿掉；换了会话的也从头来。
   * @param {import('../core/store.js').Compacting|null} state
   * @param {string|null} session
   */
  update(state, session) {
    if (session !== this.session) {
      this.stop();
      this.session = session;
    }
    if (!state) {
      if (this.state && !this.finishing) this.stop();
      return;
    }
    if (!this.state || this.state.seen !== state.seen || this.state.since !== state.since) this.start(state);
    this.state = state;
    this.draw();
    if (state.done && !this.finishing) this.finish();
  }

  start(state) {
    this.stop();
    const c = res.layout.compaction;
    this.state = state;
    this.shown = 0;
    this.finishing = false;
    this.count = h('span.compacting-count');
    this.dots = h('span.compacting-dots', '.');
    this.fill = h('span.compacting-fill');
    this.pct = h('span.compacting-pct');
    this.track = h('span.compacting-track', this.fill);
    this.bar = h('div.compacting-bar', this.track, this.pct);
    this.el.replaceChildren(
      h('div.compacting-line', h('span.compacting-label', t('notes.compacting')), this.dots, this.count),
      this.bar);
    this.el.classList.remove('is-leaving', 'is-breathing');
    this.el.hidden = false;
    let n = 1;
    this.timers.push(window.setInterval(() => { n = (n % 3) + 1; this.dots.textContent = '.'.repeat(n); }, c.dot_ms));
    this.timers.push(window.setTimeout(() => { if (!this.finishing) this.el.classList.add('is-breathing'); }, c.breathe_after_ms));
    this.step();
  }

  /** 条一顿一顿地追：停一会儿，多走 1–3 格（减少动画的直接照真实的走）。 */
  step() {
    const c = res.layout.compaction;
    const [lo, hi] = c.pause_ms;
    this.timers.push(window.setTimeout(() => {
      if (!this.state || this.finishing) return;
      this.shown = chase(this.shown, this.real());
      this.draw();
      this.step();
    }, lo + Math.random() * (hi - lo)));
  }

  real() {
    const c = res.layout.compaction;
    return this.state ? realCells(this.state.written, this.state.expected, c.cells, c.cap) : 0;
  }

  draw() {
    const s = this.state;
    if (!s || this.finishing) return;
    const c = res.layout.compaction;
    if (reduced()) this.shown = this.real();
    this.count.textContent = s.written ? t('notes.compacting_count', { count: s.written.toLocaleString('en-US') }) : '';
    this.bar.hidden = !s.expected;
    this.fill.style.width = `${(this.shown / c.cells) * 100}%`;
    if (s.expected) this.pct.textContent = `${percent(s.written, s.expected, c.cap)}%`;
  }

  /** 压好了：条在 `finish_ms` 里一格格走满、百分比跟着到 100%，停 `hold_ms`，淡出。 */
  finish() {
    this.finishing = true;
    this.el.classList.remove('is-breathing');
    const c = res.layout.compaction;
    const from = this.shown / c.cells;
    const pctFrom = this.state?.expected ? percent(this.state.written, this.state.expected, c.cap) : 0;
    const steps = reduced() ? 1 : 8;
    for (let i = 1; i <= steps; i++) {
      this.timers.push(window.setTimeout(() => {
        const r = from + (1 - from) * (i / steps);
        this.fill.style.width = `${r * 100}%`;
        this.pct.textContent = `${Math.round(pctFrom + (100 - pctFrom) * (i / steps))}%`;
        if (i === steps) this.timers.push(window.setTimeout(() => leave(this.el, () => this.done()), c.hold_ms));
      }, (c.finish_ms / steps) * i));
    }
  }

  done() {
    const session = this.session;
    this.stop();
    this.onFinished(session);
  }

  /** 拿掉：停表，藏起来。 */
  stop() {
    for (const id of this.timers) { clearTimeout(id); clearInterval(id); }
    this.timers = [];
    this.state = null;
    this.finishing = false;
    this.el.hidden = true;
    this.el.classList.remove('is-leaving', 'is-breathing');
    this.el.replaceChildren();
  }
}
