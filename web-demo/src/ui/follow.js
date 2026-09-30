// @ts-check
//! 对话区滚到哪（蓝图 `web.md`「对话区」的「滚动」，规矩照 `tui.md`「正文」第 1 条、「时间线」第 16 条）。算数的纯函数在
//! `model/anchor.js`，这里接滚动、量高度、改 `scrollTop`。
//!
//! - 跟着最新的时视口只往下走、不往回退：内容变短时停在到过的最深处，底下垫一块空白等她接下来的字填上。一段时间线收起
//!   （她开口、一轮结束）也一样：从下往上收，上面的不动（2026-09-30 项目主人定：原来收起那一刻放开一次，看着是从上往下收）。
//!   收得多、最新的内容要整个退到视口上面去的，视口跟着往上走，最新的底边留在视口从上往下 `follow_keep`（四成）的地方
//!   （2026-10-01 项目主人指出：长的时间线收起到上面去了，视口没跟过去，只剩空白）。
//!   一轮结束了（不会再有字来填）还垫着的，收起的动画走完以后慢慢收掉（`release`），不越过清空那一行（同一天项目主人指出：
//!   一轮结束以后底下留着一大块空档，发一句话才没）。
//! - 清空了上下文：「上下文已清空」那一行顶到视口最上面，下面空着接新的（`toTop`，照终端的 `Ctrl+L`）；之后发话、她回答都从它
//!   下面接着排，不跳到底（`base`：新的一屏从这里起），排满一屏以后照常跟着最新的。换了会话才忘掉。
//! - 她的一段正文长过视口：第一行顶到视口最上面就停住，停住以后上面的内容变短（前面那一段收起）它也钉在那里；
//!   她开始下一步回到最底下接着跟；一段只停一次，人滚过的不动。
//! - 点开、收起时间线：被点的那一行留在屏幕原来的地方，内容往下长、往上收，收完照人在不在底下重新定跟不跟。
//! - 人滚上去就钉住，滚回底再跟着；你发出一句话，清掉垫的空白，回到最底下。往上滚的那一下（滚轮往上、`PageUp`、`↑`、`Home`）
//!   就算离开：滚轮是一小段一小段平滑地滚，头一小段还在「算在底下」的范围里，不这样的话下一段字来了又被拉回底下，一抖一抖的。
//!   只有人动过（按下指针、滚轮、触摸、翻页键）以后一小会儿（`scroll_intent_ms`）里的滚动算人滚的：内容变矮时浏览器自己把视口
//!   往回收，不算，跟着最新的照旧放回去（2026-09-30 项目主人指出：她的思考滚着滚着，清空留的空档没了——那几行变矮时浏览器收了
//!   一下，被当成人往上滚，不再跟）。
//! - 重做（编辑、重来）：记住那一句在屏幕上的位置，撤掉的内容空出来的地方垫着、视口不退，重发的那一句落回同一个位置。

import { res } from '../util/res.js';
import { offsetIn } from './dom.js';
import { anchorAt, atBottom, pinAt, padFor } from '../model/anchor.js';

