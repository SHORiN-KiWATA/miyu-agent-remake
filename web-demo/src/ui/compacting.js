// @ts-check
//! 压缩的进度那一行（蓝图 `web.md`「压缩的进度」，方案 A：一行加细条）：正文末尾，一直是同一个节点（条在追、点在轮换，重画会
//! 从头来）。「正在压缩上下文」流光、点轮换、右边写了多少字；下面一根细条一顿一顿地追真实的字数（`model/compaction.js`），
//! 最多 95%；过了一会儿还没压好，字和条一起呼吸。会话状态不在压了（压好了、没压成、这一轮先结束了），当场拿掉，压好了的结果
//! 那一行照条目画。
//!
//! 自动压缩（`trigger` 是 `auto`）不画进度条：核心提前在后台压，到了线还没压完、停下来等的时候才在压（会话状态的 `doing`），只出一行
//! 绿点「正在压缩上下文…」，样子同结果那一行（2026-10-10 项目主人定，终端一样）。手动 `/compact` 照旧有进度条。

import { h } from './dom.js';
import { res, t } from '../util/res.js';
import { realCells, chase, percent } from '../model/compaction.js';

const reduced = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

export class CompactingRow {
  constructor() {
    this.session = /** @type {string|null} */ (null);
    this.el = h('div.compacting', { hidden: true, 'aria-live': 'polite' });
    /** @type {import('../model/view-state.js').Compacting|null} */
    this.state = null;
    this.shown = 0;
    /** @type {number[]} */
    this.timers = [];
  }

  /**
   * 照会话状态里这个会话的压缩画：`null` 是没在压（压好了、没压成、这一轮结束了），当场拿掉；换了会话的也从头来。
   * @param {import('../model/view-state.js').Compacting|null} state
   * @param {string|null} session
   */
  update(state, session) {
    if (session !== this.session) {
      this.stop();
      this.session = session;
    }
    if (!state) {
      if (this.state) this.stop();
      return;
    }
    if (!this.state || this.state.seen !== state.seen || this.state.since !== state.since || this.state.trigger !== state.trigger) this.start(state);
    this.state = state;
    this.draw();
  }

  start(state) {
    this.stop();
    if (state.trigger === 'auto') return this.waiting(state);
    const c = res.layout.compaction;
    this.state = state;
    this.shown = 0;
    this.count = h('span.compacting-count');
    this.dots = h('span.compacting-dots', '.');
    this.fill = h('span.compacting-fill');
    this.pct = h('span.compacting-pct');
    this.track = h('span.compacting-track', this.fill);
    this.bar = h('div.compacting-bar', this.track, this.pct);
    this.el.replaceChildren(
      h('div.compacting-line', h('span.compacting-label', t('notes.compacting')), this.dots, this.count),
      this.bar);
    this.el.classList.remove('is-breathing');
    this.el.hidden = false;
    let n = 1;
    this.timers.push(window.setInterval(() => { n = (n % 3) + 1; this.dots.textContent = '.'.repeat(n); }, c.dot_ms));
    this.timers.push(window.setTimeout(() => this.el.classList.add('is-breathing'), c.breathe_after_ms));
    this.step();
  }

  /** 自动压缩停下来等的那一行：绿点、「正在压缩上下文…」，不画条。 @param {import('../model/view-state.js').Compacting} state */
  waiting(state) {
    this.state = state;
    this.el.replaceChildren(h('div.note-line.tone-good.compacting-wait', h('span.note-mark', res.layout.note_marks.good), h('span.note-text', t('notes.compacting_wait'))));
    this.el.hidden = false;
  }

  /** 条一顿一顿地追：停一会儿，多走 1–3 格（减少动画的直接照真实的走）。 */
  step() {
    const c = res.layout.compaction;
    const [lo, hi] = c.pause_ms;
    this.timers.push(window.setTimeout(() => {
      if (!this.state) return;
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
    if (!s || s.trigger === 'auto') return;
    const c = res.layout.compaction;
    if (reduced()) this.shown = this.real();
    this.count.textContent = s.written ? t('notes.compacting_count', { count: s.written.toLocaleString('en-US') }) : '';
    this.bar.hidden = !s.expected;
    this.fill.style.width = `${(this.shown / c.cells) * 100}%`;
    if (s.expected) this.pct.textContent = `${percent(s.written, s.expected, c.cap)}%`;
  }

  /** 拿掉：停表，藏起来。 */
  stop() {
    for (const id of this.timers) { clearTimeout(id); clearInterval(id); }
    this.timers = [];
    this.state = null;
    this.el.hidden = true;
    this.el.classList.remove('is-breathing');
    this.el.replaceChildren();
  }
}
