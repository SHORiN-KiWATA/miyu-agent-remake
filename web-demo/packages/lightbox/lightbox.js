// @ts-check
//! 灯箱（软件包 `lightbox`，蓝图 `web.md`「图片」第 3 条，照旧版 `lightbox.js`、`styles.css:3237-3296`、`3552-3590`）：对话里的
//! 图片、mermaid 图点开放大看。背景压暗、糊一点，图放大到视口里，下面一条：说明、在预览工作区打开（给了才有）、新标签页打开、
//! 下载、关。`Esc`、点外面、点「关」都关上，焦点回到点开之前的地方；关上时松开 `src`，大图不一直占着内存。
//!
//! 图片照原大，放不下才缩（CSS 的最宽最高）；矢量的图（mermaid 图）放大到放得下的最大，窗口变大小跟着重算。
//! 一次加载一个（没有模块级的状态）：第一次打开时才造节点，`destroy` 拿掉节点和挂的事件。

import { h, icon } from '../../src/lib/dom.js';
import { show, leave } from '../../src/lib/motion.js';

/** @typedef {{url: string, name?: string, workspace?: () => void, vector?: boolean}} What 打开什么 */

export class Lightbox {
  /** @param {(path: string) => string} t 这个包的字 */
  constructor(t) {
    this.t = t;
    /** @type {{root: HTMLElement, image: HTMLImageElement, caption: HTMLElement, workspace: HTMLElement, external: HTMLAnchorElement, download: HTMLAnchorElement}|null} */
    this.box = null;
    /** @type {Element|null} */
    this.lastFocused = null;
    /** @type {(() => void)|null} */
    this.toWorkspace = null;
    this.vector = false;
    this.fit = () => this.resize();
    this.onKey = (/** @type {KeyboardEvent} */ e) => {
      // 在捕获阶段接住，别让它再去清空输入框、打断回答
      if (e.key !== 'Escape') return;
      e.stopPropagation();
      this.close();
    };
  }

  build() {
    const t = this.t;
    const image = /** @type {HTMLImageElement} */ (h('img.lightbox-image', { decoding: 'async', alt: '' }));
    const caption = h('span.lightbox-caption');
    const workspace = action('panel-right', t('workspace'));
    workspace.addEventListener('click', () => {
      const go = this.toWorkspace;
      this.close();
      go?.();
    });
    const external = /** @type {HTMLAnchorElement} */ (action('external-link', t('external'), 'a'));
    external.target = '_blank';
    external.rel = 'noreferrer noopener';
    const download = /** @type {HTMLAnchorElement} */ (action('download', t('download'), 'a'));
    download.setAttribute('download', '');
    const shut = action('x', t('close'));
    shut.addEventListener('click', () => this.close());
    image.addEventListener('load', this.fit);
    addEventListener('resize', this.fit);
    const root = h('div.lightbox', { hidden: true, role: 'dialog', 'aria-modal': 'true', 'aria-label': t('label') },
      h('button.lightbox-scrim', { type: 'button', tabindex: '-1', 'aria-label': t('close'), onclick: () => this.close() }),
      h('div.lightbox-frame', image, h('div.lightbox-bar', caption, workspace, external, download, shut)));
    document.body.append(root);
    return { root, image, caption, workspace, external, download };
  }

  /**
   * 矢量的图放大到放得下的最大：宽照灯箱里能用的宽，高照图片的最高（CSS 的 `max-height`），照图的宽高比取小的那个。
   * 图片（不是矢量的）交给 CSS：照原大，放不下才缩。
   */
  resize() {
    const box = this.box;
    if (!box || box.root.hidden) return;
    const image = box.image;
    image.style.width = '';
    image.style.height = '';
    if (!this.vector || !image.naturalWidth || !image.naturalHeight) return;
    const pad = getComputedStyle(box.root);
    const room = box.root.clientWidth - parseFloat(pad.paddingLeft) - parseFloat(pad.paddingRight);
    const tall = parseFloat(getComputedStyle(image).maxHeight) || box.root.clientHeight;
    const scale = Math.min(room / image.naturalWidth, tall / image.naturalHeight);
    image.style.width = `${Math.floor(image.naturalWidth * scale)}px`;
    image.style.height = `${Math.floor(image.naturalHeight * scale)}px`;
  }

  /**
   * 打开一张图。`workspace` 给了的，下面多一个「在预览工作区打开」；`vector` 是矢量的图，放大到放得下的最大。
   * @param {What} what
   */
  open(what) {
    if (!what.url) return;
    this.box ??= this.build();
    const box = this.box;
    this.toWorkspace = what.workspace ?? null;
    this.vector = !!what.vector;
    box.image.src = what.url;
    box.image.alt = what.name ?? '';
    box.caption.textContent = what.name ?? '';
    box.caption.title = what.name ?? '';
    box.external.href = what.url;
    box.download.href = what.url;
    box.workspace.hidden = !this.toWorkspace;
    this.lastFocused = document.activeElement;
    show(box.root);
    document.body.classList.add('has-lightbox');
    document.addEventListener('keydown', this.onKey, true);
  }

  /** 关上。 */
  close() {
    const box = this.box;
    if (!box || box.root.hidden || box.root.classList.contains('is-leaving')) return;
    // 遮罩淡出、图缩回去，走完再藏、松开大图（蓝图「动效」）
    leave(box.root, () => {
      box.root.hidden = true;
      box.image.removeAttribute('src');
    });
    this.toWorkspace = null;
    document.body.classList.remove('has-lightbox');
    document.removeEventListener('keydown', this.onKey, true);
    if (this.lastFocused instanceof HTMLElement && document.contains(this.lastFocused)) this.lastFocused.focus();
    this.lastFocused = null;
  }

  /** 停用了：关上，拿掉节点和挂的事件。 */
  destroy() {
    this.close();
    removeEventListener('resize', this.fit);
    this.box?.root.remove();
    this.box = null;
  }
}

/** 一个 30×30 的图标按钮，或者链接。 */
function action(name, label, tag = 'button') {
  return h(`${tag}.icon-button.lightbox-action`, { type: tag === 'button' ? 'button' : null, title: label, 'aria-label': label }, icon(name));
}
