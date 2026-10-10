// @ts-check
//! 「模型」页的「＋ 添加供应商」（蓝图 `web.md`「第一次引导」第 12 条：和引导用同一份，2026-10-08 项目主人）：右边换成接一家供应商的
//! 几屏（`onboard/flow.js`，设置页的那一种：最后的按钮是「保存」，只写这一家）。存好了回到模型页、选中它。还想照原来那张表一项项填的
//! （自己起编号、写显示名、写环境变量名），选一家那一屏最下面「手动填写…」打开原来的表（`provider-form.js`）。
//! 设置页收到配置推送会整页重画：这一段画好的那一屏记在 `dialog.adding` 上，重画时原样放回去，写了一半的字不丢。

import { h, icon } from '../../src/lib/dom.js';
import { ProviderFlow } from './onboard/flow.js';

/** 开始添加：先画选一家。 @param {any} dialog */
export function openAdd(dialog) {
  const adding = { node: /** @type {HTMLElement|null} */ (null), flow: /** @type {ProviderFlow|null} */ (null) };
  dialog.adding = adding;
  dialog.modelDetail = null;
  dialog.providerForm = null;
  const close = () => {
    if (dialog.adding === adding) dialog.adding = null;
    dialog.drawBody();
  };
  adding.flow = new ProviderFlow(dialog.ctx, {
    panel: dialog.panel,
    show: (screen) => {
      if (dialog.adding !== adding) return;
      adding.node = frame(dialog, screen, close);
      dialog.drawBody();
    },
    ready: () => adding.node?.dispatchEvent(new CustomEvent('ob-ready')),
    done: (got) => {
      if (dialog.adding === adding) dialog.adding = null;
      dialog.provider = got.id;
      Promise.resolve(dialog.modelsPromise).then(() => dialog.drawBody());
    },
    manual: () => {
      if (dialog.adding === adding) dialog.adding = null;
      dialog.providerForm = { id: null };
      dialog.drawBody();
    },
  }, 'settings');
  adding.flow.start();
}

/** 这一段现在的那一屏（还没画出来的写「正在加载」）。 @param {any} dialog */
export function drawAdd(dialog) {
  return dialog.adding?.node ?? h('p.set-empty', dialog.ctx.text('loading'));
}

/**
 * 一屏放进设置页的样子（同编辑供应商的表）：标题、内容、出错的字，下面一排「‹ 上一步」或「取消」、「保存」。
 * @param {any} dialog @param {import('./onboard/flow.js').Screen} screen @param {() => void} close
 */
function frame(dialog, screen, close) {
  const t = (/** @type {string} */ key) => dialog.ctx.text(key);
  const error = h('p.set-error', { hidden: true });
  const next = screen.next;
  const save = next ? /** @type {HTMLButtonElement} */ (h('button.set-btn.is-primary', { type: 'button', onclick: async () => {
    if (!next.ready() || save.disabled) return;
    save.disabled = true;
    error.hidden = true;
    const why = await next.run();
    if (why) {
      error.textContent = why;
      error.hidden = false;
    }
    save.disabled = !next.ready();
  } }, next.label)) : null;
  const sync = () => { if (save) save.disabled = !next?.ready(); };
  sync();
  const back = screen.back
    ? h('button.set-btn', { type: 'button', onclick: screen.back }, icon('chevron-left'), t('onboard.back'))
    : h('button.set-btn', { type: 'button', onclick: close }, t('models.form.cancel'));
  const el = h('section.set-form.ob-in-settings', { 'data-set-dismiss': '' },
    h('h3.set-form-title', screen.title), screen.sub ? h('p.set-desc', screen.sub) : null,
    screen.body, error, h('div.set-form-buttons', back, save));
  el.addEventListener('ob-ready', sync);
  // `Esc`：在一家那一屏回到选一家，选一家那一屏关掉
  el.addEventListener('set-dismiss', () => (screen.back ? screen.back() : close()));
  if (screen.focus) queueMicrotask(() => screen.focus?.focus({ preventScroll: true }));
  return el;
}
