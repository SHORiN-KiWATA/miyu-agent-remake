// @ts-check
//! 吉祥物（软件包 `mascot`，蓝图 `web.md`「吉祥物」）：整页最前面一层（挂在 `body` 上，不在整页放大的那一层里，位置都是屏幕像素），
//! 一个像素小吉祥物受重力站在台子上：整页的底边是地，输入框的上边沿、浮在框上面的命令列表和后台任务浮层的上边沿是台子（浮层的
//! 升起来会把它顶上去）。和 TUI 首页的是同一个模型（`model.js`），画在一块小画布上（`sprite.js`），手里的电脑、游戏机在
//! `props.js`，怎么动在 `behavior.js`、`world.js`。只有它自己接鼠标：左键拖、点它。跟着这些事动：打字（页面上哪个框都算）、鼠标动、她在
//! 回答（事件 `view.changed`）、换主题（`theme.changed` 重取颜色）。窄屏不出来；停用了就没有。
//! 给别的包一个服务 `mascot`（蓝图「第一次引导」第 11 条）：`stage(舞台)` 换到别的台子上站（撤了回到输入框），`hop()`、`droop()`、
//! `busy(on)` 做动作。`<body data-mascot="hold">` 时先藏着不画（第一次引导一打开盖着底色、还没定站哪的那一下：不然先在输入框上露一下）。

import { h } from '../../src/lib/dom.js';
import { Sprite } from './sprite.js';
import { Props } from './props.js';
import { Behavior } from './behavior.js';
import { runningDeepStatus } from '../../src/lib/jobs.js';

/** 窄屏（和左栏变抽屉的是同一个宽）不出来 */
const NARROW = '(max-width: 836px)';
/** 能打字的 `<input>`（看光标） */
const TEXTY = new Set(['text', 'search', 'url', 'email', 'password', 'number', '']);

/**
 * 别的包交来的舞台：几段台子（地照旧是整页底边，不用交）、平常站哪、放大几倍（照输入框上那一只算）、看着哪一块变（变了推一帧）、
 * 开场从窗口顶上掉下来。
 * @typedef {{platforms: () => import('./world.js').Platform[], home: () => {x: number, platform: import('./world.js').Platform}|null,
 *   scale?: number, root?: HTMLElement, drop?: boolean}} Stage
 */

