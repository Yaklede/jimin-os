import { afterEach, describe, expect, it, vi } from "vitest";
import {
  defaultRepresentativeImage,
  isStoredRepresentativeImage,
  loadRepresentativeImage,
  maximumImageFileSize,
  maximumStoredImageLength,
  persistRepresentativeImage,
  prepareRepresentativeImage,
  representativeImageEvent,
  representativeImageStorageKey,
  saveRepresentativeImage,
  subscribeRepresentativeImage,
  validateImageFile,
} from "./representative-image";

const imageData = "data:image/webp;base64,YWJj";
function memoryStorage() {
  const values = new Map<string, string>();
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
    removeItem: (key: string) => void values.delete(key),
  };
}
afterEach(() => vi.unstubAllGlobals());

describe("representative image persistence", () => {
  it("loads the original image, saves locally, restores on reload and resets", () => {
    const storage = memoryStorage();
    expect(loadRepresentativeImage(storage)).toBe(defaultRepresentativeImage);
    expect(saveRepresentativeImage(storage, imageData)).toBe(true);
    expect(loadRepresentativeImage(storage)).toBe(imageData);
    expect(saveRepresentativeImage(storage, null)).toBe(true);
    expect(loadRepresentativeImage(storage)).toBe(defaultRepresentativeImage);
  });
  it("rejects external addresses, active formats, invalid base64 and large values", () => {
    const storage = memoryStorage();
    saveRepresentativeImage(storage, imageData);
    for (const value of [
      "https://example.com/photo.png",
      "javascript:alert(1)",
      "data:image/svg+xml;base64,YWJj",
      "data:image/png;base64,<script>",
      `data:image/png;base64,${"a".repeat(maximumStoredImageLength)}`,
      "",
    ]) {
      expect(isStoredRepresentativeImage(value)).toBe(false);
      expect(saveRepresentativeImage(storage, value)).toBe(false);
      expect(loadRepresentativeImage(storage)).toBe(imageData);
      storage.setItem(representativeImageStorageKey, value);
      expect(loadRepresentativeImage(storage)).toBe(defaultRepresentativeImage);
      saveRepresentativeImage(storage, imageData);
    }
  });
  it("keeps the prior value when storage is full, denied or unavailable", () => {
    const storage = memoryStorage();
    saveRepresentativeImage(storage, imageData);
    const denied = {
      ...storage,
      setItem: () => {
        throw new Error("full");
      },
    };
    expect(saveRepresentativeImage(denied, "data:image/png;base64,ZGVm")).toBe(
      false,
    );
    expect(loadRepresentativeImage(storage)).toBe(imageData);
    expect(saveRepresentativeImage(undefined, imageData)).toBe(false);
    expect(
      loadRepresentativeImage({
        ...storage,
        getItem: () => {
          throw new Error("denied");
        },
      }),
    ).toBe(defaultRepresentativeImage);
  });
  it("updates all local subscribers only after a successful save and removes listeners", () => {
    const events = new EventTarget();
    const storage = memoryStorage();
    vi.stubGlobal("window", Object.assign(events, { localStorage: storage }));
    const listener = vi.fn();
    const unsubscribe = subscribeRepresentativeImage(listener);
    expect(persistRepresentativeImage(imageData)).toBe(true);
    expect(listener).toHaveBeenCalledTimes(1);
    expect(persistRepresentativeImage("https://example.com/bad.png")).toBe(
      false,
    );
    expect(listener).toHaveBeenCalledTimes(1);
    const unrelated = new Event("storage");
    Object.assign(unrelated, { key: "other-preference" });
    events.dispatchEvent(unrelated);
    expect(listener).toHaveBeenCalledTimes(1);
    const changed = new Event("storage");
    Object.assign(changed, { key: representativeImageStorageKey });
    events.dispatchEvent(changed);
    expect(listener).toHaveBeenCalledTimes(2);
    unsubscribe();
    events.dispatchEvent(new Event(representativeImageEvent));
    events.dispatchEvent(changed);
    expect(listener).toHaveBeenCalledTimes(2);
  });
});

describe("local image preparation", () => {
  it("validates supported input types and size without reading the file", () => {
    for (const type of ["image/png", "image/jpeg", "image/webp"]) {
      expect(() => validateImageFile({ type, size: 100 })).not.toThrow();
    }
    for (const file of [
      { type: "image/png", size: 0 },
      { type: "image/png", size: maximumImageFileSize + 1 },
      { type: "image/svg+xml", size: 100 },
      { type: "text/html", size: 100 },
    ])
      expect(() => validateImageFile(file)).toThrow();
  });
  function decodingEnvironment(fail = false) {
    const revokeObjectURL = vi.fn();
    const createObjectURL = vi.fn(() => "blob:local-photo");
    class DecodedImage {
      naturalWidth = 2048;
      naturalHeight = 1024;
      onload: (() => void) | null = null;
      onerror: (() => void) | null = null;
      set src(value: string) {
        if (value)
          queueMicrotask(() => (fail ? this.onerror?.() : this.onload?.()));
      }
    }
    const drawImage = vi.fn();
    const canvas = {
      width: 0,
      height: 0,
      getContext: () => ({ drawImage }),
      toDataURL: vi.fn(() => imageData),
    };
    vi.stubGlobal("Image", DecodedImage);
    vi.stubGlobal("URL", { createObjectURL, revokeObjectURL });
    vi.stubGlobal("document", { createElement: () => canvas });
    return { revokeObjectURL, createObjectURL, canvas, drawImage };
  }
  it("re-encodes to a bounded thumbnail and revokes the temporary source", async () => {
    const { canvas, revokeObjectURL, drawImage } = decodingEnvironment();
    const result = await prepareRepresentativeImage(
      new File(["photo"], "image.png", { type: "image/png" }),
      new AbortController().signal,
    );
    expect(result).toBe(imageData);
    expect([canvas.width, canvas.height]).toEqual([512, 256]);
    expect(drawImage).toHaveBeenCalledTimes(1);
    expect(canvas.toDataURL).toHaveBeenCalledWith("image/webp", 0.82);
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:local-photo");
  });
  it("does not persist corrupt image content and still revokes its source", async () => {
    const { revokeObjectURL } = decodingEnvironment(true);
    await expect(
      prepareRepresentativeImage(
        new File(["not an image"], "fake.png", { type: "image/png" }),
        new AbortController().signal,
      ),
    ).rejects.toMatchObject({ reason: "decode" });
    expect(revokeObjectURL).toHaveBeenCalledOnce();
  });
  it("cancels when the control unmounts, without encoding or keeping temporary URLs", async () => {
    const { revokeObjectURL, drawImage } = decodingEnvironment();
    const controller = new AbortController();
    const preparing = prepareRepresentativeImage(
      new File(["photo"], "image.png", { type: "image/png" }),
      controller.signal,
    );
    controller.abort();
    await expect(preparing).rejects.toMatchObject({ name: "AbortError" });
    expect(drawImage).not.toHaveBeenCalled();
    expect(revokeObjectURL).toHaveBeenCalledOnce();
  });
});
