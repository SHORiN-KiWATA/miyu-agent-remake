// @ts-check
//! 一套颜色（软件包 `theme-tokyonight`，蓝图 `web.md`「主题」）：挂进主题包声明的 `theme.palettes`。颜色都是这个包的设置项（清单里的出厂值
//! 是原来的 `resources/themes/tokyonight.json`），都是 live 的，能单改、当场生效：名字 `t_` 打头的是终端那一组（`--t-名字`），别的是
//! 页面那一组（`--名字`）。

/** @param {any} ctx */
export function apply(ctx) {
  /** 照现在的设置项分成两组 */
  const split = () => {
    const page = {};
    const tui = {};
    for (const [k, v] of Object.entries(ctx.config)) {
      if (k.startsWith('t_')) tui[k.slice(2)] = v;
      else page[k] = v;
    }
    return { page, tui };
  };
  const mount = () => ctx.slots.mount('theme.palettes', { id: 'tokyonight', name: ctx.text('name'), scheme: 'dark', render: split });
  let undo = mount();
  // 单改了一个颜色：换一份挂上，主题包当场重画
  ctx.watchConfig(() => {
    undo();
    undo = mount();
  });
}
