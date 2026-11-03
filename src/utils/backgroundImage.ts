import { invoke } from '@tauri-apps/api/core';

// 按魔数判断格式，后端返回的 base64 不带 MIME
function sniffMime(b64: string): string {
  if (b64.startsWith('/9j/')) return 'image/jpeg';
  if (b64.startsWith('iVBORw0KGgo')) return 'image/png';
  if (b64.startsWith('R0lGOD')) return 'image/gif';
  if (b64.startsWith('UklGR')) return 'image/webp';
  if (b64.startsWith('Qk')) return 'image/bmp';
  return 'image/jpeg';
}

// 原图直接塞 localStorage 会爆配额，缩到长边 1920 再转 JPEG
async function compressImageBase64(b64: string): Promise<string> {
  const img = new Image();
  img.src = `data:${sniffMime(b64)};base64,${b64}`;
  await img.decode();
  const maxDim = 1920;
  const scale = Math.min(1, maxDim / Math.max(img.naturalWidth, img.naturalHeight));
  // 已经足够小就原样保留，避免重编码损失画质
  if (scale === 1 && b64.length < 1_500_000) return b64;
  const canvas = document.createElement('canvas');
  canvas.width = Math.round(img.naturalWidth * scale);
  canvas.height = Math.round(img.naturalHeight * scale);
  const ctx = canvas.getContext('2d');
  if (!ctx) return b64;
  ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
  return canvas.toDataURL('image/jpeg', 0.85).split(',')[1] ?? b64;
}

// 选择并压缩背景图；用户取消或读取失败会抛错，由调用方决定怎么提示
export async function pickBackgroundImage(): Promise<string> {
  const raw = await invoke<string>('pick_background_image');
  return compressImageBase64(raw);
}
