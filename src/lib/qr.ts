import jsQR from "jsqr";
import { clipImageSrc } from "./clipMedia";
import { ipasteApi } from "./ipasteApi";
import type { ClipItem } from "../types";

const MAX_QR_SCAN_SIZE = 1200;

export async function decodeQrFromClip(clip: ClipItem) {
  if (clip.clipType !== "image") return null;
  if (clip.text.startsWith("data:image/")) {
    return decodeQrFromImageSource(clip.text);
  }

  try {
    return await decodeQrFromImageSource(await ipasteApi.readClipImageDataUrl(clip.text));
  } catch {
    return decodeQrFromImageSource(clipImageSrc(clip));
  }
}

export async function decodeQrFromFile(file: File) {
  const objectUrl = URL.createObjectURL(file);
  try {
    return await decodeQrFromImageSource(objectUrl);
  } finally {
    URL.revokeObjectURL(objectUrl);
  }
}

export async function decodeQrFromImageSource(source: string) {
  const image = await loadImage(source);
  const scale = Math.min(1, MAX_QR_SCAN_SIZE / Math.max(image.naturalWidth, image.naturalHeight));
  const width = Math.max(1, Math.round(image.naturalWidth * scale));
  const height = Math.max(1, Math.round(image.naturalHeight * scale));
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) return null;

  context.drawImage(image, 0, 0, width, height);
  const imageData = context.getImageData(0, 0, width, height);
  const result = jsQR(imageData.data, width, height, { inversionAttempts: "attemptBoth" });
  return result?.data?.trim() || null;
}

function loadImage(source: string) {
  return new Promise<HTMLImageElement>((resolve, reject) => {
    const image = new Image();
    image.decoding = "async";
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("Cannot load QR image"));
    image.src = source;
  });
}
