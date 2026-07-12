import {
  CircleAlert,
  LoaderCircle,
  Mic,
  SendHorizontal,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { copy } from "../copy";

type RecognitionState =
  "listening" | "heard" | "unsupported" | "permission" | "no-speech" | "error";

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

type VoiceCommandSheetProps = {
  open: boolean;
  onClose(): void;
  onUseTranscript(value: string): void;
  onOpenTextInput(): void;
};

export function VoiceCommandSheet({
  open,
  onClose,
  onUseTranscript,
  onOpenTextInput,
}: VoiceCommandSheetProps) {
  const recognizerRef = useRef<SpeechRecognitionLike | undefined>(undefined);
  const completedRef = useRef(false);
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<RecognitionState>("listening");
  const [transcript, setTranscript] = useState("");

  useEffect(() => {
    if (!open) return;

    const Constructor = recognitionConstructor();
    if (!Constructor) {
      setState("unsupported");
      return;
    }

    completedRef.current = false;
    const recognizer = new Constructor();
    recognizerRef.current = recognizer;
    setTranscript("");
    setState("listening");
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

  if (!open) return null;

  function retry() {
    completedRef.current = true;
    recognizerRef.current?.abort();
    setAttempt((current) => current + 1);
  }

  function finishListening() {
    completedRef.current = true;
    recognizerRef.current?.stop();
    setState(transcript.trim() ? "heard" : "no-speech");
  }

  function useTranscript() {
    if (!transcript.trim()) return;
    onUseTranscript(transcript.trim());
  }

  return (
    <div className="voice-sheet-backdrop" role="presentation">
      <section
        className="voice-sheet"
        aria-labelledby="voice-sheet-title"
        aria-modal="true"
        role="dialog"
      >
        <div className="voice-sheet__handle" aria-hidden="true" />
        <button
          className="voice-sheet__close focus-visible-control"
          type="button"
          aria-label={copy.voice.closeLabel}
          onClick={onClose}
        >
          <X aria-hidden="true" />
        </button>

        <span
          className="voice-sheet__orb"
          data-state={state}
          aria-hidden="true"
        >
          {state === "listening" ? <LoaderCircle className="spin" /> : <Mic />}
        </span>
        <h2 id="voice-sheet-title">{titleFor(state)}</h2>
        <p className="voice-sheet__description" aria-live="polite">
          {descriptionFor(state)}
        </p>

        {state === "listening" && (
          <>
            <div className="voice-sheet__wave" aria-hidden="true">
              <span />
              <span />
              <span />
              <span />
              <span />
            </div>
            <p className="voice-sheet__examples">{copy.voice.listeningHint}</p>
          </>
        )}

        {state === "heard" && transcript && (
          <p className="voice-sheet__transcript">{transcript}</p>
        )}

        {isRecoveryState(state) && (
          <p className="voice-sheet__notice" role="alert">
            <CircleAlert aria-hidden="true" />
            {recoveryFor(state)}
          </p>
        )}

        <div className="voice-sheet__actions">
          {state === "heard" ? (
            <button
              className="primary-button focus-visible-control"
              type="button"
              onClick={useTranscript}
            >
              <SendHorizontal aria-hidden="true" />
              {copy.voice.useTranscript}
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
                onClick={onOpenTextInput}
              >
                {copy.voice.useTextInput}
              </button>
            </>
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
                onClick={onOpenTextInput}
              >
                {copy.voice.useTextInput}
              </button>
            </>
          )}
        </div>
        <button
          className="voice-sheet__dismiss focus-visible-control"
          type="button"
          onClick={onClose}
        >
          {copy.voice.close}
        </button>
      </section>
    </div>
  );
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

function titleFor(state: RecognitionState) {
  if (state === "listening") return copy.voice.listeningTitle;
  if (state === "heard") return copy.voice.heardTitle;
  return copy.voice.voiceTitle;
}

function descriptionFor(state: RecognitionState) {
  if (state === "listening") return copy.voice.listeningDescription;
  if (state === "heard") return copy.voice.heardDescription;
  return copy.voice.voiceDescription;
}

function recoveryFor(state: Exclude<RecognitionState, "listening" | "heard">) {
  if (state === "permission") return copy.voice.permissionRecovery;
  if (state === "no-speech") return copy.voice.speechFallback;
  return copy.voice.fallbackRecovery;
}

function isRecoveryState(
  state: RecognitionState,
): state is Exclude<RecognitionState, "listening" | "heard"> {
  return state !== "listening" && state !== "heard";
}