export class Follow {
  /**
   * @param {HTMLElement} el 滚的那一层
   * @param {HTMLElement} list 内容
   * @param {HTMLElement} spacer 内容底下垫的空白
   */
  constructor(el, list, spacer) {
    this.el = el;
    this.list = list;
    this.spacer = spacer;
    this.follow = true;
    /** 跟着最新的时视口到过的最深处（`scrollTop`）：内容变短也不往回退。 */
    this.floor = 0;
    /** 自己最后一次把视口放到哪：滚动事件落在这里的是自己放的，不是人滚的。 */
    this.placed = /** @type {number|null} */ (null);
    /** 上一次滚动时视口在哪：比它往上的是人在往上滚 */
    this.lastTop = 0;
    /** 点开、收起时钉着的那一行和它离视口顶上多远；动画走完放开。 */
    this.hold = /** @type {{node: Element, top: number}|null} */ (null);
    this.holdTimer = 0;
    /**
     * 最后那段正文：`key` 是它的范围（回合加第几段，落了盘也不变），`streaming` 是还在写。长过视口时照它停住，
     * 停住以后照它钉着。每次画的时候由对话区交进来。
     */
    this.reply = /** @type {{key: string, node: HTMLElement, streaming: boolean}|null} */ (null);
    /** 停住了的那段正文；`auto` 是还没被人滚过（她开始下一步时回到最底下）。 */
    this.pin = /** @type {{key: string, auto: boolean}|null} */ (null);
    /** 停过的正文：一段只停一次。 */
    this.pinned = new Set();
    /** 重做时记下的：那一句离视口顶多远；重发的那一句来了照它放 */
    this.kept = /** @type {{top: number}|null} */ (null);
    /** 新的一屏从哪起（`scrollTop`）：清空以后那一行顶上去的地方；你发话时回到它，不回到 0 */
    this.base = 0;
    el.addEventListener('scroll', () => this.scrolled(), { passive: true });
    /** 人最后一次动（按下指针、滚轮、触摸、翻页键）是什么时候：这之后一小会儿里的滚动才算人滚的 */
    this.intentAt = 0;
    /** 一轮结束以后收掉垫的空白：排着的那一次 */
    this.releaseTimer = 0;
    const intend = () => { this.intentAt = performance.now(); };
    document.addEventListener('pointerdown', intend, { capture: true, passive: true });
    el.addEventListener('wheel', intend, { passive: true });
    el.addEventListener('touchmove', intend, { passive: true });
    document.addEventListener('keydown', (e) => { if (['PageUp', 'PageDown', 'ArrowUp', 'ArrowDown', 'Home', 'End', ' '].includes(e.key)) intend(); }, { capture: true });
    el.addEventListener('wheel', (e) => { if (e.deltaY < 0) this.leave(); }, { passive: true });
    el.addEventListener('keydown', (e) => { if (['PageUp', 'ArrowUp', 'Home'].includes(e.key)) this.leave(); });
    // 高度一变就对一次：收起、展开是动画，一帧一帧地变
    const watch = new ResizeObserver(() => this.anchor());
    watch.observe(list);
    watch.observe(el);
  }

  /** 滚了：自己放的不算；人滚的照新的地方定跟不跟（往上滚的一律不跟，往下滚回底边才跟），停住的那段交给人。 */
  scrolled() {
    const top = this.el.scrollTop;
    const up = top < this.lastTop - 1;
    this.lastTop = top;
    if (this.placed != null && Math.abs(top - this.placed) <= 1) return;
    // 没人动过：内容变矮时浏览器自己收的，不算人滚；跟着最新的照旧放回去
    if (performance.now() - this.intentAt > res.layout.scroll_intent_ms) {
      if (this.follow || this.pin?.auto) this.anchor();
      return;
    }
    this.placed = null;
    this.hold = null;
    this.follow = !up && atBottom(this.el, res.layout.follow_within);
    if (!this.follow) this.floor = Math.min(this.floor, this.el.scrollTop);
    if (this.pin) {
      this.pin.auto = false;
      if (this.follow) this.pin = null;
    }
  }

