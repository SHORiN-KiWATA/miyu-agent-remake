// @ts-check
//! 人格、预设、工作区（软件包 `setup`，蓝图 `web.md`「人格、预设、工作区」）：
//! - 空会话：输入框正上方「选择人格」「选择预设」「设置工作区」三个按钮（挂载位 `composer.above`），选了以后按钮上的字换成选中的；没选的照默认
//!   的用，开会话时由整页带上（服务 `chat` 的 `draft`、`setDraft`）。默认的预设、（照预设算出来的）默认人格用不了，锁住输入框，选了才能打字。
//! - 开着的会话：对话区左上角一小条（挂载位 `stage.info`）：人格名 · 预设名 · 工作目录，点目录那一截换工作区。
//! - `/workspace [路径]`：换这个会话在哪干活（服务 `chat` 的 `setWorkdir`，核心 9-7 的 `session.set_workspace`）；不带路径的开工作区的菜单。
//! - 设置页：「人格」「预设」两页（`settings.section`），通用页的默认人格、默认预设给下拉的选项（`settings.editor`）。

import { h, icon, replace } from '../../src/lib/dom.js';
import { Menu } from './menu.js';
import { personaName, presetName, defaultUsable, presetInUse, personaInUse, dirName, readPath, remember, tilde } from './model.js';
import { Catalog } from './catalog.js';
import { ListPage, problemText } from './page.js';
import { browse } from './folders.js';

const RECENT = 'setup.recent';