/** @param {any} ctx */
export function apply(ctx) {
  const c = ctx.config;
  const sprite = new Sprite(c);
  const props = new Props(c, () => sprite.colors);
  const body = h('div.mascot-body', sprite.canvas, props.el);
  // 放大（舞台的 `scale`）套在外面一层，绕着脚缩放、慢慢变过去；挪位置的在最外层，不跟着缓
  const scaler = h('div.mascot-scale', body);
  // 放到第一个位置之前藏着：不然刷新时先在左上角露一帧
  const el = h('div.mascot', { title: ctx.text('label'), style: 'visibility: hidden' }, scaler);
  const layer = h('div.mascot-layer', el);
  document.body.append(layer);
  const box = ctx.composer.box;
  const narrow = matchMedia(NARROW);
  const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches;
  const size = () => ({ w: (c.cols + 2) * c.pixel, h: (c.rows + 2) * c.pixel });
  /** 别的包交来的舞台（第一次引导），没有的站在输入框上 @type {Stage|null} */
  let stage = null;
  /** 台子：舞台的那几段，或者输入框那几段；再加整页的底边 */
  const platforms = () => [...(stage ? stage.platforms() : composerPlatforms()), { id: 'floor', x1: 0, x2: innerWidth, y: innerHeight }];
  /** 输入框的上边沿（两头圆角那一段不算）、开着的浮层的上边沿（顶得上去） */
  const composerPlatforms = () => {
    const edge = c.home.edge;
    const r = box.getBoundingClientRect();
    /** @type {import('./world.js').Platform[]} */
    // 输入框是实心的（`bottom`）：框往上长越过了往下掉的脚，把它顶上去，不从框里穿过去
    const list = r.width ? [{ id: 'composer', x1: r.left + edge, x2: r.right - edge, y: r.top, bottom: r.bottom }] : [];
    // 浮在框上面的浮层都带 `dock-float`（命令列表、后台任务、选语言……）
    for (const f of box.querySelectorAll('.dock-float')) {
      if (!(f instanceof HTMLElement) || f.hidden || f.classList.contains('is-leaving')) continue;
      const fr = f.getBoundingClientRect();
      list.push({ id: f.className.split(' ')[0], x1: fr.left + edge, x2: fr.right - edge, y: fr.top, carry: true });
    }
    return list;
  };
  const behavior = new Behavior(c, {
    draw: (pose) => sprite.draw(pose),
    place: ({ x, y, rot }) => {
      // 脚底是画布里最下面一行有东西的底边：照它对到台子上，歪绕着脚（蹲、抻在模型里画，不拉伸图片）
      const foot = (sprite.bottom + 1) * c.pixel;
      el.style.transform = `translate(${Math.round(x - size().w / 2)}px, ${Math.round(y - foot)}px)`;
      el.style.visibility = '';
      body.style.transformOrigin = `50% ${foot}px`;
      body.style.transform = `rotate(${rot.toFixed(2)}deg)`;
      scaler.style.transformOrigin = `50% ${foot}px`;
      scaler.style.transform = `scale(${stage?.scale ?? 1})`;
      body.style.setProperty('--foot-gap', `${size().h - foot}px`);
    },
    platforms,
    home: () => {
      if (stage) return stage.home();
      const p = composerPlatforms().find((x) => x.id === 'composer');
      return p ? { x: p.x2 + c.home.edge - c.home.right, platform: p } : null;
    },
    head: () => {
      const r = sprite.canvas.getBoundingClientRect();
      return r.width ? { x: r.left + r.width / 2, y: r.top + (r.height * (c.center_row + 1)) / (c.rows + 2) } : null;
    },
    size,
    // 地上正对输入框的那一段（左右各多出半个身子）不站：免得挡住输入框下面那一行字
    avoid: () => {
      if (stage) return null;
      const r = box.getBoundingClientRect();
      return r.width ? { x1: r.left - size().w / 2, x2: r.right + size().w / 2 } : null;
    },
    hold: (kind) => props.hold(kind),
    nextProp: () => props.next(),
    reduced,
    hidden: () => narrow.matches || document.body.dataset.mascot === 'hold',
  });
  // 颜色照主题；换了主题重取
  sprite.recolor();
  ctx.on('theme.changed', () => {
    sprite.recolor();
    sprite.draw(behavior.pose);
    props.draw();
  });
  // 鼠标、按键、打字：看过去（看哪照蓝图第 6 条）；打字时量光标在这一行的哪（量一个大概）。输入框、设置页、引导里的框都算
  const onMove = (/** @type {PointerEvent} */ e) => behavior.moved({ x: e.clientX, y: e.clientY });
  const onKey = () => behavior.touch();
  const texty = (/** @type {EventTarget|null} */ t) => t instanceof HTMLTextAreaElement || (t instanceof HTMLInputElement && TEXTY.has(t.type));
  const onType = (/** @type {Event} */ e) => {
    const input = /** @type {HTMLInputElement|HTMLTextAreaElement} */ (e.target);
    if (!texty(input)) return;
    const r = input.getBoundingClientRect();
    // 密码框量的是一个个圆点，不量原文
    const text = input.type === 'password' ? '•'.repeat(input.value.length) : input.value;
    const line = text.slice(0, input.selectionEnd ?? text.length).split('\n').pop() ?? '';
    const style = getComputedStyle(input);
    const pad = parseFloat(style.paddingLeft) || 0;
    const scale = r.width / (input.offsetWidth || r.width);
    behavior.typing(input.value ? { x: Math.min(r.right, r.left + (pad + measure(line, style.font)) * scale), y: r.top + r.height / 2 } : null);
  };
  const onBlur = (/** @type {Event} */ e) => {
    if (texty(e.target)) behavior.typing(null);
  };
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
  // 别的包让它先藏着（`data-mascot="hold"`）、放开了：推一帧
  const held = new MutationObserver(wake);
  held.observe(document.body, { attributes: true, attributeFilter: ['data-mascot'] });
  const listen = [
    [document, 'pointermove', onMove, { passive: true }], [document, 'keydown', onKey, true], [document, 'input', onType, true], [document, 'focusout', onBlur, true],
    [canvas, 'pointerdown', onDown], [canvas, 'pointermove', onDrag], [canvas, 'pointerup', onUp], [canvas, 'pointercancel', onUp],
    [window, 'resize', wake], [document, 'visibilitychange', wake], [narrow, 'change', wake],
  ];
  for (const [target, name, fn, opt] of listen) target.addEventListener(name, fn, opt);
  // 舞台那一块变了（换了一屏、长高了）：推一帧照新的台子站
  const stageWatch = new MutationObserver(wake);
  const stageResized = new ResizeObserver(wake);
  ctx.effect(() => () => {
    for (const [target, name, fn, opt] of listen) target.removeEventListener(name, fn, opt);
    stageWatch.disconnect();
    stageResized.disconnect();
    held.disconnect();
    watch.disconnect();
    resized.disconnect();
    behavior.destroy();
    layer.remove();
  });
  // 她在回答、别的包叫它忙（引导里测连接）：掏电脑、灯一闪一闪；结束合上、跳一下、灯快闪两下。有后台任务在跑（连嵌套的）：灯隔一阵
  // 亮一下（在跑的任务连子孙照会话状态的 `running_deep`，核心 9-8）
  let answering = false;
  let asked = false;
  const busy = (hop = true) => behavior.setBusy(answering || asked, hop);
  ctx.on('view.changed', (v) => {
    answering = !!v.running;
    busy();
    behavior.setJobs(v.session ? runningDeepStatus(v.session, (sid) => (sid === v.session ? v.status : ctx.sessions.sessions.get(sid)?.view.status ?? null)) : 0);
  });
  ctx.provide('mascot', {
    /** 换到这个舞台上（跳过去）；交回撤销的函数，撤了跳回输入框。 @param {Stage} next */
    stage: (next) => {
      stage = next;
      stageWatch.disconnect();
      stageResized.disconnect();
      if (next.root) {
        stageWatch.observe(next.root, { subtree: true, childList: true, attributes: true, attributeFilter: ['hidden', 'class'] });
        stageResized.observe(next.root);
      }
      behavior.goHome(!!next.drop);
      return () => {
        if (stage !== next) return;
        stage = null;
        stageWatch.disconnect();
        stageResized.disconnect();
        behavior.goHome();
      };
    },
    /** 舞台换了一屏：回到 `home`（同一高度的直接站上去，不然跳过去） */
    home: () => behavior.goHome(),
    hop: () => behavior.hop(),
    droop: () => behavior.droop(),
    /** 掏出、收起电脑；收起时 `hop` 是假的不跳。 @param {boolean} on @param {boolean} [hop] */
    busy: (on, hop = true) => {
      asked = on;
      busy(hop);
    },
    /** 舞台上的台子挪了（滚了一下）：推一帧 */
    refresh: wake,
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
