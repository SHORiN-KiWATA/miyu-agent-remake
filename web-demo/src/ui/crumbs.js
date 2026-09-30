// @ts-check
//! 子代理的会话顶上那条路径（蓝图 `web.md`「对话区」的「子代理的会话顶上那条路径」）：看着子代理的会话时，对话区顶上一条
//! `主会话 › 上层 › 这一个`，前面几段点了进那个会话，最后一段是现在这个、不能点。主会话里没有。路径照 `model/tree.js` 的 `pathOf`
//! 找（派它的会话照子会话 `session.created` 的 `parent`）；太长的从中间几段起收成 `…`。回派它的那一层的按钮不在这里，在输入框正上方
//! （`BackButton`：从下面进的子代理，回去不用跑到顶上）。

import { h, icon, replace } from './dom.js';
import { show, hide } from '../lib/motion.js';
import { res, t } from '../util/res.js';

export class Crumbs {
  /** @param {(id: string) => void} open 进一个会话 */
  constructor(open) {
    this.open = open;
    this.el = h('nav.chat-crumbs', { hidden: true, 'aria-label': t('crumbs.label') });
    this.drawn = '';
  }

  /**
   * 照路径画；只有一段（主会话、新会话）的藏起来。
   * @param {{session: string, title: string|null}[]} path 从主会话一层层到这一个
   */
  draw(path) {
    const sig = JSON.stringify(path);
    if (sig === this.drawn) return;
    this.drawn = sig;
    if (path.length < 2) {
      if (!this.el.hidden) hide(this.el);
      return;
    }
    const keep = res.layout.crumbs_keep;
    // 太长的：留头一段和后面 `keep - 1` 段，中间收成一个 `…`
    const shown = path.length > keep ? [path[0], null, ...path.slice(path.length - (keep - 1))] : path;
    const parts = shown.flatMap((p, i) => {
      const sep = i ? [h('span.crumb-sep', { 'aria-hidden': 'true' }, icon('chevron-right'))] : [];
      if (!p) return [...sep, h('span.crumb-gap', '…')];
      const title = p.title ?? t('sidebar.untitled');
      const last = i === shown.length - 1;
      return [...sep, last
        ? h('span.crumb.is-here', { title, 'aria-current': 'page' }, title)
        : h('button.crumb', { type: 'button', title, onclick: () => this.open(p.session) }, title)];
    });
    replace(this.el, h('div.crumb-path', parts));
    if (this.el.hidden) show(this.el);
  }
}

/**
 * 回派它的那一层（蓝图同一节）：输入框正上方靠左一个小按钮「← 返回」（悬停提示写回到哪个会话），只在子代理的会话里有；多层的
 * 一层层回；进来从下面升上来、淡入，回主会话收回。
 */
export class BackButton {
  /** @param {(id: string) => void} open 进一个会话 */
  constructor(open) {
    this.open = open;
    this.to = /** @type {string|null} */ (null);
    this.label = h('span.back-label', t('crumbs.back'));
    this.el = h('div.back-row', { hidden: true },
      h('button.back-button', { type: 'button', onclick: () => { if (this.to) this.open(this.to); } }, icon('arrow-left'), this.label));
  }

  /**
   * 照派它的那个会话画；主会话（没有派它的）藏起来。
   * @param {{session: string, title: string|null}|null} parent
   */
  draw(parent) {
    const to = parent?.session ?? null;
    const title = parent ? parent.title ?? t('sidebar.untitled') : '';
    const hint = t('crumbs.back_to', { title });
    const button = /** @type {HTMLElement} */ (this.el.firstChild);
    if (to === this.to && button.title === hint) return;
    this.to = to;
    if (!to) {
      if (!this.el.hidden) hide(this.el);
      return;
    }
    // 只写「返回」，回到哪个会话写在悬停提示里（2026-09-30 项目主人定：原来整个标题写在按钮上太长）；多层的一层层回
    button.title = hint;
    if (this.el.hidden) show(this.el);
  }
}