/** @param {any} ctx */
export function apply(ctx) {
  const t = (key, fields) => ctx.text(key, fields);
  const chat = ctx.chat;
  /** 有哪些人格、预设，默认是哪个；读不到（旧核心）的那一样的按钮不出 */
  const catalog = new Catalog(ctx.core);

  const menu = new Menu();
  const personaBtn = h('button.setup-btn', { type: 'button', onclick: () => openPersonas() });
  const presetBtn = h('button.setup-btn', { type: 'button', onclick: () => openPresets() });
  const workBtn = h('button.setup-btn', { type: 'button', onclick: () => openWorkspaces(workBtn, false) });
  const row = h('div.setup-row', { hidden: true }, personaBtn, presetBtn, workBtn, menu.el);
  const infoMenu = new Menu();
  const info = h('div.setup-info', { hidden: true });
  const infoWrap = h('div.setup-info-wrap', info, infoMenu.el);

  const home = () => chat.home() ?? null;
  const recent = () => /** @type {string[]} */ (ctx.storage.get(RECENT, []));
  const nameOf = (id) => {
    const p = catalog.personas?.find((x) => x.persona === id);
    return p ? personaName(p) : id;
  };
  const presetNameOf = (id) => {
    const p = catalog.presets?.find((x) => x.preset === id);
    return p ? presetName(p) : id;
  };
  /** 空会话里实际会用的预设、人格（没选的照默认算；预设不再带默认人格，2026-10-08 项目主人） */
  const inUse = () => {
    const draft = chat.draft();
    return { preset: presetInUse(draft.preset, catalog.presetDefault), persona: personaInUse(draft.persona, catalog.personaDefault, catalog.personas ?? []) };
  };

  /** 读人格、预设的列表和默认的（进空会话时读一次，读完重画）。 */
  const load = async () => {
    await catalog.load();
    draw(true);
  };

  /** 照现在的会话画：空会话画按钮、管锁；开着的会话画左上角那一条。没变的不动（对话区每画一次都会来）。 */
  let drawn = '';
  const draw = (force = false) => {
    const session = chat.current();
    const draft = chat.draft();
    const cwd = chat.workdir();
    const used = session ? null : inUse();
    const sig = JSON.stringify([session, draft, cwd, catalog.personas?.length ?? -1, catalog.presets?.length ?? -1, catalog.personaDefault, catalog.presetDefault,
      used, session ? [chat.persona(session), chat.preset(session)] : null]);
    if (!force && sig === drawn) return;
    drawn = sig;
    if (!session) {
      info.hidden = true;
      infoMenu.close();
      row.hidden = false;
      personaBtn.hidden = !catalog.personas;
      replace(personaBtn, icon('user-round'), h('span', draft.persona === false ? t('no_persona') : draft.persona ? nameOf(draft.persona) : t('choose_persona')));
      personaBtn.classList.toggle('is-set', draft.persona != null);
      presetBtn.hidden = !catalog.presets;
      replace(presetBtn, icon('toggle-right'), h('span', draft.preset ? presetNameOf(draft.preset) : t('choose_preset')));
      presetBtn.classList.toggle('is-set', !!draft.preset);
      // 选了默认工作区的写「默认工作区」：它的目录名是数据目录里的 `workspace`，写出来像没翻译的字（2026-10-08 项目主人）
      const workLabel = !draft.cwd ? t('set_workspace') : draft.cwd === chat.defaultWorkdir() ? t('default_workspace') : dirName(tilde(draft.cwd, home()));
      replace(workBtn, icon('folder'), h('span', workLabel));
      workBtn.classList.toggle('is-set', !!draft.cwd);
      workBtn.title = tilde(draft.cwd ?? chat.defaultWorkdir(), home());
      // 没选、默认的又用不了：锁住输入框（第 2 条）。预设一定要有（Y12）；人格可以没有，只有算出来的默认人格文件写错了才锁
      const needPreset = !!catalog.presets && !draft.preset && !defaultUsable(catalog.presets, used?.preset ?? null, 'preset');
      const needPersona = !!catalog.personas && draft.persona == null && !!used?.persona && !defaultUsable(catalog.personas, used.persona);
      presetBtn.classList.toggle('is-need', needPreset);
      personaBtn.classList.toggle('is-need', needPersona);
      ctx.composer.lock(needPreset ? t('locked_preset') : needPersona ? t('locked') : null);
      return;
    }
    row.hidden = true;
    menu.close();
    ctx.composer.lock(null);
    const persona = chat.persona(session);
    const preset = chat.preset(session);
    const path = tilde(cwd, home());
    const sep = () => h('span.setup-info-sep', '·');
    replace(info,
      persona ? [h('span.setup-info-part', icon('user-round'), h('b', nameOf(persona))), sep()] : null,
      preset ? [h('span.setup-info-part', icon('toggle-right'), h('b', presetNameOf(preset))), sep()] : null,
      h('button.setup-info-part.is-path', { type: 'button', title: path, onclick: () => openWorkspaces(info, true) }, icon('folder'), h('span', path)));
    info.hidden = false;
  };

  const openPersonas = () => {
    if (!catalog.personas) return;
    const chosen = inUse().persona;
    menu.show(personaBtn, {
      title: t('choose_persona'),
      rows: [
        // 第一行「无人格」：不带人格（2026-10-08 项目主人：人格可以留空，这一项叫「无人格」；没有人格的会话记忆不生效，知识库照样能查）
        { title: t('no_persona'), desc: t('no_persona_desc'), current: chosen == null, pick: () => chat.setDraft({ persona: false }) },
        ...catalog.personas.map((p) => ({
        title: personaName(p),
        desc: p.problem ? t('persona_bad') : p.summary ?? '',
        tip: p.problem ? catalog.problemOf('persona', p).map((x) => problemText(ctx, x)).join('\n') : undefined,
        current: p.persona === chosen && !p.problem,
        off: !!p.problem,
        pick: () => chat.setDraft({ persona: p.persona }),
      })),
      ],
    });
  };

  /** 预设的菜单：照 `preset.list`，一行一个名字（预设不写说明）；写错的暗着。 */
  const openPresets = () => {
    if (!catalog.presets) return;
    const chosen = inUse().preset;
    menu.show(presetBtn, {
      title: t('choose_preset'),
      rows: catalog.presets.map((p) => ({
        title: presetName(p),
        desc: p.problem ? t('preset_bad') : '',
        tip: p.problem ? catalog.problemOf('preset', p).map((x) => problemText(ctx, x)).join('\n') : undefined,
        current: p.preset === chosen && !p.problem,
        off: !!p.problem,
        pick: () => chat.setDraft({ preset: p.preset }),
      })),
    });
  };

  /** 核心拒了换目录的原因写给人看的一句（`fs.list`、`session.set_workspace` 同一套原因码）。 @param {any} err */
  const refused = (err) => err?.reason === 'path_forbidden' ? t('forbidden') : err?.reason === 'not_a_directory' ? t('not_a_dir')
    : err?.reason === 'path_unreadable' ? t('not_dir') : err?.message ?? String(err);

  /**
   * 换到这个目录：空会话改选的工作区（选了默认的也记着，按钮上换成它的目录名，2026-10-08 项目主人；开会话时核心才判，这里先问
   * `fs.list` 能不能用）；开着的会话交给核心换
   * （`session.set_workspace`，核心 9-7：会话记着、别的头跟着换，不在、不是目录、在数据根里的当场拒）。成了记进最近用过的。
   * 交回一句错，成了是 `null`。
   * @param {string} path
   */
  const use = async (path) => {
    const session = chat.current();
    const def = chat.defaultWorkdir();
    try {
      if (!session) {
        await ctx.core.request('fs.list', { cwd: path, dir: '' });
        chat.setDraft({ cwd: path });
      } else {
        // 换成了的正文里多一行「工作区：路径」（照核心推来的 `session.workspace_changed`），这里不再另提示；太宽退回了工作区的，
        // `workdir.adjusted` 那边提示
        await chat.setWorkdir(session, path);
      }
    } catch (err) {
      return refused(err);
    }
    ctx.storage.set(RECENT, remember(recent(), path, def, 5));
    return null;
  };
  /** 菜单里点一项：错了写进提示。 @param {string} path */
  const pickDir = (path) => use(path).then((why) => { if (why) ctx.composer.say(why); });

  /** `/workspace 路径` 写的路径：先认写法，再换；交回一句错，成了是 `null`。 @param {string} text */
  const check = async (text) => {
    const path = readPath(text);
    return path ? use(path) : t('relative');
  };

  /** 工作区的菜单：默认工作区、最近用过的、「选择文件夹…」（2026-10-08 项目主人：不要手写路径的框；要写路径用 `/workspace 路径`）。 @param {HTMLElement} anchor @param {boolean} below */
  const openWorkspaces = (anchor, below) => {
    const def = chat.defaultWorkdir();
    const now = chat.current() ? chat.workdir() : chat.draft().cwd ?? def;
    const others = recent().filter((p) => p !== def);
    const which = below ? infoMenu : menu;
    which.show(anchor, {
      title: t('set_workspace'),
      below,
      rows: [
        { title: t('default_workspace'), desc: tilde(def, home()), current: now === def, pick: () => pickDir(def) },
        ...(others.length ? [{ section: t('recent') }] : []),
        ...others.map((p) => ({ title: dirName(tilde(p, home())), desc: tilde(p, home()), current: now === p, pick: () => pickDir(p) })),
        { title: t('folders.open'), icon: 'folder-open', pick: () => browse(ctx, which, anchor, below, now, pickDir, (p) => tilde(p, home())) },
      ],
    });
  };

  ctx.slots.mount('composer.above', { id: 'setup', order: 90, render: () => row });
  // 设置页：「人格」「预设」两页；通用页的「默认人格」「默认预设」照列表给下拉的选项（只有一个也列，2026-10-07 项目主人）
  const personaPage = new ListPage(ctx, catalog, 'persona');
  const presetPage = new ListPage(ctx, catalog, 'preset');
  ctx.slots.mount('settings.section', { id: 'personas', name: t('page.title'), render: (kit) => personaPage.render(kit) });
  ctx.slots.mount('settings.section', { id: 'presets', name: t('presets.title'), render: (kit) => presetPage.render(kit) });
  ctx.slots.mount('settings.editor', {
    id: 'setup-persona',
    key: 'persona.default',
    // 「无人格」：新会话默认不带人格（写成删掉这一项）
    options: () => [{ value: null, name: t('default_none') }, ...(catalog.personas ?? []).filter((p) => !p.problem).map((p) => ({ value: p.persona, name: personaName(p) }))],
  });
  ctx.slots.mount('settings.editor', {
    id: 'setup-preset',
    key: 'preset.default',
    options: () => (catalog.presets ?? []).filter((p) => !p.problem).map((p) => ({ value: p.preset, name: presetName(p) })),
  });
  ctx.slots.mount('stage.info', { id: 'setup', order: 10, render: () => infoWrap });
  ctx.on('view.changed', () => draw());
  ctx.on('draft.changed', () => draw());
  ctx.on('session.opened', (id) => { if (id === null) load(); else draw(); });
  ctx.on('session.created', () => draw());
  // 核心照「工作目录太宽」退回了工作区：照实际的提示一句，用不了的那个不留在最近用过里
  ctx.on('workdir.adjusted', ({ asked, cwd }) => {
    ctx.storage.set(RECENT, recent().filter((p) => p !== asked));
    ctx.composer.say(t('workspace_adjusted', { path: tilde(cwd, home()) }));
    draw(true);
  });
  ctx.commands.register({ name: 'workspace', summary: t('command'), args: true }, async (words) => {
    const text = (words ?? '').trim();
    if (!text) return openWorkspaces(chat.current() ? info : workBtn, !!chat.current());
    const why = await check(text);
    if (why) ctx.composer.say(why);
  });
  ctx.effect(() => () => {
    menu.close();
    infoMenu.close();
    ctx.composer.lock(null);
  });
  load();
}
