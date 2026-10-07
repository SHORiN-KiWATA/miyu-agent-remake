// @ts-check
//! 人格、工作区（软件包 `setup`，蓝图 `web.md`「人格、预设、工作区」）：
//! - 空会话：输入框正上方「选择人格」「设置工作区」两个按钮（挂载位 `composer.above`），选了以后按钮上的字换成选中的；没选的照默认的用，
//!   开会话时由整页带上（服务 `chat` 的 `draft`、`setDraft`）。默认的人格没了、写错了，锁住输入框，选了才能打字。
//! - 开着的会话：对话区左上角一小条（挂载位 `stage.info`）：人格名（核心 P-1 下以后才有）· 工作目录，点目录那一截换工作区。
//! - `/workspace [路径]`：换这个会话以后在哪干活（服务 `chat` 的 `setWorkdir`）；不带路径的开工作区的菜单。

import { h, icon, replace } from '../../src/lib/dom.js';
import { Menu } from './menu.js';
import { personaName, defaultUsable, dirName, readPath, remember, tilde } from './model.js';
import { personaPage } from './page.js';

const RECENT = 'setup.recent';

/** @param {any} ctx */
export function apply(ctx) {
  const t = (key, fields) => ctx.text(key, fields);
  const chat = ctx.chat;
  /** 核心的人格列表、配置项 `persona.default`；读不到（旧核心）的是 `null`，人格那个按钮不出 */
  let personas = /** @type {import('./model.js').Persona[]|null} */ (null);
  let fallback = /** @type {string|null} */ (null);

  const menu = new Menu();
  const personaBtn = h('button.setup-btn', { type: 'button', onclick: () => openPersonas() });
  const workBtn = h('button.setup-btn', { type: 'button', onclick: () => openWorkspaces(workBtn, false) });
  const row = h('div.setup-row', { hidden: true }, personaBtn, workBtn, menu.el);
  const infoMenu = new Menu();
  const info = h('div.setup-info', { hidden: true });
  const infoWrap = h('div.setup-info-wrap', info, infoMenu.el);

  const home = () => chat.home() ?? null;
  const recent = () => /** @type {string[]} */ (ctx.storage.get(RECENT, []));
  const nameOf = (id) => {
    const p = personas?.find((x) => x.persona === id);
    return p ? personaName(p) : id;
  };

  /** 读人格列表和默认的人格（进空会话时读一次，读完重画）。 */
  const load = async () => {
    try {
      const [list, got] = await Promise.all([ctx.core.request('persona.list', {}), ctx.core.request('config.get', { keys: ['persona.default'] })]);
      personas = list?.personas ?? [];
      fallback = got?.items?.['persona.default']?.value ?? null;
    } catch (err) {
      console.error(`读不到人格：${err?.message ?? err}`);
      personas = null;
    }
    draw(true);
  };

  /** 照现在的会话画：空会话画按钮、管锁；开着的会话画左上角那一条。没变的不动（对话区每画一次都会来）。 */
  let drawn = '';
  const draw = (force = false) => {
    const session = chat.current();
    const draft = chat.draft();
    const cwd = chat.workdir();
    const sig = JSON.stringify([session, draft, cwd, personas?.length ?? -1, fallback, session ? chat.persona(session) : null]);
    if (!force && sig === drawn) return;
    drawn = sig;
    if (!session) {
      info.hidden = true;
      infoMenu.close();
      row.hidden = false;
      personaBtn.hidden = !personas;
      replace(personaBtn, icon('user-round'), h('span', draft.persona ? nameOf(draft.persona) : t('choose_persona')));
      personaBtn.classList.toggle('is-set', !!draft.persona);
      replace(workBtn, icon('folder'), h('span', draft.cwd ? dirName(tilde(draft.cwd, home())) : t('set_workspace')));
      workBtn.classList.toggle('is-set', !!draft.cwd);
      workBtn.title = tilde(draft.cwd ?? chat.defaultWorkdir(), home());
      // 没选、默认的又用不了：锁住输入框（第 2 条）
      const locked = !!personas && !draft.persona && !defaultUsable(personas, fallback);
      personaBtn.classList.toggle('is-need', locked);
      ctx.composer.lock(locked ? t('locked') : null);
      return;
    }
    row.hidden = true;
    menu.close();
    ctx.composer.lock(null);
    const persona = chat.persona(session);
    const path = tilde(cwd, home());
    replace(info,
      persona ? [h('span.setup-info-part', icon('user-round'), h('b', nameOf(persona))), h('span.setup-info-sep', '·')] : null,
      h('button.setup-info-part.is-path', { type: 'button', title: path, onclick: () => openWorkspaces(info, true) }, icon('folder'), h('span', path)));
    info.hidden = false;
  };

  const openPersonas = () => {
    if (!personas) return;
    const chosen = chat.draft().persona ?? fallback;
    menu.show(personaBtn, personas.map((p) => ({
      title: personaName(p),
      desc: p.problem ? t('persona_bad') : p.summary || (p.name ? p.persona : ''),
      tip: p.problem ?? undefined,
      current: p.persona === chosen && !p.problem,
      off: !!p.problem,
      pick: () => chat.setDraft({ persona: p.persona }),
    })));
  };

  /** 换到这个目录：空会话改选的工作区（默认的记成没选），开着的会话之后每句话带上；记进最近用过的。 @param {string} path */
  const use = (path) => {
    const session = chat.current();
    const def = chat.defaultWorkdir();
    if (!session) chat.setDraft({ cwd: path === def ? null : path });
    else {
      chat.setWorkdir(session, path);
      ctx.composer.say(t('workspace_set', { path: tilde(path, home()) }), true);
    }
    ctx.storage.set(RECENT, remember(recent(), path, def, 5));
  };

  /** 先问核心这个目录能不能用（`fs.list`：不在、不是目录、落在数据根里的拒掉）；交回一句错，能用的是 `null`。 @param {string} text */
  const check = async (text) => {
    const path = readPath(text);
    if (!path) return t('relative');
    try {
      await ctx.core.request('fs.list', { cwd: path, dir: '' });
    } catch (err) {
      return err?.reason === 'path_forbidden' ? t('forbidden') : t('not_dir');
    }
    use(path);
    return null;
  };

  /** 工作区的菜单：默认工作区、最近用过的、最下面写路径。 @param {HTMLElement} anchor @param {boolean} below */
  const openWorkspaces = (anchor, below) => {
    const def = chat.defaultWorkdir();
    const now = chat.current() ? chat.workdir() : chat.draft().cwd ?? def;
    const others = recent().filter((p) => p !== def);
    (below ? infoMenu : menu).show(anchor, [
      { title: t('default_workspace'), desc: tilde(def, home()), current: now === def, pick: () => use(def) },
      ...(others.length ? [{ section: t('recent') }] : []),
      ...others.map((p) => ({ title: dirName(tilde(p, home())), desc: tilde(p, home()), current: now === p, pick: () => use(p) })),
    ], { hint: t('path_hint'), submit: check }, below);
  };

  ctx.slots.mount('composer.above', { id: 'setup', order: 90, render: () => row });
  // 设置页：「人格」一页；通用页的「默认人格」照人格列表给下拉的选项，只有一个人格时不列（没得选）
  ctx.slots.mount('settings.section', { id: 'personas', name: t('page.title'), render: () => personaPage(ctx, () => fallback) });
  ctx.slots.mount('settings.editor', {
    id: 'setup',
    key: 'persona.default',
    options: () => (personas ?? []).filter((p) => !p.problem).map((p) => ({ value: p.persona, name: personaName(p) })),
    hidden: () => !!personas && personas.filter((p) => !p.problem).length <= 1,
  });
  ctx.slots.mount('stage.info', { id: 'setup', order: 10, render: () => infoWrap });
  ctx.on('view.changed', () => draw());
  ctx.on('draft.changed', () => draw());
  ctx.on('workdir.changed', () => draw());
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
