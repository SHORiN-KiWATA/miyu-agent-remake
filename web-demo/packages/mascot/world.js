// @ts-check
//! 吉祥物的重力（软件包 `mascot`，蓝图 `web.md`「吉祥物」第 2–4 条）：台子是一段段水平的线（整页的底边是地，输入框的上边沿、浮在框
//! 上面的东西的上边沿是台子）。站在台子上的照走的速度挪，走出了台子的边就往下掉；掉的照重力越掉越快，落在下面第一个台子上，交回
//! 落地那一刻有多快（压扁多少照它）。台子只挡往下掉的（从下面跳上去穿得过）。台子升上来把它顶上去由 `supportAt` 找（给多远算
//! 顶得到）。纯函数：位置、速度都是屏幕上的像素。

/**
 * @typedef {{x1: number, x2: number, y: number, id?: string, carry?: boolean, bottom?: number}} Platform 一个台子：从哪到哪、多高（y 往下是正）；
 *   是哪个（跟着它挪）、升起来会不会把站在下面台子上的顶上去；`bottom` 是实心的（输入框）下沿：往下掉时脚落进上沿和它之间的，顶到上沿
 * @typedef {{x: number, y: number, vx: number, vy: number, ground: Platform|null}} Body 吉祥物的脚底中点、速度、站在哪
 */

/** 顶上去最多顶多高（像素）：台子升到脚上面这么远以内的算把它顶上去，再远的是从旁边升起来的，不管 */
export const PUSH = 90;

/**
 * 脚底下（或者脚上面不远，被顶上去）的台子：x 在台子范围里，台子在脚上面 `reach` 以内、脚下 1 像素以内；几个都合的取最高的。
 * @param {Body} body
 * @param {Platform[]} platforms
 * @param {number} [reach]
 */
export function supportAt(body, platforms, reach = PUSH) {
  const hits = platforms.filter((p) => body.x >= p.x1 && body.x <= p.x2 && p.y <= body.y + 1 && p.y >= body.y - reach);
  return hits.reduce((best, p) => (!best || p.y < best.y ? p : best), /** @type {Platform|null} */ (null));
}

/**
 * 走一步（`dt` 毫秒）。
 * @param {Body} body
 * @param {number} dt
 * @param {Platform[]} platforms
 * @param {number} g 重力（像素每平方秒）
 * @returns {{body: Body, landed: number|null}} 落地的交回落地那一刻往下的速度
 */
export function step(body, dt, platforms, g) {
  const s = dt / 1000;
  const b = { ...body };
  if (b.ground) {
    b.x += b.vx * s;
    const under = supportAt(b, platforms, 1);
    if (under) return { body: { ...b, y: under.y, vy: 0, ground: under }, landed: null };
    b.ground = null;
  }
  const y0 = b.y;
  b.vy += g * s;
  b.x += b.vx * s;
  b.y += b.vy * s;
  const hit = platforms
    .filter((p) => b.x >= p.x1 && b.x <= p.x2 && p.y >= y0 - 0.001 && p.y <= b.y)
    .reduce((best, p) => (!best || p.y < best.y ? p : best), /** @type {Platform|null} */ (null));
  if (!hit) return { body: b, landed: null };
  return { body: { ...b, y: hit.y, vy: 0, vx: 0, ground: hit }, landed: b.vy };
}
