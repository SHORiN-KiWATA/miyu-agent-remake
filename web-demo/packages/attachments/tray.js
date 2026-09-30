// @ts-check
//! 框里那一排附件（蓝图 `web.md`「附件」第 3 条，照 Claude 网页端）：挂进写字的地方上面（`composer.head`），一个附件一张卡，每张
//! 一样大（112×104）。文件写名字（最多两行）、下面一行小字（文字文件写几行，别的写大小）；图片、视频铺满这张卡（视频取第一帧，
//! 正中一个播放记号）；左下角一个扩展名的小标签。只排一行，放不下的往右排出去、横着滚，两头有东西没露全的那一边虚化；不折行、
//! 不叠。传的时候罩一层、转一个小圈；悬停露 ×。进来缩放淡入，拿掉缩回去淡出、宽度收掉，后面的滑过来补上（「动效」）。
//! 缩略图的地址由宿主给（浏览器是文件内容的临时地址，桌面端是路径换成的），跟着那一张：拿走时松开。

import { h, icon } from '../../src/lib/dom.js';
import { leave } from '../../src/lib/motion.js';
import { bytes } from '../../src/lib/format.js';
import { kindOf, extOf, lineCount } from './model.js';

export class TrayView {
  /**
   * @param {import('./model.js').Tray} tray
   * @param {(path: string, fields?: any) => string} t 这个包的字
   * @param {(id: number) => void} remove 点了 × 拿掉
   * @param {{preview: (file: any) => {url: string, release: () => void}|null, text: (file: any, max: number) => Promise<string|null>}} files
   *   宿主的文件能力：缩略图的地址、读文字文件
   * @param {{count_lines_max: number}} config
   */
  constructor(tray, t, remove, files, config) {
    this.tray = tray;
    this.t = t;
    this.remove = remove;
    this.files = files;
    this.config = config;
    this.strip = h('div.attach-strip');
    // 那一排出来、收起：高度从 0 长出来、收回去（外面一层放高度，里面一层裁掉，间距在最里面，收着的连间距一起收掉）
    this.el = h('div.attach-tray', h('div.attach-clip', h('div.attach-pad', this.strip)));
    /** 画着的几张：附件的编号 → 节点、缩略图 */
    /** @type {Map<number, {el: HTMLElement, shown: {url: string, release: () => void}|null, state: string}>} */
    this.chips = new Map();
    this.stop = tray.watch(() => this.draw());
    this.strip.addEventListener('scroll', () => this.edges(), { passive: true });
    // 竖着的滚轮也横着滚（照 Claude 网页端：没有横滚轮的鼠标也滚得动）
    this.strip.addEventListener('wheel', (e) => {
      if (Math.abs(e.deltaY) <= Math.abs(e.deltaX) || this.strip.scrollWidth <= this.strip.clientWidth) return;
      e.preventDefault();
      this.strip.scrollLeft += e.deltaY;
    }, { passive: false });
    new ResizeObserver(() => this.edges()).observe(this.strip);
    this.draw();
  }

  /** 照 `Tray` 对一遍：没了的原地退场（宽度收掉，后面的滑过来），新来的排在后面，状态变了的换样子。 */
  draw() {
    const alive = new Set(this.tray.items.map((it) => it.id));
    for (const [id, chip] of [...this.chips]) {
      if (alive.has(id)) continue;
      this.chips.delete(id);
      leave(chip.el, () => {
        chip.el.remove();
        chip.shown?.release();
        this.edges();
      });
    }
    for (const it of this.tray.items) {
      let chip = this.chips.get(it.id);
      if (!chip) {
        chip = this.chip(it);
        this.chips.set(it.id, chip);
        this.strip.append(chip.el);
        // 新来的排在最后：滚到头，露出来（不用按那一张对齐：整页放大以后对齐会差几个小数像素，右边一直虚着）
        requestAnimationFrame(() => this.strip.scrollTo({ left: this.strip.scrollWidth, behavior: 'smooth' }));
      }
      if (chip.state !== it.state) {
        chip.state = it.state;
        chip.el.classList.toggle('is-uploading', it.state === 'uploading');
      }
    }
    this.el.classList.toggle('has-items', this.tray.items.length > 0);
    this.edges();
  }

  /** 两头有东西没露全的那一边虚化。 */
  edges() {
    const s = this.strip;
    const slack = 1;
    this.el.classList.toggle('fade-left', s.scrollLeft > slack);
    this.el.classList.toggle('fade-right', s.scrollLeft + s.clientWidth < s.scrollWidth - slack);
  }

  /**
   * 一张卡：图片、视频是缩略图，别的写名字和一行小字；左下角扩展名。视频取不出第一帧的照文件画。
   * @param {import('./model.js').Item} it
   */
  chip(it) {
    const t = this.t;
    const kind = kindOf(it.file);
    const shown = kind === 'image' || kind === 'video' ? this.files.preview(it.file) : null;
    const ext = extOf(it.name);
    const badge = () => (ext ? h('span.attach-ext', ext) : null);
    const x = h('button.attach-remove', { type: 'button', title: t('remove'), 'aria-label': t('remove'), onclick: () => this.remove(it.id) }, icon('x'));
    const busy = h('span.attach-busy', { title: t('uploading') }, icon('loader-circle'));
    const card = () => {
      const sub = h('span.attach-sub', bytes(it.size));
      if (kind === 'text') {
        this.files.text(it.file, this.config.count_lines_max).then((text) => {
          if (text != null) sub.textContent = t('lines', { count: lineCount(text) });
        });
      }
      return h('div.attach-chip.is-file', { title: it.name }, h('span.attach-name', it.name), sub, badge(), busy, x);
    };
    let el;
    if (shown && kind === 'video') {
      const video = h('video', { src: `${shown.url}#t=0.1`, muted: true, preload: 'metadata', playsinline: true });
      el = h('div.attach-chip.is-media.is-video', { title: it.name }, video, h('span.attach-play', icon('play')), badge(), busy, x);
      video.addEventListener('error', () => {
        const plain = card();
        plain.classList.toggle('is-uploading', el.classList.contains('is-uploading'));
        el.replaceWith(plain);
        const chip = this.chips.get(it.id);
        if (chip) chip.el = plain;
        el = plain;
      }, { once: true });
    } else if (shown) {
      const img = h('img', { src: shown.url, alt: it.name, decoding: 'async' });
      el = h('div.attach-chip.is-media', { title: it.name }, img, badge(), busy, x);
      // 取不出图的（输入历史翻出来的、发它的会话已经删了）照文件画
      img.addEventListener('error', () => {
        const plain = card();
        el.replaceWith(plain);
        const chip = this.chips.get(it.id);
        if (chip) chip.el = plain;
        el = plain;
      }, { once: true });
    } else {
      el = card();
    }
    return { el, shown, state: '' };
  }

  /** 停用了：不看了，缩略图都松开。 */
  destroy() {
    this.stop();
    for (const chip of this.chips.values()) chip.shown?.release();
    this.chips.clear();
    this.el.remove();
  }
}
