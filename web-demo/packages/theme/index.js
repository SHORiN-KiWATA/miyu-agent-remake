// @ts-check
//! 主题（软件包 `theme`，蓝图 `web.md`「主题」）：声明 `theme.palettes`，一套一套的颜色由主题包（`theme-morning`、
//! `theme-tokyonight`……）挂进来；照设置项 `palette` 选一套（空着的跟着系统：浅色用第一套浅的，深色用第一套深的），把颜色写成
//! 页面的 CSS 变量：页面那一组是 `--名字`，终端那一组是 `--t-名字`，名字里的 `_` 换成 `-`。
//!
//! 提供服务 `theme`：现在是哪一套、是不是深色、换到下一套（写进个人那一层的设置，`/theme`、左栏的按钮用）。换了发事件
//! `theme.changed`。
//!
//! 声明 `theme.overlays`：叠在选中的那一套上面的几格颜色（跟着人格的主题色，软件包 `theme-persona`，蓝图「跟着人格的外观」）。挂进来的
//! `render(scheme)` 照浅色、深色交 `{page, tui}` 里要盖的那几格，交 `null` 的不盖。头一次以后再换颜色（换了一套、叠的变了），整页的
//! 底色、字色、线色 0.3 秒渐变过去（`style.css` 的 `is-retheming`，过完拿掉）。

const RETHEME_MS = 400;

/** @param {any} ctx */
export function apply(ctx) {
  ctx.slots.declare('theme.palettes', 'list');
  ctx.slots.declare('theme.overlays', 'list');
  const system = matchMedia('(prefers-color-scheme: dark)');
  const root = document.documentElement;
  /** 写上去的变量：换了一套先拿掉原来的，撤回时全拿掉 */
  let written = [];

  const palettes = () => ctx.slots.list('theme.palettes');
  /** 现在用哪一套：设置项写了的（还在的）用它；没写的跟着系统 */
  const chosen = () => {
    const all = palettes();
    const named = all.find((p) => p.id === ctx.config.palette);
    if (named) return named;
    const scheme = system.matches ? 'dark' : 'light';
    return all.find((p) => p.scheme === scheme) ?? all[0] ?? null;
  };
  let painted = false;
  let fading = 0;
  const paint = () => {
    const p = chosen();
    // 头一次以后的换色带过渡
    if (painted && p) {
      root.classList.add('is-retheming');
      clearTimeout(fading);
      fading = window.setTimeout(() => root.classList.remove('is-retheming'), RETHEME_MS);
    }
    for (const name of written) root.style.removeProperty(name);
    written = [];
    if (!p) return;
    painted = true;
    const base = p.render();
    const page = { ...base.page };
    const tui = { ...base.tui };
    for (const o of ctx.slots.list('theme.overlays')) {
      const got = o.render(p.scheme);
      if (!got) continue;
      Object.assign(page, got.page ?? {});
      Object.assign(tui, got.tui ?? {});
    }
    const set = (name, value) => {
      root.style.setProperty(name, value);
      written.push(name);
    };
    for (const [k, v] of Object.entries(page)) set(`--${k.replaceAll('_', '-')}`, v);
    for (const [k, v] of Object.entries(tui)) set(`--t-${k.replaceAll('_', '-')}`, v);
    root.style.colorScheme = p.scheme;
    root.dataset.theme = p.id;
    ctx.emit('theme.changed', p.id);
  };
  ctx.slots.watch('theme.palettes', paint);
  ctx.slots.watch('theme.overlays', paint);
  // 换了一套（设置项 palette 是 live 的）：当场重画，不重来
  ctx.watchConfig(paint);
  ctx.effect(() => {
    system.addEventListener('change', paint);
    return () => system.removeEventListener('change', paint);
  });
  ctx.effect(() => () => {
    clearTimeout(fading);
    root.classList.remove('is-retheming');
    for (const name of written) root.style.removeProperty(name);
    root.style.colorScheme = '';
    delete root.dataset.theme;
  });
  paint();

  ctx.provide('theme', {
    current: () => chosen(),
    dark: () => chosen()?.scheme === 'dark',
    /** 换到下一套，记进个人那一层（这个包的设置项 `palette`），这个包照新的设置重来 */
    next: async () => {
      const all = palettes();
      if (!all.length) return null;
      const now = chosen();
      const after = all[(all.indexOf(now) + 1) % all.length];
      await ctx.packages.set('theme', { config: { palette: after.id } });
      return after;
    },
  });
}
