// @ts-check
//! 选文件夹（蓝图 `web.md`「人格、预设、工作区」第 4 条）：先照宿主的 `files.pickDir()` 开系统的选目录对话框（2026-10-08 项目主人：
//! 选择文件夹应该打开目录选择器；浏览器经桥开，桌面端自己开），点了取消就算了；开不了的（交 `null`：页面开在别的机器上、那台机器上
//! 没有对话框程序）在菜单里画一个文件夹浏览器，列核心那台机器上的目录（`fs.list`，路径都照核心给的，
//! 哪个系统都一样）：顶上是现在的目录，一行一个子目录，点进去；「上一级」照 `fs.realpath` 的 `..`；「选这个目录」交出去。点开的点
//! 文件夹不收起菜单。以点开头的隐藏目录不列（要用的直接写路径）。

import { h } from '../../src/lib/dom.js';

/**
 * 在菜单里浏览文件夹，从 `start` 开始。
 * @param {any} ctx
 * @param {import('./menu.js').Menu} menu
 * @param {HTMLElement} anchor
 * @param {boolean} below
 * @param {string} start 从哪个目录开始
 * @param {(path: string) => void} done 选定了
 * @param {(path: string) => string} show 路径写给人看的样子（家目录写 `~`）
 */
export async function browse(ctx, menu, anchor, below, start, done, show) {
  const t = (key, fields) => ctx.text(key, fields);
  const button = (label, primary, onclick, off = false) => h(`button.setup-menu-btn${primary ? '.is-primary' : ''}`, { type: 'button', onclick, disabled: off }, label);
  const go = async (dir) => {
    let items = [];
    let note = '';
    let bad = false;
    try {
      const got = await ctx.core.request('fs.list', { cwd: dir, dir: '' });
      items = (got?.items ?? []).filter((x) => x.dir && !x.path.startsWith('.'));
      if (got?.partial) note = t('folders.partial');
    } catch (err) {
      note = err?.reason === 'path_forbidden' ? t('forbidden') : t('not_dir');
      bad = true;
    }
    const parent = await ctx.core.request('fs.realpath', { path: '..', cwd: dir }).then((r) => r?.path ?? null, () => null);
    const up = parent && parent !== dir ? () => go(parent) : undefined;
    const name = (x) => x.path.replace(/[\\/]+$/, '');
    menu.show(anchor, {
      title: show(dir),
      below,
      back: up,
      note: note || (items.length ? '' : t('folders.empty')),
      rows: [
        ...(up ? [{ title: t('folders.up'), icon: 'arrow-up', pick: up }] : []),
        ...items.sort((a, b) => name(a).localeCompare(name(b))).map((x) => ({ title: name(x), icon: 'folder', pick: () => go(x.full) })),
      ],
      foot: [button(t('folders.cancel'), false, () => menu.close()), button(t('folders.choose'), true, () => { menu.close(); done(dir); }, bad)],
    });
  };
  // 系统的选目录对话框：选了交出去，取消了（`false`）就算了，开不了（`null`）才画浏览器
  const picked = await ctx.host?.files?.pickDir?.({ title: t('folders.title'), start }).catch(() => null);
  if (typeof picked === 'string') return done(picked);
  if (picked === false) return;
  await go(start);
}
