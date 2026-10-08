// @ts-check
//! 造 DOM 的小工具（`lib/`：谁都能用，不依赖别的）：不用框架（设计 21 X2），一个 `h` 够用；图标照内核交进来的图标表画
//! （`resources/lucide.json`，内核起来时 `useIcons`）。

/** 图标表：名字 → 几段 SVG。内核起来时交进来。 */
let icons = /** @type {Record<string, Array<[string, Record<string, string>]>>} */ ({});

/** 内核起来时交进图标表。 */
export function useIcons(table) {
  icons = table;
}

const SVG = 'http://www.w3.org/2000/svg';

/**
 * `h('div.a.b', {onclick, title}, 子节点…)`。属性里 `on*` 挂事件，`style` 是字符串或对象，`dataset` 照常；
 * 值是 `false`、`null` 的属性不写。子节点可以是字、节点、数组，`null` 跳过。
 * @returns {HTMLElement}
 */
export function h(tag, attrs, ...kids) {
  const [name, ...classes] = tag.split('.');
  const el = document.createElement(name || 'div');
  if (classes.length) el.className = classes.join(' ');
  if (attrs && (typeof attrs !== 'object' || attrs instanceof Node || Array.isArray(attrs))) {
    kids.unshift(attrs);
    attrs = null;
  }
  for (const [k, v] of Object.entries(attrs ?? {})) {
    if (v == null || v === false) continue;
    if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else if (k === 'style' && typeof v === 'object') Object.assign(el.style, v);
    else if (k === 'dataset') Object.assign(el.dataset, v);
    else el.setAttribute(k, v === true ? '' : String(v));
  }
  return append(el, kids);
}

/** 往节点里接子节点。 */
export function append(el, kids) {
  for (const k of kids.flat(Infinity)) {
    if (k == null || k === false) continue;
    el.append(k instanceof Node ? k : document.createTextNode(String(k)));
  }
  return el;
}

/** 换掉全部子节点。 */
export function replace(el, ...kids) {
  el.replaceChildren();
  return append(el, kids);
}

/**
 * 换成这几个孩子，已经在里面、先后没变的不拿下来：拿下来再放回去的元素正在走的过渡会作废、一下跳到终点（展开收起到一半重画就一跳）。
 * 空的（`null`、`false`）跳过；不在新的里的拿掉。
 * @param {Element} el @param {(Node|null|undefined|false)[]} kids
 */
export function keep(el, kids) {
  const want = /** @type {Node[]} */ (kids.filter(Boolean));
  for (const old of [...el.childNodes]) if (!want.includes(old)) old.remove();
  want.forEach((node, i) => {
    if (el.childNodes[i] !== node) el.insertBefore(node, el.childNodes[i] ?? null);
  });
  return el;
}

/**
 * 一个 Lucide 图标（`resources/lucide.json`）：24 的画布，线宽 1.8（照旧版 `.icon-slot svg`）。
 * 没登记的名字画一个空的框：看得出漏了，不崩。
 */
export function icon(name) {
  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('aria-hidden', 'true');
  svg.setAttribute('class', 'icon');
  for (const [tag, attrs] of icons[name] ?? []) {
    const part = document.createElementNS(SVG, tag);
    for (const [k, v] of Object.entries(attrs)) part.setAttribute(k, v);
    svg.append(part);
  }
  return svg;
}

/**
 * `node` 的顶边离 `box` 的顶边多远，照 `box` 自己的 CSS 像素（和 `scrollTop`、`clientHeight` 一个单位）。整页放大
 * （`--ui-scale`，1.1）以后 `getBoundingClientRect` 量的是屏幕上的像素，和滚动的像素差一成，得换回来（2026-09-30 查出：
 * 原来混着算，长回答停住、跳转条跳过去的位置越往后越偏）。
 * @param {Element} node
 * @param {HTMLElement} box
 */
export function offsetIn(node, box) {
  return (node.getBoundingClientRect().top - box.getBoundingClientRect().top) / scaleOf(box);
}

/** `box` 在屏幕上放大了几倍：屏幕上的像素 ÷ 它自己的 CSS 像素（整页放大 1.1 时是 1.1）。 */
export function scaleOf(box) {
  return box.offsetWidth ? box.getBoundingClientRect().width / box.offsetWidth : 1;
}
