// @ts-check
//! 链接卡片（蓝图 `web.md`「链接卡片」，照旧版 `linkcards.js`）：一段里只有一个 `http(s)` 链接的，换成一张卡片；句子中间的链接不换。
//!
//! 元数据由桥去抓（`web.link_preview`，以后是核心的 `link.preview`）：页面自己不去别的网站取东西，配图、图标也经桥转一道
//! （`/link-image`）。几条规矩，出问题一律照普通链接、不让正文变样：
//!
//! - 一条回答最多换 3 张（真做成了的才算名额），最多试 6 个；同一个地址只换第一次出现的；
//! - 取不到、超时、不是网页的，什么都不做，不画「载入中」；
//! - 她还在写的，最后一次画完停 250 毫秒再取：地址还没写完就去抓纯属浪费（回答每来一段字整段重画，旧的那份已经不在页面上了，
//!   它的定时到了也不取）。

import { h, icon } from './dom.js';
import { res } from '../util/res.js';
import { linkImageUrl } from '../core/host.js';

/** @type {import('../core/connection.js').Connection|null} */
let conn = null;
/** 地址 → 取到的元数据（取不到的是 `null`）：整页只问一次。 */
const lookups = new Map();
/** 已经取回来的：地址 → 元数据（取不到的是 `null`）。重画时直接换成卡片，不等，不闪。 */
const known = new Map();
/** 做好的卡片：地址 → 节点。重画时挪过去用（配图不重载）；还在页面上的（别处也用着）另造一张。 */
const cards = new Map();
/** 容器 → 还没到点的定时。 */
const pending = new WeakMap();

/** 入口连上核心以后交进来。 */
export function useConnection(c) {
  conn = c;
}

/** 这一段是不是只有一个 `http(s)` 链接（`<br>` 不算）；是的交回那个链接。 */
function soleLink(p) {
  if (p.dataset.linkCard) return null;
  let link = null;
  for (const node of p.childNodes) {
    if (node.nodeType === Node.TEXT_NODE) {
      if (node.textContent?.trim()) return null;
      continue;
    }
    if (!(node instanceof HTMLElement) || node.tagName === 'BR') continue;
    if (node.tagName !== 'A' || link) return null;
    link = node;
  }
  return link && /^https?:\/\//i.test(link.getAttribute('href') ?? '') ? link : null;
}

/** 问桥要一个地址的元数据；问过的直接给。 */
function previewFor(url) {
  if (!lookups.has(url)) {
    const ask = conn
      ? conn.request('web.link_preview', { url }).then((r) => (r?.ok ? r.preview : null)).catch((err) => {
        console.error(`取不到链接卡片 ${url}：${err.message}`);
        return null;
      }).then((preview) => { known.set(url, preview); return preview; })
      : Promise.resolve(null);
    lookups.set(url, ask);
  }
  return lookups.get(url);
}

/** 一张卡片：配图（有的话）、图标和标题、描述、站名和外链图标。 */
function card(preview, href) {
  const media = preview.image
    ? h('div.link-card-media', h('img', { src: linkImageUrl(preview.image), alt: '', loading: 'lazy', decoding: 'async', onerror: (e) => e.target.parentElement?.remove() }))
    : null;
  const mark = preview.icon
    ? h('img.link-card-icon', { src: linkImageUrl(preview.icon), alt: '', loading: 'lazy', onerror: (e) => e.target.remove() })
    : h('span.link-card-icon.is-letter', (preview.site || preview.title || '?').trim().charAt(0).toUpperCase());
  let host = href;
  try { host = new URL(href).host; } catch { /* 写坏了的地址照原样当站名 */ }
  return h('a.link-card', { href, target: '_blank', rel: 'noopener noreferrer', title: preview.title || href },
    media,
    h('div.link-card-body',
      h('div.link-card-head', mark, h('strong.link-card-title', preview.title || href)),
      preview.description ? h('p.link-card-desc', preview.description) : null,
      h('div.link-card-foot', h('span.link-card-site', preview.site || host), withClass(icon('external-link'), 'link-card-arrow'))));
}

/** 给图标加一个类。 */
function withClass(el, name) {
  el.classList.add(name);
  return el;
}

/** 这一块里够格的段落换成卡片（最多几张、试几个照 `markdown.json` 的 `link_cards`）。 */
async function upgrade(container) {
  if (!container.isConnected) return;
  const limits = res.markdown.link_cards;
  const seen = new Set();
  const targets = [];
  for (const p of container.querySelectorAll('p')) {
    if (targets.length >= limits.tries) break;
    const link = soleLink(p);
    if (!link || seen.has(link.href)) continue;
    seen.add(link.href);
    targets.push({ p, href: link.href });
  }
  let made = 0;
  for (const t of targets) t.p.dataset.linkCard = 'pending';
  await Promise.all(targets.map(async (t) => {
    const preview = await previewFor(t.href);
    if (!t.p.isConnected) return;
    if (!preview || made >= limits.max) {
      t.p.dataset.linkCard = 'none';
      return;
    }
    made += 1;
    t.p.dataset.linkCard = 'done';
    t.p.classList.add('has-link-card');
    t.p.replaceChildren(cardFor(preview, t.href));
  }));
}

/** 地址的卡片：做过、这会儿没在页面上的挪过来用，不然造一张。 */
function cardFor(preview, href) {
  const old = cards.get(href);
  if (old && !old.isConnected) return old;
  const node = card(preview, href);
  cards.set(href, node);
  return node;
}

/**
 * 画完一块正文以后调（Markdown 的 `hooks.after`）：已经取回来的地址当场换成卡片（等这一次重画换上页面以后，旧的那份
 * 下了页面，它的卡片好挪过来）；还没取的停一会儿没再画才去取。
 */
export function scanLinks(container) {
  queueMicrotask(() => {
    if (!container.isConnected) return;
    let made = container.querySelectorAll('.link-card').length;
    for (const p of container.querySelectorAll('p')) {
      const link = soleLink(p);
      if (!link || !known.has(link.href)) continue;
      const preview = known.get(link.href);
      if (!preview || made >= res.markdown.link_cards.max) {
        p.dataset.linkCard = 'none';
        continue;
      }
      made += 1;
      p.dataset.linkCard = 'done';
      p.classList.add('has-link-card');
      p.replaceChildren(cardFor(preview, link.href));
    }
  });
  clearTimeout(pending.get(container));
  pending.set(container, setTimeout(() => upgrade(container), res.markdown.link_cards.settle_ms));
}
