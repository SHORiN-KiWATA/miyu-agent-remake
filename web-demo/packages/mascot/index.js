// @ts-check
//! 吉祥物（软件包 `mascot`，蓝图 `web.md`「吉祥物」）：整页最前面一层（挂在 `body` 上，不在整页放大的那一层里，位置都是屏幕像素），
//! 一个像素小吉祥物受重力站在台子上：整页的底边是地，输入框的上边沿、浮在框上面的命令列表和后台任务浮层的上边沿是台子（浮层的
//! 升起来会把它顶上去）。和 TUI 首页的是同一个模型（`model.js`），画在一块小画布上（`sprite.js`），手里的电脑、游戏机在
//! `props.js`，怎么动在 `behavior.js`、`world.js`。只有它自己接鼠标：左键拖、点它。跟着这些事动：在输入框里打字、鼠标动、她在
//! 回答（事件 `view.changed`）、换主题（`theme.changed` 重取颜色）。窄屏不出来；停用了就没有。

import { h } from '../../src/lib/dom.js';
import { Sprite } from './sprite.js';
import { Props } from './props.js';
import { Behavior } from './behavior.js';
import { runningDeep } from '../../src/lib/jobs.js';

/** 窄屏（和左栏变抽屉的是同一个宽）不出来 */
const NARROW = '(max-width: 836px)';

