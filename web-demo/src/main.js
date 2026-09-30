// @ts-check
//! 网页演示的入口：起内核（`kernel/boot.js`），软件包由内核照发行版加载（蓝图 `web/architecture.md`）。起不来的原因写在页面上。

import { boot } from './kernel/boot.js';

const root = /** @type {HTMLElement} */ (document.getElementById('app'));
boot(root).catch((err) => {
  console.error(err);
  root.textContent = err.message;
  root.classList.add('fatal');
});
