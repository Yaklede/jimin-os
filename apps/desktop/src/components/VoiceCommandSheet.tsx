import {
  CircleAlert,
  LoaderCircle,
  Mic,
  SendHorizontal,
  X,
} from "lucide-react";
import {
  type PointerEvent as ReactPointerEvent,
  useEffect,
  useRef,
  useState,
} from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";

import { copy } from "../copy";

type RecognitionState =
  | "listening"
  | "finalizing"
  | "heard"
  | "unsupported"
  | "permission"
  | "no-speech"
  | "error";

type NativeVoiceResult = {
  transcript: string;
};

type SpeechRecognitionResultLike = {
  isFinal: boolean;
  0: { transcript: string };
};

type SpeechRecognitionEventLike = {
  resultIndex: number;
  results: ArrayLike<SpeechRecognitionResultLike>;
};

type SpeechRecognitionErrorLike = {
  error: string;
};

type SpeechRecognitionLike = {
  lang: string;
  interimResults: boolean;
  continuous: boolean;
  onresult: ((event: SpeechRecognitionEventLike) => void) | null;
  onerror: ((event: SpeechRecognitionErrorLike) => void) | null;
  onend: (() => void) | null;
  start(): void;
  stop(): void;
  abort(): void;
};

type SpeechRecognitionConstructor = new () => SpeechRecognitionLike;

export type VoiceCommandOutcome =
  | {
      kind: "handled";
      message: string;
      destination: "calendar";
    }
  | {
      kind: "needs-details" | "conversation" | "failed";
      message: string;
    };

type VoiceCommandSheetProps = {
  open: boolean;
  onClose(): void;
  onProcessTranscript(value: string): Promise<VoiceCommandOutcome>;
  onOpenTextInput(value?: string): void;
  onOpenDestination(destination: "calendar"): void;
};

