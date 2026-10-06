import { ImagePlus, LoaderCircle, RotateCcw } from "lucide-react";
import { type ChangeEvent, useEffect, useId, useRef, useState } from "react";
import { copy } from "../copy";
import {
  defaultRepresentativeImage,
  persistRepresentativeImage,
  prepareRepresentativeImage,
  RepresentativeImageError,
  type ImageError,
  useRepresentativeImage,
} from "../representative-image";
import { RepresentativeImage } from "./RepresentativeImage";

export function RepresentativeImageControl() {
  const inputRef = useRef<HTMLInputElement>(null);
  const operationRef = useRef<AbortController | null>(null);
  const inputId = useId();
  const source = useRepresentativeImage();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ImageError>();
  const [saved, setSaved] = useState(false);
  useEffect(() => () => operationRef.current?.abort(), []);

  async function chooseImage(event: ChangeEvent<HTMLInputElement>) {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = "";
    if (!file || operationRef.current) return;
    const operation = new AbortController();
    operationRef.current = operation;
    setBusy(true);
    setError(undefined);
    setSaved(false);
    try {
      const prepared = await prepareRepresentativeImage(file, operation.signal);
      if (operation.signal.aborted) return;
      if (!persistRepresentativeImage(prepared))
        throw new RepresentativeImageError("save");
      setSaved(true);
    } catch (cause) {
      if (!operation.signal.aborted) {
        setError(
          cause instanceof RepresentativeImageError ? cause.reason : "decode",
        );
      }
    } finally {
      operationRef.current = null;
      if (!operation.signal.aborted) setBusy(false);
    }
  }

  function resetImage() {
    setError(undefined);
    setSaved(false);
    if (!persistRepresentativeImage(null)) setError("save");
    else setSaved(true);
  }

  return (
    <section
      className="appearance-panel__section"
      aria-label={copy.appearance.image.title}
    >
      <span>{copy.appearance.image.title}</span>
      <div className="representative-image-control" aria-busy={busy}>
        <span
          className="representative-image-control__preview"
          aria-hidden="true"
        >
          <RepresentativeImage />
        </span>
        <div className="representative-image-control__actions">
          <input
            ref={inputRef}
            id={inputId}
            className="sr-only"
            type="file"
            accept="image/png,image/jpeg,image/webp"
            aria-label={copy.appearance.image.choose}
            tabIndex={-1}
            disabled={busy}
            onChange={(event) => void chooseImage(event)}
          />
          <button
            type="button"
            className="secondary-button focus-visible-control"
            disabled={busy}
            onClick={() => inputRef.current?.click()}
          >
            {busy ? (
              <LoaderCircle className="spin" aria-hidden="true" />
            ) : (
              <ImagePlus aria-hidden="true" />
            )}
            {busy
              ? copy.appearance.image.preparing
              : copy.appearance.image.change}
          </button>
          <button
            type="button"
            className="text-button focus-visible-control"
            disabled={busy || source === defaultRepresentativeImage}
            onClick={resetImage}
          >
            <RotateCcw aria-hidden="true" />
            {copy.appearance.image.reset}
          </button>
        </div>
      </div>
      <p className="appearance-panel__help">{copy.appearance.image.help}</p>
      <p className="appearance-panel__help">
        {copy.appearance.image.localOnly}
      </p>
      {(busy || error || saved) && (
        <p
          className="representative-image-control__status"
          data-error={Boolean(error)}
          role={error ? "alert" : "status"}
        >
          {error
            ? copy.appearance.image.errors[error]
            : busy
              ? copy.appearance.image.preparing
              : copy.appearance.image.saved}
        </p>
      )}
    </section>
  );
}
