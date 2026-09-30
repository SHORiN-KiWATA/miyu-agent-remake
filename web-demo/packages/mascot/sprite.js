// @ts-check
//! 吉祥物的画布（软件包 `mascot`，蓝图 `web.md`「吉祥物」第 1 条）：一格一个像素画进一块小画布，再照 `pixel` 放大（`image-rendering:
//! pixelated`，边不糊）。颜色取主题的 `--t-mascot-*`（每套主题一份）：头、耳朵、鳍各两色，亮的和影子的，照那一格第几档亮在两个
//! 之间取（影子偏冷偏紫，不是把亮色压暗：项目主人嫌原来金黄色压暗成了「屎黄色」）；眼睛照原色；锯齿线（嘴）和肚子上的圈是头的
//! 影子往轮廓色靠一半；张开的嘴里是轮廓色；肚子上的灯亮着时圈里是灯的颜色。外面一圈一像素的轮廓（奶白的头放在浅色的底上才看得清），所以画布四边各多一格。主题换了
//! 由 `recolor` 重取。

import { h } from '../../src/lib/dom.js';
import { render } from './model.js';

/** 颜色的名字 → CSS 变量 */
const VARS = {
  head: '--t-mascot-head', head_shade: '--t-mascot-head-shade', ear: '--t-mascot-ear', ear_shade: '--t-mascot-ear-shade',
  fin: '--t-mascot-fin', fin_shade: '--t-mascot-fin-shade', eye: '--t-mascot-eye', outline: '--t-mascot-outline', lamp: '--t-mascot-lamp',
};

/**
 * 每一格画什么（画布四边各多一格给轮廓）：身子的格子交回 `{part, level}`；挨着身子的空格是 `'outline'`；外面是 `null`。纯函数。
 * @param {(import('./model.js').Cell|null)[][]} cells `render` 画出来的
 * @returns {(import('./model.js').Cell|'outline'|null)[][]}
 */
export function paint(cells) {
  const rows = cells.length;
  const cols = cells[0]?.length ?? 0;
  const solid = (x, y) => y >= 0 && y < rows && x >= 0 && x < cols && !!cells[y][x];
  const out = [];
  for (let y = -1; y <= rows; y++) {
    const row = [];
    for (let x = -1; x <= cols; x++) {
      if (solid(x, y)) row.push(cells[y][x]);
      else row.push(solid(x - 1, y) || solid(x + 1, y) || solid(x, y - 1) || solid(x, y + 1) ? 'outline' : null);
    }
    out.push(row);
  }
  return out;
}
/** 两个颜色之间取 `k`（0 是 `a`） */
const mix = (a, b, k) => a.map((v, i) => Math.round(v + (b[i] - v) * k));

export class Sprite {
  /**
   * @param {any} config 这个包的设置项
   */
  constructor(config) {
    this.config = config;
    const w = config.cols + 2;
    const hgt = config.rows + 2;
    this.canvas = /** @type {HTMLCanvasElement} */ (h('canvas.mascot-canvas', { width: w, height: hgt }));
    this.canvas.style.width = `${w * config.pixel}px`;
    this.canvas.style.height = `${hgt * config.pixel}px`;
    this.ctx = /** @type {CanvasRenderingContext2D} */ (this.canvas.getContext('2d'));
    /** @type {Record<string, number[]>} */
    this.colors = {};
    this.drawn = '';
    /** 最下面一行有东西的在第几行（算脚踩在哪，轮廓那一格也算） */
    this.bottom = hgt - 1;
  }

  /** 照主题重取颜色（CSS 变量换成 rgb），下一次画时用。 */
  recolor() {
    const probe = h('span', { style: 'display:none' });
    document.body.append(probe);
    for (const [name, css] of Object.entries(VARS)) {
      probe.style.color = `var(${css})`;
      const m = getComputedStyle(probe).color.match(/\d+(\.\d+)?/g) ?? ['0', '0', '0'];
      this.colors[name] = m.slice(0, 3).map(Number);
    }
    probe.remove();
    this.drawn = '';
  }

  /**
   * 画成 `pose` 的样子；和上一次一样的不画。
   * @param {import('./model.js').Pose} pose
   */
  draw(pose) {
    const c = this.config;
    const sig = [pose.yaw.toFixed(1), pose.pitch.toFixed(1), pose.blink, pose.ear.toFixed(2), (pose.stride ?? 0).toFixed(2), (pose.crouch ?? 0).toFixed(2), (pose.mouth ?? 0).toFixed(2), (pose.turn ?? 0).toFixed(1), !!pose.lamp].join('|');
    if (sig === this.drawn) return;
    this.drawn = sig;
    const cells = render(c.model, pose, { cols: c.cols, rows: c.rows, radius: c.radius, center_row: c.center_row });
    const grid = paint(cells);
    const w = c.cols + 2;
    const image = this.ctx.createImageData(w, c.rows + 2);
    let bottom = 0;
    grid.forEach((row, y) => row.forEach((cell, x) => {
      if (!cell) return;
      const rgb = cell === 'outline' ? this.colors.outline : this.rgbOf(cell, c.model.levels);
      image.data.set([rgb[0], rgb[1], rgb[2], 255], (y * w + x) * 4);
      bottom = Math.max(bottom, y);
    }));
    this.bottom = bottom;
    this.ctx.putImageData(image, 0, 0);
  }

  /** 一格的颜色：眼睛照原色；锯齿线是头的影子往轮廓色靠一半；别的照这一档在影子和亮色之间取。 */
  rgbOf(cell, levels) {
    const k = this.colors;
    if (cell.part === 'eye') return k.eye;
    if (cell.part === 'line' || cell.part === 'belly') return mix(k.head_shade, k.outline, this.config.line_shade);
    if (cell.part === 'mouth') return k.outline;
    if (cell.part === 'lamp') return k.lamp;
    const top = Math.max(1, levels - 1);
    return mix(k[`${cell.part}_shade`], k[cell.part], cell.level / top);
  }
}
