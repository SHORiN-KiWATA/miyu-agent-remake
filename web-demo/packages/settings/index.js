// @ts-check
//! 设置页（软件包 `settings`，蓝图 `web.md`「设置页」、`web/architecture.md`「设置页」）：页面正中的弹窗，核心的配置和网页自己的
//! 设置都在这里改。打开：左栏底下的齿轮、`/settings`、`/config`（直接到「模型」）、`/pkg`（直接到「软件包」）、`Ctrl+,`（macOS `⌘,`）。
//! 声明 `settings.section`（多一页：桌面端的设置以后挂这里；人格那一页由软件包 `setup` 挂）、`settings.editor`（keyed，照配置项的键：
//! `{options(): [{value, name}], hidden(): boolean}`，给这一项下拉的选项、没得选时藏了这一行；默认人格由软件包 `setup` 给）。

import { h, icon } from '../../src/lib/dom.js';
import { SettingsDialog } from './dialog.js';

/** @param {any} ctx */
export function apply(ctx) {
  ctx.slots.declare('settings.section', 'list');
  ctx.slots.declare('settings.editor', 'keyed');
  /** @type {SettingsDialog|null} */
  let dialog = null;
  const open = (page) => {
    if (dialog?.isOpen) {
      if (page) dialog.show(page);
      return;
    }
    dialog = new SettingsDialog(ctx);
    dialog.open(page);
  };
  ctx.effect(() => () => dialog?.close());

  ctx.commands.register({ name: 'settings', summary: ctx.text('command_settings') }, () => open(null));
  ctx.commands.register({ name: 'config', summary: ctx.text('command_config') }, () => open('models'));
  ctx.commands.register({ name: 'pkg', summary: ctx.text('command_pkg') }, () => open('packages'));

  ctx.slots.mount('sidebar.actions', {
    id: 'settings',
    order: 10,
    render: () => h('button.icon-button', { type: 'button', title: ctx.text('open'), 'aria-label': ctx.text('open'), onclick: () => open(null) }, icon('settings')),
  });

  // `Ctrl+,`（macOS `⌘,`）：照系统软件打开偏好设置的习惯；浏览器不占这个键
  const onKey = (/** @type {KeyboardEvent} */ e) => {
    if (e.key !== ',' || e.altKey || e.shiftKey) return;
    const mac = /Mac|iPhone|iPad/.test(navigator.platform);
    if (mac ? !e.metaKey : !e.ctrlKey) return;
    e.preventDefault();
    open(null);
  };
  ctx.effect(() => {
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  });
}
