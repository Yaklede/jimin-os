import { useSyncExternalStore } from "react";

export const defaultRepresentativeImage = "/images/hamster-home-wave.png";
export const representativeImageStorageKey = "jimin-os.representative-image.v1";
export const representativeImageEvent = "jimin-os:representative-image";
export const maximumImageFileSize = 10 * 1024 * 1024;
export const maximumStoredImageLength = 350_000;
type ImageStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;
export type ImageError = "fileSize" | "format" | "decode" | "save";

export class RepresentativeImageError extends Error {
  constructor(public readonly reason: ImageError) {
    super(reason);
  }
}

export function isStoredRepresentativeImage(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length <= maximumStoredImageLength &&
    /^data:image\/(?:png|jpeg|webp);base64,[A-Za-z0-9+/]+={0,2}$/.test(value)
  );
}

export function loadRepresentativeImage(storage?: ImageStorage): string {
  try {
    const value = storage?.getItem(representativeImageStorageKey);
    return isStoredRepresentativeImage(value)
      ? value
      : defaultRepresentativeImage;
  } catch {
    return defaultRepresentativeImage;
  }
}

export function saveRepresentativeImage(
  storage: ImageStorage | undefined,
  image: string | null,
): boolean {
  if (!storage || (image !== null && !isStoredRepresentativeImage(image))) {
    return false;
  }
  try {
    if (image === null) storage.removeItem(representativeImageStorageKey);
    else storage.setItem(representativeImageStorageKey, image);
    return true;
  } catch {
    return false;
  }
}

function browserStorage(): ImageStorage | undefined {
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

export function persistRepresentativeImage(image: string | null): boolean {
  if (!saveRepresentativeImage(browserStorage(), image)) return false;
  window.dispatchEvent(new Event(representativeImageEvent));
  return true;
}

export function subscribeRepresentativeImage(listener: () => void): () => void {
  const onStorage = (event: StorageEvent) => {
    if (event.key === representativeImageStorageKey || event.key === null)
      listener();
  };
  window.addEventListener(representativeImageEvent, listener);
  window.addEventListener("storage", onStorage);
  return () => {
    window.removeEventListener(representativeImageEvent, listener);
    window.removeEventListener("storage", onStorage);
  };
}

export function useRepresentativeImage(): string {
  return useSyncExternalStore(
    subscribeRepresentativeImage,
    () => loadRepresentativeImage(browserStorage()),
    () => defaultRepresentativeImage,
  );
}

export function validateImageFile(file: Pick<File, "size" | "type">): void {
  if (!file.size || file.size > maximumImageFileSize) {
    throw new RepresentativeImageError("fileSize");
  }
  if (!["image/png", "image/jpeg", "image/webp"].includes(file.type)) {
    throw new RepresentativeImageError("format");
  }
}

/** Decode and re-encode locally: do not store URLs, SVG, original metadata or large files. */
export async function prepareRepresentativeImage(
  file: File,
  signal: AbortSignal,
): Promise<string> {
  validateImageFile(file);
  const objectUrl = URL.createObjectURL(file);
  try {
    const image = new Image();
    await new Promise<void>((resolve, reject) => {
      const cleanup = () => {
        image.onload = null;
        image.onerror = null;
        signal.removeEventListener("abort", onAbort);
      };
      const onAbort = () => {
        cleanup();
        image.src = "";
        reject(new DOMException("Aborted", "AbortError"));
      };
      image.onload = () => {
        cleanup();
        resolve();
      };
      image.onerror = () => {
        cleanup();
        reject(new RepresentativeImageError("decode"));
      };
      signal.addEventListener("abort", onAbort, { once: true });
      if (signal.aborted) onAbort();
      else image.src = objectUrl;
    });
    if (signal.aborted) throw new DOMException("Aborted", "AbortError");
    if (!image.naturalWidth || !image.naturalHeight) {
      throw new RepresentativeImageError("decode");
    }
    const canvas = document.createElement("canvas");
    const ratio = Math.min(
      1,
      512 / Math.max(image.naturalWidth, image.naturalHeight),
    );
    canvas.width = Math.max(1, Math.round(image.naturalWidth * ratio));
    canvas.height = Math.max(1, Math.round(image.naturalHeight * ratio));
    const context = canvas.getContext("2d");
    if (!context) throw new RepresentativeImageError("decode");
    context.drawImage(image, 0, 0, canvas.width, canvas.height);
    const result = canvas.toDataURL("image/webp", 0.82);
    if (!isStoredRepresentativeImage(result)) {
      throw new RepresentativeImageError("decode");
    }
    return result;
  } finally {
    URL.revokeObjectURL(objectUrl);
  }
}
