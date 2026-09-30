// @ts-check
//! 时间线的一段（蓝图 `web.md`「时间线」，照旧版网页 `app.js:804-1064`、`styles.css:10650-11353`），和她出第一个字之前的
//! 三个球（「她出第一个字之前」，照旧版 `app.js:7813-7896`）。
//!
//! - 进行中的一段展开，没有收起那一行；她开口、这一轮结束就收起，收起那一行照 TUI（`model/words.js` 的 `summary`）。
//! - 一段是一个一直在的节点，里面的步照编号对上（`steps.js`）：流式的字来了只改字，新来的一步淡入一次。
//! - 同一时刻只转一处（`model/timeline.js` 的 `active`）；在走的用时由 [`Ticker`] 走表，不重画。

import { h, icon } from './dom.js';
import { res, t } from '../util/res.js';
import { active } from '../model/timeline.js';
import { summary } from '../model/words.js';
import { seconds, jobDuration } from '../model/format.js';
import { StepView, guard } from './steps.js';

export class SegmentView {
  /**
   * @param {import('./rich.js').Where} where 看着的会话在哪（家目录、会话编号）
   * @param {boolean} fresh 新来的：里面的步淡入（读回来的历史不淡入）
   */
  constructor(where, fresh) {
    this.where = where;
    this.fresh = fresh;
    /** 人点过：开还是关；没点过的是 `null`，照「进行中展开、做完收起」。 */
    this.open = /** @type {boolean|null} */ (null);
    /** @type {Map<string, StepView>} */
    this.views = new Map();
    this.segment = /** @type {import('../model/timeline.js').Segment|null} */ (null);
    this.summary = h('button.tl-summary', { type: 'button', onclick: guard(() => this.toggle()) });
    /** 收起那一行的字；后面的箭头悬停才露（照 Claude 网页端） */
    this.summaryText = h('span.tl-summary-text');
    this.drawnSummary = '';
    this.steps = h('div.tl-steps');
    // 在想时滚几行、命令写几行：CSS 照它定高（`timeline.json`）
    const tl = res.timeline;
    this.el = h(`div.tl-segment${fresh ? '' : '.is-static'}`,
      // 一行里几段之间的分隔（`·`，蓝图「时间线」的「工具一行」）：CSS 照它画
      { style: `--tl-rows: ${tl.thinking_rows}; --tl-cmd-rows: ${tl.command_rows}; --tl-fold: ${tl.fold_ms}ms; --tl-step: ${tl.step_ms}ms; --tl-sep: ${JSON.stringify(t('timeline.peek_sep'))}` },
      this.summary, h('div.tl-wrap', h('div.tl-clip', this.steps)));
    this.summary.append(this.summaryText, h('span.tl-chevron', icon('chevron-right')));
  }

  /** 现在展开着没有。 */
  expanded() {
    const s = this.segment;
    return this.open ?? (!s?.finished || !res.timeline.fold);
  }

  /**
   * 点收起那一行：展开、收起。先告诉对话区钉住这一行（内容往下长，`follow.js`）。只有一步的一段，展开时不画那一步的
   * 那一行，直接铺开它的内容（`tui.md`「时间线」第 15 条，见 `bare`）。
   */
  toggle() {
    this.summary.dispatchEvent(new CustomEvent('tl-toggle', { bubbles: true }));
    this.open = !this.expanded();
    if (this.segment) this.update(this.segment);
  }

  /**
   * 只有一步、人点过收起那一行的一段：那一步不画那一行，直接铺开内容。人点过以后一直这样（收起时藏在收起的动画里，
   * 不来回换样子）；没点过的（做完自动收起的那一下）照常，免得收起的动画里样子一变。
   */
  bare() {
    const s = this.segment;
    return !!s && s.finished && s.steps.length === 1 && this.open != null;
  }

  /** @param {import('../model/timeline.js').Segment} segment */
  update(segment) {
    this.segment = segment;
    this.el.classList.toggle('is-live', !segment.finished);
    this.el.classList.toggle('is-open', this.expanded());
    // 收起那一行：做完了才有
    this.summary.hidden = !segment.finished;
    if (segment.finished) {
      // 字变了才换：换一次淡入一次
      const s = summary(segment.steps, Date.now());
      const sig = JSON.stringify(s);
      if (sig !== this.drawnSummary) {
        this.drawnSummary = sig;
        this.summary.classList.toggle('is-failed', s.failed);
        this.summaryText.replaceChildren(...s.spans.map((span) => (span.tone === 'base' ? span.text : h(`span.tl-${span.tone}`, span.text))));
      }
    }
    const spinning = active(segment);
    const bare = this.bare();
    const keep = new Set();
    let prev = null;
    segment.steps.forEach((step, j) => {
      let view = this.views.get(step.key);
      if (!view) {
        view = new StepView(step, this.where, this.fresh, () => this.toggle());
        this.views.set(step.key, view);
      }
      view.bare = bare;
      view.update(step, spinning === j, !segment.finished);
      keep.add(step.key);
      const want = prev ? prev.nextSibling : this.steps.firstChild;
      if (want !== view.el) this.steps.insertBefore(view.el, want);
      prev = view.el;
    });
    for (const [key, view] of this.views) {
      if (keep.has(key)) continue;
      view.el.remove();
      this.views.delete(key);
    }
  }
}

/** 三个球：金、缎带、蓝摆成三角，转、聚、展开，6 秒一圈（旧版 `styles.css:4831-4875` 原样）。 */
export function waitingNode() {
  return h('span.miyu-run.typing-run', { 'aria-hidden': 'true' }, h('span.mr-spin', h('i.mr1'), h('i.mr2'), h('i.mr3')));
}

/** 走表的写法：`secs` 整秒 `12s`（在想），`tenths` 一位小数 `1.2 s`（准备），`job` 读秒 `3m 05s`（在跑的命令）。 */
const FORMATS = {
  secs: (ms) => `${Math.floor(ms / 1000)}s`,
  tenths: (ms) => seconds(ms).replace(/s$/, ' s'),
  job: (ms) => jobDuration(ms / 1000),
};

/** 在走的用时：页面上有在走的才走，多久一格照 `timeline.json` 的 `tick_ms`。 */
export class Ticker {
  /** @param {HTMLElement} root 在它里面找 */
  constructor(root) {
    this.root = root;
    this.timer = 0;
  }

  /** 画完一次调一次：有在走的就走表，没有就停。 */
  update() {
    const busy = this.root.querySelector('.tl-timer');
    if (busy && !this.timer) this.timer = window.setInterval(() => this.tick(), res.timeline.tick_ms);
    if (!busy && this.timer) {
      clearInterval(this.timer);
      this.timer = 0;
    }
    this.tick();
  }

  tick() {
    const now = Date.now();
    for (const el of /** @type {NodeListOf<HTMLElement>} */ (this.root.querySelectorAll('.tl-timer'))) {
      const write = FORMATS[el.dataset.format ?? 'secs'];
      el.textContent = (el.dataset.lead ?? '') + write(Math.max(0, now - Number(el.dataset.since)));
    }
  }
}
