// @ts-check
//! 人格的头像（核心 P-5，`personas.md`「头像」）：列表、`persona.get` 里只有版本（图的 SHA-256 前 16 位），图经 `persona.avatar` 取
//! （base64），照「人格 + 版本」缓存成 blob 地址，版本变了重取。读到了叫一声（列表、对话区她那一轮的头重画）。换头像：选一张图，
//! 长边大于设置项 `avatar_side`（512）的先在浏览器里缩小（核心只收 1 MiB、1024 像素以内），传成 blob，再 `persona.set` 带
//! `avatar: {blob}`；移除是 `avatar: {unset: true}`。

import { upload } from '../../src/lib/upload.js';

/** 核心收的几种图 */
const KINDS = ['image/png', 'image/jpeg', 'image/webp'];

export class Avatars {
  /** @param {any} core 核心的连接 @param {() => void} changed 读到了一张新的（重画） */
  constructor(core, changed) {
    this.core = core;
    this.changed = changed;
    /** `人格@版本` → blob 地址 @type {Map<string, string>} */
    this.urls = new Map();
    /** 正在取的 @type {Set<string>} */
    this.loading = new Set();
  }

  /** 这个人格这个版本的头像的地址：缓存里有的交回，没有的去取（取到了叫 `changed`），先交 `null`。 @param {string} id @param {string|null|undefined} version */
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
      const got = await this.core.request('persona.avatar', { persona: id });
      if (!got?.data) return;
      const bytes = Uint8Array.from(atob(got.data), (c) => c.charCodeAt(0));
      this.urls.set(`${id}@${got.avatar}`, URL.createObjectURL(new Blob([bytes], { type: got.media_type })));
      this.changed();
    } catch {
      // 取不到的照没有头像画（名字的第一个字）
    } finally {
      this.loading.delete(key);
    }
  }
}

/**
 * 换头像要传的那张图：核心收的类型、长边不超过 `side` 的照原样；别的在浏览器里画到一张小画布上重新存（WebP，浏览器存不了 WebP 的存 PNG）。
 * @param {File} file @param {number} side 长边最多多少像素
 * @returns {Promise<Blob>}
 */
export async function shrink(file, side) {
  const bitmap = await createImageBitmap(file);
  const scale = Math.min(1, side / Math.max(bitmap.width, bitmap.height));
  if (scale === 1 && KINDS.includes(file.type)) {
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
  const png = /** @type {Blob|null} */ (await encode('image/png'));
  if (!png) throw new Error('canvas');
  return png;
}

/**
 * 传成 blob：分块上传（`blob.open`、`blob.write`、`blob.close`），交回内容哈希。
 * @param {any} core @param {Blob} blob @param {{chunk: number, tries: number}} limits
 * @returns {Promise<string>}
 */
export async function store(core, blob, limits) {
  const request = (/** @type {string} */ method, /** @type {any} */ params) => core.request(method, params);
  const read = async (/** @type {number} */ offset, /** @type {number} */ length) => new Uint8Array(await blob.slice(offset, offset + length).arrayBuffer());
  const got = await upload(request, { name: `avatar.${blob.type.split('/')[1] ?? 'png'}`, size: blob.size }, blob.type || null, read, limits);
  return got.blob;
}
