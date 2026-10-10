// @ts-check
//! 运行状态行（蓝图 `web.md`「运行状态行」，照 `tui.md`「运行状态行和排队的消息」第 1–4 条）：输入框左上角浮着的一行，
//! 从 `turn.started` 到 `turn.ended` 一直在；出来、收起照「动效」：高度从 0 长出来、收回去（`unfold`），收的时候字留着。
//! 挑哪个词、点几个是 `model/pulse.js` 算的，这里只画。
//!
//! 一行：词和贴着的点一起被流光扫（CSS 的渐变加 `background-clip: text`，一趟 `layout.json` 的 `shimmer.sweep_seconds`；
//! 换了词换一个节点，亮光从头扫）；点 `.`、`..`、`...` 跟着流光轮，没写满的格子占着地方不画，扫的长度不跳；后面垫一个看不见的
//! 最宽的词，用时不左右跳；再是用时和重试。每 `pulse_tick_ms` 看一眼，字变了才动 DOM。

import { h } from '../../src/lib/dom.js';
import { unfold } from '../../src/lib/motion.js';
import { clock } from '../../src/lib/format.js';
import { Pulse, widest, dotCount, retryLine } from './model.js';

/**
 * @typedef {{id: string, start: number, beat: unknown, retry: {attempt: number, limit: number, message: string, failover?: boolean, due?: number}|null, queued: string[], waiting?: string|null}} PulseState
 *   在跑的那一轮（会话加回合、开始的时刻）、这一轮出过的事的记号（`entriesBeat`）、在等的重试、排着的话（蓝图「排队的消息」）；`waiting` 是
 *   在等你确认、回答时写的那几个字（「等你回答」：静止的，不扫流光、不轮点、不写用时）
 */

export class PulseLine {
  /**
   * @param {{sweep_seconds: number, dim: number, lift: number, dot_mark: string, dot_count: number, tick_ms: number, words: any}} config 这个包的设置项
   * @param {(path: string, fields?: any) => string} t 这个包的字
   */
  constructor(config, t) {
    this.config = config;
    this.t = t;
    this.pulse = new Pulse();
    /** @type {PulseState|null} */
    this.state = null;
    this.timer = 0;
    /** 画着的那个词是什么时候换上的：变了换一个节点，流光从头扫。 */
    this.drawn = /** @type {number|null} */ (null);
    const { dot_mark: mark, dot_count: count } = config;
    this.dots = h('span.pulse-dots');
    this.ghost = h('span.pulse-ghost');
    this.shine = h('span.pulse-shine');
    this.clock = h('span.pulse-clock');
    this.retry = h('span.pulse-retry');
    /** 排着的话：一条一行，列在运行状态行下面 */
    this.queue = h('div.pulse-queue');
    this.drawnQueue = '';
    this.el = h('div.dock-pulse.unfold', {
      style: `--pulse-sweep: ${config.sweep_seconds}s; --pulse-dim: ${config.dim * 100}%; --pulse-lift: ${config.lift * 100}%`,
    },
    h('div.unfold-inner', h('div.pulse-body',
      h('div.pulse-row',
        h('span.pulse-lead', h('span.pulse-sizer', { 'aria-hidden': 'true' }, widest(config.words) + mark.repeat(count)), this.shine),
        this.clock, this.retry),
      this.queue)));
    unfold(this.el, false);
  }

  /** 这一刻的状态；没在回答是 `null`：收起、停表。在回答时每隔一会儿自己重画（点、用时、到点换词）。 */
  set(state) {
    this.state = state;
    if (state && !this.timer) this.timer = setInterval(() => this.draw(), this.config.tick_ms);
    if (!state && this.timer) {
      clearInterval(this.timer);
      this.timer = 0;
    }
    this.draw();
  }

  draw() {
    const s = this.state;
    unfold(this.el, !!s);
    if (!s) return;
    const now = Date.now();
    // 在等你确认、回答：一行静止的字（蓝图「运行状态行」），了结了照常走、流光从头扫
    if (s.waiting) {
      if (this.drawn !== -1) {
        this.drawn = -1;
        const wait = h('span.pulse-shine.is-waiting', s.waiting);
        this.shine.replaceWith(wait);
        this.shine = wait;
      }
      setText(this.clock, '');
      setText(this.retry, '');
      return;
    }
    if (this.drawn === -1) this.drawn = null;
    const word = this.pulse.word({ id: s.id, start: s.start }, s.beat, now, this.config.words);
    const { dot_mark: mark, dot_count: count, sweep_seconds: sweep } = this.config;
    if (this.pulse.shown !== this.drawn) {
      this.drawn = this.pulse.shown;
      // 新的节点：CSS 的流光动画从头来
      const shine = h('span.pulse-shine', word, this.dots, this.ghost);
      this.shine.replaceWith(shine);
      this.shine = shine;
    }
    const n = dotCount((now - (this.pulse.shown ?? now)) / 1000, sweep, count);
    setText(this.dots, mark.repeat(n));
    setText(this.ghost, mark.repeat(count - n));
    setText(this.clock, clock(Math.max(0, now - s.start) / 1000));
    // 重试：还在等的倒数（一秒走一格），换了端点当场再来的（`failover`，核心施工 8-9）写「换端点重试」；只有一行，放不下的截掉
    const retry = retryLine(s.retry, now);
    setText(this.retry, retry ? this.t(retry.key, retry.fields) : '');
    // 排着的话：`↳ ` 加这句话，换行压成空格，放不下截掉加 `…`
    const queued = JSON.stringify(s.queued);
    if (queued !== this.drawnQueue) {
      this.drawnQueue = queued;
      this.queue.replaceChildren(...s.queued.map((text) => h('div.pulse-queued', h('span.pulse-arrow', '↳ '), h('span.pulse-queued-text', text.replace(/\s+/g, ' ').trim()))));
    }
  }
}

/** 字变了才写，免得每一下都动 DOM。 */
function setText(el, text) {
  if (el.textContent !== text) el.textContent = text;
}
