// @ts-check
//! 人格的外观那几行（蓝图 `web.md`「主题」的「跟着人格的外观」第 3、4 条；核心 P-5 头像、P-6 主题色和背景图）：人格的编辑卡片里最上面
//! 三行，换、删都当场存（只带这一样的 `persona.set`），不等「保存」。
//! - 头像：圆的图（没有的画名字的第一个字），「更换头像」「移除头像」。
//! - 主题色：自选了的画一块颜色、写 `#rrggbb`，「更换颜色」「跟随头像」；没选的写「跟随头像」，「自选颜色」。颜色用浏览器的取色器。
//! - 背景图：一张小图（没有的空一块底色），「更换背景图」「移除背景图」；大的先缩小、存成 WebP（存不了的 JPEG，照片存 PNG 太大）。

import { h, replace } from '../../src/lib/dom.js';
import { shrink, store } from './avatars.js';

/** 核心收背景图的上限（P-6：`blob.put` 收图就封在 5 MiB） */
const BACKGROUND_BYTES = 5 * 1024 * 1024;

export class Appearance {
  /**
   * @param {any} ctx @param {import('./form.js').Kit} kit @param {import('./catalog.js').Catalog} catalog @param {string} id
   * @param {{saved: () => void, say: (text: string) => void, name: () => string}} hooks 存了（列表重读）、写一句（出错）、现在的名字（没头像时画第一个字）
   */
  constructor(ctx, kit, catalog, id, hooks) {
    this.ctx = ctx;
    this.kit = kit;
    this.catalog = catalog;
    this.id = id;
    this.hooks = hooks;
    this.t = (/** @type {string} */ key) => ctx.text(key);
    /** `persona.get` 的那三格：头像、背景图的版本，自选的主题色；没有的是 `null` */
    this.avatar = /** @type {string|null} */ (null);
    this.background = /** @type {string|null} */ (null);
    this.seed = /** @type {string|null} */ (null);
    // 三行各一格（换了、读到了只重画它，不动写了一半的字）
    this.avatarBox = h('div.setup-avatar-box');
    this.seedBox = h('div.setup-seed-box');
    this.backgroundBox = h('div.setup-bg-box');
  }

  /** 照 `persona.get` 的回应记下。 @param {any} got */
  set(got) {
    this.avatar = got?.avatar ?? null;
    this.background = got?.background ?? null;
    this.seed = got?.seed ?? null;
  }

  /** 三行都画一遍。 */
  paint() {
    this.paintAvatar();
    this.paintSeed();
    this.paintBackground();
  }

  paintAvatar() {
    const t = this.t;
    const url = this.imageUrl(this.catalog.avatars, this.avatar, () => this.paintAvatar(), this.avatarBox);
    const pick = this.filePicker((file) => this.changeImage('avatar', file));
    replace(this.avatarBox,
      h('span.setup-face.is-persona.is-big', { 'aria-hidden': 'true' }, url ? h('img', { src: url, alt: '' }) : [...this.hooks.name()][0] ?? ''),
      this.kit.button(t('edit.avatar_change'), {}, () => pick.click()),
      this.avatar ? this.kit.button(t('edit.avatar_remove'), {}, () => this.setPart('avatar', { unset: true })) : null,
      pick);
  }

  paintSeed() {
    const t = this.t;
    const input = /** @type {HTMLInputElement} */ (h('input.setup-color-input', { type: 'color', value: this.seed ?? '#3368c0', tabindex: '-1', 'aria-hidden': 'true' }));
    input.addEventListener('change', () => this.setPart('seed', input.value));
    const open = () => (typeof input.showPicker === 'function' ? input.showPicker() : input.click());
    replace(this.seedBox, this.seed
      ? [h('span.setup-swatch', { style: `background: ${this.seed}`, 'aria-hidden': 'true' }), h('span.setup-seed-hex', this.seed),
        this.kit.button(t('edit.seed_change'), {}, open), this.kit.button(t('edit.seed_follow'), {}, () => this.setPart('seed', { unset: true }))]
      : [h('span.setup-seed-follow', t('edit.seed_follow')), this.kit.button(t('edit.seed_pick'), {}, open)],
    input);
  }

  paintBackground() {
    const t = this.t;
    const url = this.imageUrl(this.catalog.backgrounds, this.background, () => this.paintBackground(), this.backgroundBox);
    const pick = this.filePicker((file) => this.changeImage('background', file));
    replace(this.backgroundBox,
      h('span.setup-bg-thumb', { 'aria-hidden': 'true' }, url ? h('img', { src: url, alt: '' }) : null),
      this.kit.button(t('edit.background_change'), {}, () => pick.click()),
      this.background ? this.kit.button(t('edit.background_remove'), {}, () => this.setPart('background', { unset: true })) : null,
      pick);
  }

  /**
   * 一张图的地址：缓存里没有的去取，取到了重画这一格（这一格不在页面上了就不画）。
   * @param {import('./avatars.js').PersonaImages} images @param {string|null} version @param {() => void} repaint @param {HTMLElement} box
   */
  imageUrl(images, version, repaint, box) {
    const url = images.url(this.id, version);
    if (!url && version) {
      const again = () => {
        this.catalog.listeners.delete(again);
        if (box.isConnected) repaint();
      };
      this.catalog.listeners.add(again);
    }
    return url;
  }

  /** 选图的框（藏着，按钮点它）。 @param {(file: File) => void} onPick */
  filePicker(onPick) {
    const pick = /** @type {HTMLInputElement} */ (h('input', { type: 'file', accept: 'image/png,image/jpeg,image/webp,image/*', hidden: true }));
    pick.addEventListener('change', () => {
      const file = pick.files?.[0];
      pick.value = '';
      if (file) onPick(file);
    });
    return pick;
  }

  /** 换成选的这张：大的先缩小，传成 blob，再交给核心。 @param {'avatar'|'background'} part @param {File} file */
  async changeImage(part, file) {
    const cfg = this.ctx.config;
    try {
      const blob = part === 'avatar'
        ? await shrink(file, cfg.avatar_side)
        : await shrink(file, cfg.background_side, { fallback: 'image/jpeg', maxBytes: BACKGROUND_BYTES });
      const hash = await store(this.ctx.core, blob, { chunk: cfg.avatar_chunk_bytes, tries: cfg.avatar_tries }, part);
      await this.setPart(part, { blob: hash });
    } catch (err) {
      this.hooks.say(/** @type {any} */ (err)?.data?.message ?? /** @type {any} */ (err)?.message ?? String(err));
    }
  }

  /**
   * `persona.set` 只带这一样（头像、背景图 `{blob}` 换、`{unset: true}` 删；主题色 `#rrggbb` 设、`{unset: true}` 回到跟随头像），存了重读、
   * 重画这三行，叫列表、左上角、对话区跟着换。
   * @param {'avatar'|'background'|'seed'} part @param {any} value
   */
  async setPart(part, value) {
    try {
      await this.ctx.core.request('persona.set', { persona: this.id, [part]: value });
      this.set(await this.ctx.core.request('persona.get', { persona: this.id }));
      this.hooks.say('');
      this.paint();
      this.hooks.saved();
    } catch (err) {
      this.hooks.say(/** @type {any} */ (err)?.data?.message ?? /** @type {any} */ (err)?.message ?? String(err));
    }
  }
}
