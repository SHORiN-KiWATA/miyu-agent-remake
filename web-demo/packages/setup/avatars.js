// @ts-check
//! 人格的头像（核心 P-5）、背景图（核心 P-6，`personas.md`）：列表、`persona.get` 里只有版本（图的 SHA-256 前 16 位），图经
//! `persona.avatar`、`persona.background` 取（base64），照「人格 + 版本」缓存成 blob 地址，版本变了重取。读到了叫一声（列表、对话区
//! 她那一轮的头、对话区的背景重画）。换：选一张图，长边大于设置项（头像 `avatar_side` 512、背景图 `background_side` 2560）的先在浏览器里
//! 缩小（核心收头像 1 MiB、1024 像素以内，背景图 5 MiB、4096 像素以内），传成 blob，再 `persona.set` 带 `avatar` / `background: {blob}`；
//! 移除是 `{unset: true}`。

import { upload } from '../../src/lib/upload.js';

/** 核心收的几种图 */
const KINDS = ['image/png', 'image/jpeg', 'image/webp'];

export class PersonaImages {
  /** @param {any} core 核心的连接 @param {() => void} changed 读到了一张新的（重画） @param {'avatar'|'background'} [kind] 头像还是背景图 */
  constructor(core, changed, kind = 'avatar') {
    this.core = core;
    this.changed = changed;
    this.kind = kind;
    /** `人格@版本` → blob 地址 @type {Map<string, string>} */
    this.urls = new Map();
    /** 正在取的 @type {Set<string>} */
    this.loading = new Set();
  }

  /** 这个人格这个版本的图的地址：缓存里有的交回，没有的去取（取到了叫 `changed`），先交 `null`。 @param {string} id @param {string|null|undefined} version */
  url(id, version) {
    if (!version) return null;
    const key = `${id}@${version}`;
    const got = this.urls.get(key);
    if (got) return got;
    if (!this.loading.has(key)) this.fetch(id, key);
    return null;
  }

  /** @param {string} id @param {string} key */
  async fetch(id, key) {
    this.loading.add(key);
    try {
      const got = await this.core.request(`persona.${this.kind}`, { persona: id });
      if (!got?.data) return;
      const bytes = Uint8Array.from(atob(got.data), (c) => c.charCodeAt(0));
      this.urls.set(`${id}@${got[this.kind]}`, URL.createObjectURL(new Blob([bytes], { type: got.media_type })));
      this.changed();
    } catch {
      // 取不到的照没有画（头像画名字的第一个字，背景图不铺）
    } finally {
      this.loading.delete(key);
    }
  }
}

/**
 * 要传的那张图：核心收的类型、长边不超过 `side`、不比 `maxBytes` 大的照原样；别的在浏览器里画到一张小画布上重新存（WebP，浏览器存不了 WebP
 * 的存 `fallback`：头像 PNG，背景图是照片、PNG 太大，存 JPEG）。
 * @param {File} file @param {number} side 长边最多多少像素 @param {{fallback?: string, maxBytes?: number}} [opts]
 * @returns {Promise<Blob>}
 */
export async function shrink(file, side, opts = {}) {
  const bitmap = await createImageBitmap(file);
  const scale = Math.min(1, side / Math.max(bitmap.width, bitmap.height));
  if (scale === 1 && KINDS.includes(file.type) && file.size <= (opts.maxBytes ?? Infinity)) {
    bitmap.close();
    return file;
  }
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(bitmap.width * scale));
  canvas.height = Math.max(1, Math.round(bitmap.height * scale));
  canvas.getContext('2d')?.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  bitmap.close();
  const encode = (/** @type {string} */ type) => new Promise((resolve) => canvas.toBlob(resolve, type, 0.9));
  const webp = /** @type {Blob|null} */ (await encode('image/webp'));
  if (webp?.type === 'image/webp') return webp;
  const other = /** @type {Blob|null} */ (await encode(opts.fallback ?? 'image/png'));
  if (!other) throw new Error('canvas');
  return other;
}

/**
 * 传成 blob：分块上传（`blob.open`、`blob.write`、`blob.close`），交回内容哈希。
 * @param {any} core @param {Blob} blob @param {{chunk: number, tries: number}} limits @param {string} [name] 起名用（`avatar`、`background`）
 * @returns {Promise<string>}
 */
export async function store(core, blob, limits, name = 'avatar') {
  const request = (/** @type {string} */ method, /** @type {any} */ params) => core.request(method, params);
  const read = async (/** @type {number} */ offset, /** @type {number} */ length) => new Uint8Array(await blob.slice(offset, offset + length).arrayBuffer());
  const got = await upload(request, { name: `${name}.${blob.type.split('/')[1] ?? 'png'}`, size: blob.size }, blob.type || null, read, limits);
  return got.blob;
}