/** @param {any} ctx */
export function apply(ctx) {
  const c = ctx.config;
  const sprite = new Sprite(c);
  const props = new Props(c, () => sprite.colors);
  const body = h('div.mascot-body', sprite.canvas, props.el);
  const el = h('div.mascot', { title: ctx.text('label') }, body);
  const layer = h('div.mascot-layer', el);
  document.body.append(layer);
  const box = ctx.composer.box;
  const narrow = matchMedia(NARROW);
  const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches;
  const size = () => ({ w: (c.cols + 2) * c.pixel, h: (c.rows + 2) * c.pixel });
  /** 台子：输入框的上边沿（两头圆角那一段不算）、开着的浮层的上边沿（顶得上去）、整页的底边 */
  const platforms = () => {
    const edge = c.home.edge;
    const r = box.getBoundingClientRect();
    /** @type {import('./world.js').Platform[]} */
    const list = r.width ? [{ id: 'composer', x1: r.left + edge, x2: r.right - edge, y: r.top }] : [];
    // 浮在框上面的浮层都带 `dock-float`（命令列表、后台任务、选语言……）
    for (const f of box.querySelectorAll('.dock-float')) {
      if (!(f instanceof HTMLElement) || f.hidden || f.classList.contains('is-leaving')) continue;
      const fr = f.getBoundingClientRect();
      list.push({ id: f.className.split(' ')[0], x1: fr.left + edge, x2: fr.right - edge, y: fr.top, carry: true });
    }
    list.push({ id: 'floor', x1: 0, x2: innerWidth, y: innerHeight });
    return list;
  };
  const behavior = new Behavior(c, {
    draw: (pose) => sprite.draw(pose),
    place: ({ x, y, rot }) => {
      // 脚底是画布里最下面一行有东西的底边：照它对到台子上，歪绕着脚（蹲、抻在模型里画，不拉伸图片）
      const foot = (sprite.bottom + 1) * c.pixel;
      el.style.transform = `translate(${Math.round(x - size().w / 2)}px, ${Math.round(y - foot)}px)`;
      body.style.transformOrigin = `50% ${foot}px`;
      body.style.transform = `rotate(${rot.toFixed(2)}deg)`;
      body.style.setProperty('--foot-gap', `${size().h - foot}px`);
    },
    platforms,
    home: () => {
      const p = platforms().find((x) => x.id === 'composer');
      return p ? { x: p.x2 + c.home.edge - c.home.right, platform: p } : null;
    },
    head: () => {
      const r = sprite.canvas.getBoundingClientRect();
      return r.width ? { x: r.left + r.width / 2, y: r.top + (r.height * (c.center_row + 1)) / (c.rows + 2) } : null;
    },
    size,
    // 地上正对输入框的那一段（左右各多出半个身子）不站：免得挡住输入框下面那一行字
    avoid: () => {
      const r = box.getBoundingClientRect();
      return r.width ? { x1: r.left - size().w / 2, x2: r.right + size().w / 2 } : null;
    },
    hold: (kind) => props.hold(kind),
    nextProp: () => props.next(),
    reduced,
    hidden: () => narrow.matches,
  });
  // 颜色照主题；换了主题重取
  sprite.recolor();
  ctx.on('theme.changed', () => {
    sprite.recolor();
    sprite.draw(behavior.pose);
    props.draw();
  });
  // 鼠标、按键、打字：看过去（看哪照蓝图第 6 条）；打字时量光标在这一行的哪（量一个大概）
  const input = ctx.composer.input;
  const onMove = (/** @type {PointerEvent} */ e) => behavior.moved({ x: e.clientX, y: e.clientY });
  const onKey = () => behavior.touch();
  const onType = () => {
    const r = input.getBoundingClientRect();
    const line = input.value.slice(0, input.selectionEnd ?? input.value.length).split('\n').pop() ?? '';
    const style = getComputedStyle(input);
    const pad = parseFloat(style.paddingLeft) || 0;
    const scale = r.width / (input.offsetWidth || r.width);
    behavior.typing(input.value ? { x: Math.min(r.right, r.left + (pad + measure(line, style.font)) * scale), y: r.top + r.height / 2 } : null);
  };
  const onBlur = () => behavior.typing(null);
  // 拖：左键按住跟着指针走，松手掉下去；没挪动的算点它
  const canvas = sprite.canvas;
  const onDown = (/** @type {PointerEvent} */ e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    canvas.setPointerCapture(e.pointerId);
    el.classList.add('is-dragging');
    behavior.grab(e.clientX, e.clientY);
  };
  const onDrag = (/** @type {PointerEvent} */ e) => {
    if (canvas.hasPointerCapture(e.pointerId)) behavior.dragTo(e.clientX, e.clientY);
  };
  const onUp = (/** @type {PointerEvent} */ e) => {
    if (!el.classList.contains('is-dragging')) return;
    el.classList.remove('is-dragging');
    if (canvas.hasPointerCapture(e.pointerId)) canvas.releasePointerCapture(e.pointerId);
    behavior.release();
  };
  // 台子挪了（改窗口、框变高、浮层开关）、标签页回到前面、窄屏宽屏换了：推一帧，照新的台子站
  const wake = () => behavior.wake();
  const watch = new MutationObserver(wake);
  watch.observe(box, { subtree: true, attributes: true, attributeFilter: ['hidden', 'class'], childList: true });
  const resized = new ResizeObserver(wake);
  resized.observe(box);
  const listen = [
    [document, 'pointermove', onMove, { passive: true }], [document, 'keydown', onKey, true], [input, 'input', onType], [input, 'blur', onBlur],
    [canvas, 'pointerdown', onDown], [canvas, 'pointermove', onDrag], [canvas, 'pointerup', onUp], [canvas, 'pointercancel', onUp],
    [window, 'resize', wake], [document, 'visibilitychange', wake], [narrow, 'change', wake],
  ];
  for (const [target, name, fn, opt] of listen) target.addEventListener(name, fn, opt);
  ctx.effect(() => () => {
    for (const [target, name, fn, opt] of listen) target.removeEventListener(name, fn, opt);
    watch.disconnect();
    resized.disconnect();
    behavior.destroy();
    layer.remove();
  });
  // 她在回答：掏电脑、灯一闪一闪；结束合上、跳一下、灯快闪两下。有后台任务在跑（连嵌套的）：灯隔一阵亮一下。事件条数没变的不重算
  let counted = '';
  ctx.on('view.changed', (v) => {
    behavior.setBusy(!!v.running);
    const events = (sid) => (sid === v.session ? v.events : ctx.sessions.sessions.get(sid)?.events ?? null);
    const sig = `${v.session}|${v.events?.length ?? 0}|${[...ctx.sessions.sessions.values()].reduce((n, s) => n + s.events.length, 0)}`;
    if (sig === counted) return;
    counted = sig;
    behavior.setJobs(v.session ? runningDeep(v.session, events) : 0);
  });
  sprite.draw(behavior.pose);
  behavior.wake();
}

/** 一段字照这个字体有多宽（量一个大概，看光标用）。 */
function measure(text, font) {
  const canvas = measure.canvas ??= document.createElement('canvas');
  const g = /** @type {CanvasRenderingContext2D} */ (canvas.getContext('2d'));
  g.font = font;
  return g.measureText(text).width;
}
/** @type {HTMLCanvasElement|undefined} */
measure.canvas = undefined;
