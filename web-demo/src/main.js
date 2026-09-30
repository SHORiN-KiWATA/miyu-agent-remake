// @ts-check
//! 网页演示的入口：起内核（`kernel/boot.js`），软件包由内核照发行版加载（蓝图 `web/architecture.md`）。起不来的原因写在页面上。

import { boot, Offline } from './kernel/boot.js';
import { offline } from './model/offline.js';

const root = /** @type {HTMLElement} */ (document.getElementById('app'));
boot(root).catch((err) => {
  console.error(err);
  // 连不上桥：页面正中一张卡，写为什么、怎么办（蓝图 `web.md`「连核心」第 9 条）；别的起不来的照原话
  if (err instanceof Offline) {
    const text = offline(err.kind, err.detail);
    const line = (cls, words) => Object.assign(document.createElement('p'), { className: cls, textContent: words });
    const card = document.createElement('div');
    card.className = 'boot-card';
    card.append(Object.assign(document.createElement('h1'), { textContent: text.title }), line('boot-why', text.why), line('boot-how', text.how));
    root.replaceChildren(card);
    root.classList.add('offline');
    return;
  }
  root.textContent = err.message;
  root.classList.add('fatal');
});
