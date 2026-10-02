// @ts-check
//! mermaid 图（软件包 `mermaid`，蓝图 `web.md`「mermaid 图」）：收齐了的 ```` ```mermaid ```` 由核心画成 SVG（`mermaid.render`，
//! `draw.js`），直接画在她的正文上，不加卡片的底。样子照旧版 `app.js:4580-4698`：上面一行暗的小字「图表」，右边切「源码」、
//! 复制；缩小了的写「· 点开看原图」，点图在灯箱里看。画不出来的这一块原地换成代码块。
//!
//! - SVG 里的字、线是页面的 CSS 变量（`draw.js` 换好的），换主题不重画；插进页面前先过一遍：去掉 `<script>`、`on…` 属性、
//!   `javascript:` 的链接（图里的字是她写的，渲染器转没转义不归我们管）；
//! - 灯箱里的那份是一张图片（`blob:` 地址），看不到页面的变量：先照现在的主题把颜色填成定值、铺上底色；
//! - 用到的都由入口交进来（`Deps`）：问核心画图、代码块、复制、灯箱、这个包的字；没有模块级的状态。

import { h, icon, scaleOf } from '../../src/lib/dom.js';

/**
 * @typedef {{draw: (source: string) => Promise<string|null>, codeBlock: (block: any, ctx: any) => HTMLElement,
 *   copy: (text: string, say?: (text: string, good?: boolean) => void) => void,
 *   open: (what: {url: string, name?: string, vector?: boolean}) => void, t: (path: string, fields?: any) => string}} Deps
 *   问核心画一张（问过的直接给）、Markdown 的代码块和复制、点开看大图（有灯箱在灯箱里，没有在新标签页）、这个包的字
 */

/**
 * 一张图：先写「正在画图…」，画好了换上；画不出来的这一块原地换成代码块。
 * @param {string} source mermaid 源码
 * @param {(text: string, good?: boolean) => void} say 提示（复制了几个字）
 * @param {Deps} deps
 */
export function mermaidBlock(source, say, deps) {
  const { draw, codeBlock, copy, open: lightbox, t } = deps;
  const label = h('span', t('label'));
  const figure = h('div.mermaid-figure', t('drawing'));
  const code = codeBlock({ lang: 'mermaid', text: source, closed: true }, { hooks: {}, say });
  code.hidden = true;
  const toggle = h('button.mermaid-source-toggle', { type: 'button', 'aria-expanded': 'false' }, t('source'));
  // 切过去、切回来这一块高度不变：源码照图的高度，长了在里面滚、短了下面空着（蓝图「mermaid 图」第 3 条）
  toggle.addEventListener('click', () => {
    const showing = code.hidden;
    if (showing) code.style.height = `${figure.offsetHeight}px`;
    code.hidden = !showing;
    figure.hidden = showing;
    toggle.textContent = t(showing ? 'diagram' : 'source');
    toggle.classList.toggle('is-active', showing);
    toggle.setAttribute('aria-expanded', String(showing));
  });
  const copyButton = h('button.code-copy-button', { type: 'button', title: t('copy'), 'aria-label': t('copy'), onclick: () => copy(source, say) }, icon('copy'));
  const block = h('div.mermaid-block', h('div.mermaid-toolbar', label, h('div.mermaid-actions', toggle, copyButton)), figure, code);
  draw(source).then((svg) => {
    const node = svg && clean(svg);
    if (!node) {
      // 原地换成代码块：回答重画时这一块会被挪过去重用（`rich.js` 的 `reuse`），把自己换出去的话重画又把「正在画图…」放回来
      code.hidden = false;
      block.classList.add('is-plain');
      block.replaceChildren(code);
      return;
    }
    figure.replaceChildren(node);
    figure.classList.add('is-zoomable');
    figure.setAttribute('role', 'button');
    figure.tabIndex = 0;
    const open = () => {
      const url = standalone(node);
      lightbox({ url, name: t('label'), vector: true });
    };
    figure.addEventListener('click', open);
    figure.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        open();
      }
    });
    // 画出来比原图小的，提示点开看原图
    requestAnimationFrame(() => {
      const natural = Number(node.getAttribute('height')) || 0;
      if (natural && node.getBoundingClientRect().height / scaleOf(figure) + 1 < natural) label.textContent = `${t('label')} · ${t('zoom')}`;
    });
  });
  return block;
}

/** 核心给的 SVG 文字 → 一个能插进页面的 `<svg>`：去掉脚本、`on…` 属性、`javascript:` 链接；读不懂的是 `null`。 */
function clean(text) {
  const doc = new DOMParser().parseFromString(text, 'image/svg+xml');
  const svg = doc.documentElement;
  if (svg.nodeName !== 'svg' || doc.querySelector('parsererror')) return null;
  for (const el of [svg, ...svg.querySelectorAll('*')]) {
    if (el.nodeName.toLowerCase() === 'script') {
      el.remove();
      continue;
    }
    for (const attr of [...el.attributes]) {
      const bad = attr.name.toLowerCase().startsWith('on') || (/href$/i.test(attr.name) && /^\s*javascript:/i.test(attr.value));
      if (bad) el.removeAttribute(attr.name);
    }
  }
  svg.style.maxWidth = '100%';
  svg.style.height = 'auto';
  return /** @type {SVGSVGElement} */ (document.importNode(svg, true));
}

/** 灯箱里看的那份：CSS 变量照现在的主题换成定值，铺上正文的底色，做成一个 `blob:` 地址。 */
function standalone(svg) {
  const style = getComputedStyle(document.documentElement);
  const text = new XMLSerializer().serializeToString(svg).replace(/var\((--[\w-]+)\)/g, (all, name) => style.getPropertyValue(name).trim() || all);
  const copyNode = new DOMParser().parseFromString(text, 'image/svg+xml').documentElement;
  copyNode.setAttribute('style', `background: ${style.getPropertyValue('--surface').trim()}`);
  // 灯箱照宽高比放大：没写像素宽高的（百分比、没写）照 viewBox 补上，不然浏览器当它 300×150
  const box = (copyNode.getAttribute('viewBox') ?? '').split(/[\s,]+/).map(Number);
  if (box.length === 4 && box[2] > 0 && box[3] > 0) {
    for (const [name, size] of [['width', box[2]], ['height', box[3]]]) {
      if (!/^\d+(\.\d+)?(px)?$/.test(copyNode.getAttribute(name) ?? '')) copyNode.setAttribute(name, String(size));
    }
  }
  const blob = new Blob([new XMLSerializer().serializeToString(copyNode)], { type: 'image/svg+xml' });
  return URL.createObjectURL(blob);
}