export function VoiceCommandSheet({
  open,
  onClose,
  onProcessTranscript,
  onOpenTextInput,
  onOpenDestination,
}: VoiceCommandSheetProps) {
  const recognizerRef = useRef<SpeechRecognitionLike | undefined>(undefined);
  const usingNativeRecognitionRef = useRef(false);
  const completedRef = useRef(false);
  const dragStartYRef = useRef<number | undefined>(undefined);
  const dragOffsetRef = useRef(0);
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<RecognitionState>("listening");
  const [transcript, setTranscript] = useState("");
  const [commandOutcome, setCommandOutcome] = useState<
    VoiceCommandOutcome | undefined
  >(undefined);
  const [processingCommand, setProcessingCommand] = useState(false);
  const [dragOffset, setDragOffset] = useState(0);
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    if (!open) return;

    completedRef.current = false;
    setTranscript("");
    setState("listening");
    setCommandOutcome(undefined);
    setProcessingCommand(false);
    resetSheetPosition();

    if (usesAndroidNativeRecognition()) {
      usingNativeRecognitionRef.current = true;
      let disposed = false;

      void invoke<NativeVoiceResult>("plugin:voice-recognition|start")
        .then((result) => {
          if (disposed) return;
          completedRef.current = true;
          const text = result.transcript.trim();
          setTranscript(text);
          setState(text ? "heard" : "no-speech");
        })
        .catch((error: unknown) => {
          if (disposed) return;
          completedRef.current = true;
          setState(nativeErrorStateFor(error));
        });

      return () => {
        disposed = true;
        usingNativeRecognitionRef.current = false;
        void invoke("plugin:voice-recognition|cancel").catch(() => undefined);
      };
    }

    usingNativeRecognitionRef.current = false;
    const Constructor = recognitionConstructor();
    if (!Constructor) {
      setState("unsupported");
      return;
    }

    const recognizer = new Constructor();
    recognizerRef.current = recognizer;
    recognizer.lang = "ko-KR";
    recognizer.interimResults = true;
    recognizer.continuous = false;
    recognizer.onresult = (event) => {
      const result = event.results[event.resultIndex];
      const text = result?.[0]?.transcript.trim();
      if (!text) return;
      setTranscript(text);
      if (result.isFinal) {
        completedRef.current = true;
        setState("heard");
      }
    };
    recognizer.onerror = (event) => {
      completedRef.current = true;
      setState(errorStateFor(event.error));
    };
    recognizer.onend = () => {
      if (!completedRef.current) setState("no-speech");
    };

    try {
      recognizer.start();
    } catch {
      completedRef.current = true;
      setState("error");
    }

    const timeout = window.setTimeout(() => {
      if (completedRef.current) return;
      completedRef.current = true;
      recognizer.stop();
      setState("no-speech");
    }, 8_000);

    return () => {
      window.clearTimeout(timeout);
      recognizer.abort();
      if (recognizerRef.current === recognizer) {
        recognizerRef.current = undefined;
      }
    };
  }, [attempt, open]);

  useEffect(() => {
    if (!open) return;

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      resetSheetPosition();
      onClose();
    }

    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose, open]);

  if (!open) return null;

  function retry() {
    setCommandOutcome(undefined);
    setProcessingCommand(false);
    if (usingNativeRecognitionRef.current) {
      setAttempt((current) => current + 1);
      return;
    }
    completedRef.current = true;
    recognizerRef.current?.abort();
    setAttempt((current) => current + 1);
  }

  function finishListening() {
    if (usingNativeRecognitionRef.current) {
      setState("finalizing");
      void invoke("plugin:voice-recognition|stop").catch((error: unknown) => {
        completedRef.current = true;
        setState(nativeErrorStateFor(error));
      });
      return;
    }
    completedRef.current = true;
    recognizerRef.current?.stop();
    setState(transcript.trim() ? "heard" : "no-speech");
  }

  async function processTranscript() {
    if (!transcript.trim()) return;
    setProcessingCommand(true);
    try {
      setCommandOutcome(await onProcessTranscript(transcript.trim()));
    } catch {
      setCommandOutcome({
        kind: "failed",
        message: copy.voice.commandFailed,
      });
    } finally {
      setProcessingCommand(false);
    }
  }

  function continueConversation() {
    onOpenTextInput(transcript.trim());
  }

  function openCommandDestination() {
    if (commandOutcome?.kind !== "handled") return;
    onOpenDestination(commandOutcome.destination);
  }

  function resetSheetPosition() {
    dragStartYRef.current = undefined;
    dragOffsetRef.current = 0;
    setDragOffset(0);
    setDragging(false);
  }

  function closeSheet() {
    resetSheetPosition();
    onClose();
  }

  function startSheetDrag(event: ReactPointerEvent<HTMLDivElement>) {
    if (event.pointerType === "mouse" && event.button !== 0) return;
    dragStartYRef.current = event.clientY;
    dragOffsetRef.current = 0;
    setDragging(true);
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function moveSheetDrag(event: ReactPointerEvent<HTMLDivElement>) {
    const startY = dragStartYRef.current;
    if (startY === undefined) return;
    const nextOffset = Math.max(0, event.clientY - startY);
    dragOffsetRef.current = nextOffset;
    setDragOffset(nextOffset);
  }

  function endSheetDrag(event: ReactPointerEvent<HTMLDivElement>) {
    if (dragStartYRef.current === undefined) return;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    const shouldClose = dragOffsetRef.current >= 108;
    resetSheetPosition();
    if (shouldClose) onClose();
  }

  return (
    <div
      className="voice-sheet-backdrop"
      role="presentation"
      onPointerDown={(event) => {
        if (event.target === event.currentTarget) closeSheet();
      }}
    >
      <section
        className="voice-sheet"
        aria-labelledby="voice-sheet-title"
        aria-modal="true"
        role="dialog"
        data-dragging={dragging}
        style={{ transform: `translateY(${dragOffset}px)` }}
      >
        <div
          className="voice-sheet__grab-area"
          aria-hidden="true"
          onPointerDown={startSheetDrag}
          onPointerMove={moveSheetDrag}
          onPointerUp={endSheetDrag}
          onPointerCancel={endSheetDrag}
        >
          <div className="voice-sheet__handle" />
        </div>
        <button
          className="voice-sheet__close focus-visible-control"
          type="button"
          aria-label={copy.voice.closeLabel}
          onClick={closeSheet}
        >
          <X aria-hidden="true" />
        </button>

        <span
          className="voice-sheet__orb"
          data-state={processingCommand ? "processing" : state}
          aria-hidden="true"
        >
          {isRecordingState(state) || processingCommand ? (
            <LoaderCircle className="spin" />
          ) : (
            <Mic />
          )}
        </span>
        <h2 id="voice-sheet-title">
          {commandOutcome
            ? titleForOutcome(commandOutcome)
            : processingCommand
              ? copy.voice.processingCommandTitle
              : titleFor(state)}
        </h2>
        <p className="voice-sheet__description" aria-live="polite">
          {commandOutcome
            ? commandOutcome.message
            : processingCommand
              ? copy.voice.processingCommandDescription
              : descriptionFor(state)}
        </p>

        {isRecordingState(state) && (
          <>
            <div className="voice-sheet__wave" aria-hidden="true">
              <span />
              <span />
              <span />
              <span />
              <span />
            </div>
            {state === "listening" && (
              <p className="voice-sheet__examples">
                {copy.voice.listeningHint}
              </p>
            )}
          </>
        )}

        {(state === "heard" || processingCommand || commandOutcome) &&
          transcript && <p className="voice-sheet__transcript">{transcript}</p>}

        {isRecoveryState(state) && (
          <p className="voice-sheet__notice" role="alert">
            <CircleAlert aria-hidden="true" />
            {recoveryFor(state)}
          </p>
        )}

        <div className="voice-sheet__actions">
          {processingCommand ? (
            <button className="primary-button" type="button" disabled>
              <LoaderCircle className="spin" aria-hidden="true" />
              {copy.voice.processingCommandAction}
            </button>
          ) : commandOutcome?.kind === "handled" ? (
            <>
              <button
                className="primary-button focus-visible-control"
                type="button"
                onClick={openCommandDestination}
              >
                <SendHorizontal aria-hidden="true" />
                {copy.voice.openSchedule}
              </button>
              <button
                className="voice-sheet__secondary focus-visible-control"
                type="button"
                onClick={closeSheet}
              >
                {copy.voice.close}
              </button>
            </>
          ) : commandOutcome ? (
            <>
              <button
                className="primary-button focus-visible-control"
                type="button"
                onClick={retry}
              >
                <Mic aria-hidden="true" />
                {copy.voice.retry}
              </button>
              <button
                className="voice-sheet__secondary focus-visible-control"
                type="button"
                onClick={continueConversation}
              >
                {copy.voice.continueConversation}
              </button>
            </>
          ) : state === "heard" ? (
            <button
              className="primary-button focus-visible-control"
              type="button"
              onClick={() => void processTranscript()}
            >
              <SendHorizontal aria-hidden="true" />
              {copy.voice.processTranscript}
            </button>
          ) : state === "listening" ? (
            <>
              <button
                className="primary-button focus-visible-control"
                type="button"
                onClick={finishListening}
              >
                <Mic aria-hidden="true" />
                {copy.voice.finishListening}
              </button>
              <button
                className="voice-sheet__secondary focus-visible-control"
                type="button"
                onClick={() => onOpenTextInput()}
              >
                {copy.voice.useTextInput}
              </button>
            </>
          ) : state === "finalizing" ? (
            <button className="primary-button" type="button" disabled>
              <LoaderCircle className="spin" aria-hidden="true" />
              {copy.voice.finalizingAction}
            </button>
          ) : (
            <>
              <button
                className="primary-button focus-visible-control"
                type="button"
                onClick={retry}
              >
                <Mic aria-hidden="true" />
                {copy.voice.retry}
              </button>
              <button
                className="voice-sheet__secondary focus-visible-control"
                type="button"
                onClick={() => onOpenTextInput()}
              >
                {copy.voice.useTextInput}
              </button>
            </>
          )}
        </div>
        <button
          className="voice-sheet__dismiss focus-visible-control"
          type="button"
          onClick={closeSheet}
        >
          {copy.voice.close}
        </button>
      </section>
    </div>
  );
}