  /**
   * 一轮结束了（不会再有字来填）：收起的动画（`fold_ms`）走完以后，垫的空白在 `release_ms` 里慢慢收掉，上面的内容落下来补满；
   * 清空过的不越过那一行（`base`）。人不在跟着的、钉着一行的不收；收的时候人一动就停。
   */
  release() {
    clearTimeout(this.releaseTimer);
    this.releaseTimer = window.setTimeout(() => {
      if (!this.follow || this.hold || this.pin?.auto) return;
      const target = Math.max(this.base, this.list.offsetHeight - this.el.clientHeight, 0);
      const from = this.floor;
      if (from <= target + 1) return;
      const start = performance.now();
      const ms = res.layout.release_ms;
      const step = (/** @type {number} */ now) => {
        if (this.intentAt > start || !this.follow) return;
        const k = Math.min(1, (now - start) / ms);
        this.floor = from + (target - from) * (1 - (1 - k) ** 3);
        this.anchor();
        if (k < 1) requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    }, res.timeline.fold_ms);
  }

  /** 人往上滚了：不再跟着最新的，钉在这里（停住的正文交给人）。 */
  leave() {
    this.placed = null;
    this.hold = null;
    this.follow = false;
    this.floor = Math.min(this.floor, this.el.scrollTop);
    if (this.pin) this.pin.auto = false;
  }

  /** 照现在的样子放视口：钉着一行的保它不动；停住的正文钉在顶上；跟着最新的往下走，碰到长过视口的正文停住。 */
  anchor() {
    const view = this.el.clientHeight;
    // 对话区没有高度（空会话时输入框居中、对话区收成 0）：不算停在哪，不然到过的最深处被记成整段内容的高度
    if (!view) return;
    const natural = this.list.offsetHeight;
    if (this.hold) {
      const top = this.el.scrollTop + offsetIn(this.hold.node, this.el) - this.hold.top;
      this.place(Math.max(0, top), view, natural);
      return;
    }
    const reply = this.reply;
    const margin = res.layout.pin_margin;
    if (this.pin?.auto) {
      if (reply?.key === this.pin.key) this.place(Math.max(0, offsetIn(reply.node, this.list) - margin), view, natural);
      return;
    }
    if (!this.follow) return;
    // 收得多、最新的要整个退到视口上面去的：跟着往上走，底边留在视口 `follow_keep` 那么深的地方（不越过清空那一行）
    let { top } = anchorAt(this.floor, view, natural, view * res.layout.follow_keep, this.base);
    if (reply?.streaming && !this.pinned.has(reply.key)) {
      const pin = pinAt(top, offsetIn(reply.node, this.list), margin);
      if (pin.pinned) {
        top = pin.top;
        this.pinned.add(reply.key);
        this.pin = { key: reply.key, auto: true };
        this.follow = false;
      }
    }
    this.place(top, view, natural);
    this.floor = top;
  }

  /** 视口放到 `top`，底下不够的垫上。 */
  place(top, view, natural) {
    this.spacer.style.height = `${padFor(top, view, natural)}px`;
    this.el.scrollTop = top;
    this.placed = this.el.scrollTop;
    this.lastTop = this.placed;
  }

  /**
   * 清空了上下文：那一行顶到视口最上面（留 `pin_margin`），下面空着接新的，往上滚还看得到以前的；接着照常跟着最新的。
   * @param {Element} node 「上下文已清空」那一行
   */
  toTop(node) {
    const top = Math.max(0, offsetIn(node, this.list) - res.layout.pin_margin);
    this.hold = null;
    this.pin = null;
    this.kept = null;
    this.follow = true;
    this.floor = top;
    this.base = top;
    this.place(top, this.el.clientHeight, this.list.offsetHeight);
  }

  /** 她开始下一步了（思考、调工具）：停住着、人没滚过的，回到最底下接着跟。 */
  stepped() {
    if (!this.pin?.auto) return;
    this.pin = null;
    this.follow = true;
    this.anchor();
  }

  /**
   * 人点开、收起了时间线的一段、一步：被点的那一行钉在原地，动画走完照人在不在底下重新定（一段照 `timeline.json` 的
   * `fold_ms`、一步（回报那一行照一步做）照 `step_ms`）。
   * @param {Element} node 被点的那一行
   */
  toggled(node) {
    const ms = node.closest('.tl-step, .note') ? res.timeline.step_ms : res.timeline.fold_ms;
    this.hold = { node, top: offsetIn(node, this.el) };
    clearTimeout(this.holdTimer);
    this.holdTimer = window.setTimeout(() => {
      this.hold = null;
      this.follow = atBottom(this.el, res.layout.follow_within);
      if (this.follow) this.floor = this.el.scrollTop;
    }, ms);
  }

  /**
   * 重做开始：记住这一句在屏幕上的位置；撤掉的内容空出来的地方垫着，视口不退（照跟着最新的、到过的最深处不往回退）。
   * @param {Element} node 被重做的那一句
   */
  keep(node) {
    this.kept = { top: offsetIn(node, this.el) };
    this.floor = this.el.scrollTop;
    this.follow = true;
    this.hold = null;
    this.pin = null;
  }

  /** 重做没发成：忘掉记下的位置。 */
  forget() {
    this.kept = null;
  }

  /**
   * 你新说的一句来了：重做记下的，放回原来的位置；别的回到最底下（`toLatest`）。
   * @param {Element} node 新来的那一句
   */
  arrived(node) {
    if (!this.kept) {
      this.toLatest();
      return;
    }
    const want = this.kept.top;
    this.kept = null;
    const top = Math.max(0, this.el.scrollTop + offsetIn(node, this.el) - want);
    this.place(top, this.el.clientHeight, this.list.offsetHeight);
    this.floor = top;
    this.follow = true;
  }

  /**
   * 从最新的露起：清掉垫的空白（你发出一句话、换了会话）。清空过的，新的一屏从那一行起：垫的留着，照它放（`base`）；
   * 换了会话（`fresh`）的连它一起忘掉。
   */
  toLatest(fresh = false) {
    if (fresh) this.base = 0;
    this.kept = null;
    this.follow = true;
    this.floor = this.base;
    this.hold = null;
    this.pin = null;
    if (!this.base) this.spacer.style.height = '0px';
  }
}
