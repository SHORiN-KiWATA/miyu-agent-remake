// @ts-check
//! 吉祥物手里的东西（软件包 `mascot`，蓝图 `web.md`「吉祥物」第 5 条）：她在回答时一台电脑（看得到盖子背面，上面的记号是灯的颜色、一闪一闪：电脑挡住了肚子上的灯），
//! 没人动好一阵一台游戏机（屏幕里一个小人跳过一个方块，两边的按键轮着亮）。像素图和图例在清单的 `props` 里，颜色照主题的吉祥物
//! 颜色（和身子同一份，`Sprite.colors`）。盖在身子前面、底边照 `dy` 离脚多高；拿出来从下面弹出来，收起来缩回去（`style.css`）。

import { h } from '../../src/lib/dom.js';
import { show, hide } from '../../src/lib/motion.js';

export class Props {
  /**
   * @param {any} config 这个包的设置项
   * @param {() => Record<string, number[]>} colors 身子的颜色（换主题会变）
   */
  constructor(config, colors) {
    this.config = config;
    this.colors = colors;
    this.canvas = /** @type {HTMLCanvasElement} */ (h('canvas.mascot-prop-canvas'));
    this.ctx = /** @type {CanvasRenderingContext2D} */ (this.canvas.getContext('2d'));
    this.el = h('div.mascot-prop', { hidden: true }, this.canvas);
    /** 拿着哪个；没有是 `null` */
    this.kind = /** @type {string|null} */ (null);
    this.frame = 0;
  }

  /**
   * 拿出来、换一个、收起来（`null`）。
   * @param {string|null} kind `laptop`、`console`
   */
  hold(kind) {
    if (kind === this.kind) return;
    this.kind = kind;
    if (!kind) {
      hide(this.el);
      return;
    }
    const p = this.config.props[kind];
    const px = this.config.pixel;
    const [w, hgt] = [p.frames[0][0].length, p.frames[0].length];
    this.canvas.width = w;
    this.canvas.height = hgt;
    Object.assign(this.canvas.style, { width: `${w * px}px`, height: `${hgt * px}px` });
    this.el.style.setProperty('--prop-dy', `${p.dy}px`);
    this.frame = 0;
    this.draw();
    // 收到一半又拿出来的：`show` 停掉退场
    show(this.el);
  }

  /** 下一帧。 */
  next() {
    if (!this.kind) return;
    this.frame += 1;
    this.draw();
  }

  /** 照图例画这一帧。 */
  draw() {
    if (!this.kind) return;
    const props = this.config.props;
    const frames = props[this.kind].frames;
    const rows = frames[this.frame % frames.length];
    const colors = this.colors();
    const w = rows[0].length;
    const image = this.ctx.createImageData(w, rows.length);
    rows.forEach((row, y) => [...row].forEach((ch, x) => {
      const rgb = ch === '.' ? null : colors[props.legend[ch]];
      if (rgb) image.data.set([rgb[0], rgb[1], rgb[2], 255], (y * w + x) * 4);
    }));
    this.ctx.putImageData(image, 0, 0);
  }
}
