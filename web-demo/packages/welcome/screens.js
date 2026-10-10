// @ts-check
//! 引导自己画的三屏（蓝图 `web.md`「第一次引导」第 3、4、10 条）：欢迎、界面语言、完成。模型、人格、预设那几屏由别的包交来。

import { h, icon, replace } from '../../src/lib/dom.js';

/** 这台设备上记一笔走到哪一步（换了界面语言要重载页面，重载完接着走） */
export const STEP_KEY = 'welcome.step';
/** 这台设备上记着这个账号走过了引导：一打开不再先盖底色 */
export const DONE_KEY = 'welcome.done';

/**
 * 欢迎（第 3 条）：吉祥物站在标题上面（放大，舞台在 `guide.js`），标题、一行说明、「开始」，下面暗色「按 Enter 开始」。
 * @param {import('./guide.js').Guide} g
 */
export function helloScreen(g) {
  const t = g.t;
  return {
    title: t('hello.title'),
    sub: t('hello.sub'),
    center: true,
    body: h('p.wl-hint', t('hello.hint')),
    next: { label: t('hello.start'), ready: () => true, run: async () => { g.go('language', 1); return null; } },
  };
}

/**
 * 界面语言（第 4 条）：表里的每一种一行（写它自己的名字），浏览器的语言那一行标「跟随系统」；先选中现在用的。设置项是跟着浏览器、
 * 选的又是浏览器的那一种：不写（照旧跟着）；别的和设置项不一样的写进去，要重载的先记一笔接着第 5 条。
 * @param {import('./guide.js').Guide} g
 */
export function languageScreen(g) {
  const t = g.t;
  const lang = g.ctx.language;
  const { items } = lang.options();
  const auto = items.find((/** @type {any} */ x) => x.auto);
  const rows = items.filter((/** @type {any} */ x) => !x.auto);
  const browser = rows.find((/** @type {any} */ x) => x.name === auto?.name)?.value ?? null;
  const setting = lang.setting;
  let chosen = setting === 'auto' ? browser ?? rows[0]?.value : setting;
  const list = h('div.wl-langs');
  const draw = () => replace(list, rows.map((/** @type {any} */ r) => h(`button.wl-lang${r.value === chosen ? '.is-on' : ''}`, { type: 'button', onclick: () => { chosen = r.value; draw(); } },
    h('span.wl-lang-name', r.name), r.value === browser ? h('span.wl-tag', t('language.system')) : null,
    r.value === chosen ? h('span.wl-check', icon('check')) : null)));
  draw();
  return {
    title: t('language.title'),
    sub: t('language.sub'),
    body: list,
    focus: /** @type {HTMLElement|null} */ (list.querySelector('.wl-lang.is-on')),
    next: {
      ready: () => !!chosen,
      run: async () => {
        const keep = chosen === setting || (setting === 'auto' && chosen === browser);
        if (!keep) {
          // 界面的字变了要重载页面：先记一笔，重载完接着选模型
          g.ctx.storage.set(STEP_KEY, 'model');
          const got = await lang.set(chosen);
          if (got?.reload) return null;
          g.ctx.storage.set(STEP_KEY, null);
        }
        g.go('model', 1);
        return null;
      },
    },
  };
}

/**
 * 完成（第 10 条）：一行写用的模型、人格、预设，「开始聊天」。
 * @param {import('./guide.js').Guide} g
 */
export function doneScreen(g) {
  const t = g.t;
  const r = g.result;
  const model = r.model ? r.model.model || r.model.name : '';
  const parts = [model ? t('done.model', { name: model }) : null, r.persona ? t('done.persona', { name: r.persona }) : null, r.preset ? t('done.preset', { name: r.preset }) : null].filter(Boolean);
  return {
    title: t('done.title'),
    sub: parts.join(' · '),
    center: true,
    body: h('span'),
    next: {
      label: t('done.start'),
      ready: () => true,
      run: async () => {
        try {
          g.ctx.mascot?.hop?.();
          return await g.finish();
        } catch (err) {
          return t('failed', { reason: /** @type {any} */ (err)?.message ?? String(err) });
        }
      },
    },
  };
}
