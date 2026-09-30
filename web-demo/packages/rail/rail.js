// @ts-check
//! 右边的跳转条（蓝图 `web.md`「右边的跳转条」，照 ChatGPT 网页端）：你说的一句话一条短横线，正在看的那条亮；指针靠近
//! 弹出一个浮层列出每一句，点哪句平滑滚到哪句（浮层照样开着，指针离开才收）。哪一句算正在看的由 `reading.js` 算。
//! 右边的空白放不下它（预览工作区开着、窗口窄）就不画，不压着正文。数都是这个包的设置项（清单）。

import { h, offsetIn, scaleOf } from '../../src/lib/dom.js';
import { reading } from './reading.js';

/**
 * @typedef {{min_prompts: number, current_at: number, jump_margin: number, gutter: number, close_ms: number}} Config 这个包的设置项
 */

/** @typedef {{key: string, text: string, node: HTMLElement}} Prompt 你说的一句：编号、原话、气泡那一块 */

export class PromptRail {
  /**
   * @param {HTMLElement} scroller 对话区滚的那一层
   * @param {HTMLElement} list 正文那一列：量它右边还空多少
   * @param {Config} config
   */
  constructor(scroller, list, config) {
    this.config = config;
    this.scroller = scroller;
    this.content = list;
    /** @type {Prompt[]} */
    this.prompts = [];
    this.drawn = '';
    this.current = -1;
    this.closeTimer = 0;
    this.lines = h('div.rail-lines');
    this.list = h('div.rail-list', { role: 'menu' });
    this.pop = h('div.rail-pop', this.list);
    this.el = h('nav.prompt-rail', { hidden: true }, this.pop, this.lines);
    // 指针进这一列或浮层：弹出；离开：过一小会儿收起，在两者之间挪不算离开
    this.el.addEventListener('mouseenter', () => this.show(true));
    this.el.addEventListener('mouseleave', () => this.show(false));
    let frame = 0;
    this.onScroll = () => {
      if (!frame) frame = requestAnimationFrame(() => { frame = 0; this.mark(); });
    };
    scroller.addEventListener('scroll', this.onScroll, { passive: true });
    this.watch = new ResizeObserver(() => this.fit());
    this.watch.observe(scroller);
  }

  /** 画不画：句子够多、右边的空白放得下（`rail.min_prompts`、`rail.gutter`）。 */
  fit() {
    const { min_prompts: least, gutter } = this.config;
    // 正文右边到对话区右边空多少，照 CSS 像素（屏幕上量的除掉整页放大）
    const pad = parseFloat(getComputedStyle(this.content).paddingRight) || 0;
    const room = (this.scroller.getBoundingClientRect().right - this.content.getBoundingClientRect().right) / scaleOf(this.scroller) + pad;
    this.el.hidden = this.prompts.length < least || room < gutter;
  }

  /**
   * 换一份句子（对话区画完以后）：句子变了才重排横线和浮层。
   * @param {Prompt[]} prompts
   */
  update(prompts) {
    this.prompts = prompts;
    this.fit();
    const sig = JSON.stringify(prompts.map((p) => [p.key, p.text]));
    if (sig !== this.drawn) {
      this.drawn = sig;
      this.current = -1;
      this.lines.replaceChildren(...prompts.map((p, i) => h('button.rail-line', { type: 'button', 'aria-label': p.text, onclick: () => this.jump(i) })));
      this.list.replaceChildren(...prompts.map((p, i) => h('button.rail-item', { type: 'button', role: 'menuitem', title: p.text, onclick: () => this.jump(i) }, oneLine(p.text))));
    }
    this.mark();
  }

  /** 照现在滚到哪，亮正在看的那一句。 */
  mark() {
    if (this.el.hidden) return;
    const tops = this.prompts.map((p) => offsetIn(p.node, this.scroller));
    const now = reading(tops, this.scroller.clientHeight, this.config.current_at);
    if (now === this.current) return;
    this.current = now;
    [...this.lines.children].forEach((el, i) => el.classList.toggle('is-current', i === now));
    [...this.list.children].forEach((el, i) => el.classList.toggle('is-current', i === now));
  }

  /** 弹出、收起浮层（CSS 照 `is-open` 淡入淡出）；弹出时正在看的那一句滚进视野。 */
  show(yes) {
    clearTimeout(this.closeTimer);
    if (yes) {
      if (!this.pop.classList.contains('is-open')) this.list.children[this.current]?.scrollIntoView({ block: 'nearest' });
      this.pop.classList.add('is-open');
      return;
    }
    this.closeTimer = window.setTimeout(() => this.pop.classList.remove('is-open'), this.config.close_ms);
  }

  /** 平滑滚到第 `i` 句，它的顶离视口顶 `rail.jump_margin`；算人滚的（`follow.js` 照滚动事件认）。 */
  jump(i) {
    const p = this.prompts[i];
    if (!p) return;
    const top = this.scroller.scrollTop + offsetIn(p.node, this.scroller) - this.config.jump_margin;
    this.scroller.scrollTo({ top, behavior: 'smooth' });
  }

  /** 不画了：停掉量尺寸的、收起的计时。 */
  dispose() {
    this.scroller.removeEventListener('scroll', this.onScroll);
    this.watch.disconnect();
    clearTimeout(this.closeTimer);
  }
}

/** 浮层里一行：换行压成空格，放不下的由 CSS 截掉加 `…`。 */
function oneLine(text) {
  return text.replace(/\s+/g, ' ').trim();
}