function titleForOutcome(outcome: VoiceCommandOutcome): string {
  if (outcome.kind === "handled") return copy.voice.commandHandledTitle;
  if (outcome.kind === "needs-details")
    return copy.voice.commandNeedsDetailsTitle;
  if (outcome.kind === "conversation")
    return copy.voice.commandConversationTitle;
  return copy.voice.commandFailedTitle;
}

function recognitionConstructor(): SpeechRecognitionConstructor | undefined {
  const windowWithRecognition = window as Window & {
    SpeechRecognition?: SpeechRecognitionConstructor;
    webkitSpeechRecognition?: SpeechRecognitionConstructor;
  };
  return (
    windowWithRecognition.SpeechRecognition ??
    windowWithRecognition.webkitSpeechRecognition
  );
}

function errorStateFor(value: string): RecognitionState {
  if (value === "not-allowed" || value === "service-not-allowed") {
    return "permission";
  }
  if (value === "no-speech") return "no-speech";
  return "error";
}

function usesAndroidNativeRecognition() {
  return isTauri() && /Android/i.test(navigator.userAgent);
}

function nativeErrorStateFor(error: unknown): RecognitionState {
  const detail = nativeErrorDetail(error);
  if (detail.includes("VOICE_PERMISSION")) return "permission";
  if (detail.includes("VOICE_NO_SPEECH")) return "no-speech";
  if (detail.includes("VOICE_UNAVAILABLE")) return "unsupported";
  return "error";
}

function nativeErrorDetail(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error !== null) {
    const value = error as { code?: unknown; message?: unknown };
    return [value.code, value.message]
      .filter((part): part is string => typeof part === "string")
      .join(" ");
  }
  return String(error);
}

function titleFor(state: RecognitionState) {
  if (state === "listening") return copy.voice.listeningTitle;
  if (state === "finalizing") return copy.voice.finalizingTitle;
  if (state === "heard") return copy.voice.heardTitle;
  if (state === "no-speech") return copy.voice.noSpeechTitle;
  return copy.voice.voiceTitle;
}

function descriptionFor(state: RecognitionState) {
  if (state === "listening") return copy.voice.listeningDescription;
  if (state === "finalizing") return copy.voice.finalizingDescription;
  if (state === "heard") return copy.voice.heardDescription;
  if (state === "no-speech") return copy.voice.noSpeechDescription;
  return copy.voice.voiceDescription;
}

function recoveryFor(
  state: Exclude<RecognitionState, "listening" | "finalizing" | "heard">,
) {
  if (state === "permission") return copy.voice.permissionRecovery;
  if (state === "no-speech") return copy.voice.speechFallback;
  return copy.voice.fallbackRecovery;
}

function isRecoveryState(
  state: RecognitionState,
): state is Exclude<RecognitionState, "listening" | "finalizing" | "heard"> {
  return state !== "listening" && state !== "finalizing" && state !== "heard";
}

function isRecordingState(
  state: RecognitionState,
): state is "listening" | "finalizing" {
  return state === "listening" || state === "finalizing";
}
