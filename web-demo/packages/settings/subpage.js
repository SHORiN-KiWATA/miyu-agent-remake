// @ts-check
//! 设置页里点进去的一层（蓝图 `web.md`「设置页」第 13 条；2026-10-10 项目主人：软件包页点一个软件包，滑动进入它的设置页，左上角一个箭头
//! 返回软件包页）：右边整栏换成这一层，页头写它的名字、左边一个返回箭头；进去时新的从右边滑进来、原来的往左滑走，返回时反过来。
//! 换左边的分页、搜索时直接收掉，不滑。回来时列表回到进去前滚到的地方。

import { h, replace } from '../../src/lib/dom.js';

/** 滑的方向：进去 `push`、回来 `pop` @typedef {'push'|'pop'} Dir */

export class Subpage {
  constructor() {
    /**
     * 开着的那一层：名字、画法、进去前列表滚到哪；`keep` 的画过一次就不重画（软件后台页的框：重画会把框挪出、挪进文档，页面整个重新载入），
     * `leave` 是收掉时要做的；没开是 `null`
     * @type {{title: string, render: () => any, scroll: number, keep: boolean, shown: boolean, leave: (() => void)|null}|null}
     */
    this.page = null;
  }

  /** 进去：记下列表滚到哪。 @param {string} title @param {() => any} render @param {number} scroll @param {{keep?: boolean, leave?: () => void}} [opts] */
  open(title, render, scroll, opts = {}) {
    this.close();
    this.page = { title, render, scroll, keep: !!opts.keep, shown: false, leave: opts.leave ?? null };
  }

  /** 回来：交回进去前列表滚到哪。 */
  close() {
    const scroll = this.page?.scroll ?? 0;
    const leave = this.page?.leave;
    this.page = null;
    leave?.();
    return scroll;
  }
}

/** 系统设了少动画的不滑。 */
const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

/**
 * 换右边的内容，滑一下：原来的内容原地留一层往旁边滑走、淡掉，新的从另一边滑进来。`dir` 是空的直接换。
 * @param {HTMLElement} body 右边那一栏（能滚的那一层） @param {any} kids 新的内容（节点、套着的数组、空的都行） @param {Dir|null} dir @param {number} scroll 新的内容滚到哪
 */
export function swap(body, kids, dir, scroll) {
  if (!dir || reduced()) {
    replace(body, kids);
    body.scrollTop = scroll;
    return;
  }
  const before = body.scrollTop;
  const old = [...body.childNodes];
  const next = h(`div.set-slide.is-${dir}`, kids);
  body.replaceChildren(next);
  body.scrollTop = scroll;
  // 原来的那一层摆在原来看得到的位置：滚动换了，照差值往上下挪
  const ghost = h(`div.set-slide-ghost.is-${dir}`, { 'aria-hidden': 'true', style: { top: `${body.scrollTop - before}px` } }, old);
  body.append(ghost);
  ghost.addEventListener('animationend', () => ghost.remove(), { once: true });
}
