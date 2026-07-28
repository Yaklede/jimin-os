import json
import os
import tempfile
from pathlib import Path
from threading import Lock
from typing import Annotated
from urllib.error import HTTPError, URLError
from urllib.request import Request as UrlRequest
from urllib.request import urlopen

from fastapi import FastAPI, Header, HTTPException, Request
from pydantic import BaseModel, Field

MAX_AUDIO_BYTES = 512 * 1024 * 1024
MAX_SECRET_FILE_BYTES = 16 * 1024
MODEL_LOCK = Lock()
WHISPER_MODEL = None
DIARIZATION_PIPELINE = None
MODEL_ACCESS_READY = False

app = FastAPI(title="Jimin OS meeting transcriber", docs_url=None, redoc_url=None)


class Speaker(BaseModel):
    key: str
    display_name: str | None = Field(default=None, alias="displayName")


class Segment(BaseModel):
    speaker_key: str = Field(alias="speakerKey")
    starts_at_milliseconds: int = Field(alias="startsAtMilliseconds")
    ends_at_milliseconds: int = Field(alias="endsAtMilliseconds")
    text: str
    confidence: int | None = None


class Transcription(BaseModel):
    transcript: str
    speakers: list[Speaker]
    segments: list[Segment]


@app.get("/healthz")
def health() -> dict[str, str]:
    if os.getenv("JIMIN_TRANSCRIBER_MODE", "production") != "fake":
        _verify_model_access(_hugging_face_token())
    return {"status": "ok"}


@app.post("/v1/transcribe", response_model=Transcription, response_model_by_alias=True)
async def transcribe(
    request: Request,
    content_type: Annotated[str | None, Header()] = None,
    x_meeting_participants: Annotated[str, Header()] = "[]",
) -> Transcription:
    audio = await request.body()
    if not audio or len(audio) > MAX_AUDIO_BYTES:
        raise HTTPException(status_code=413, detail="invalid_audio_size")
    participants = _participants(x_meeting_participants)
    if os.getenv("JIMIN_TRANSCRIBER_MODE", "production") == "fake":
        return _fake_transcription(participants)

    suffix = _audio_suffix(content_type)
    with tempfile.TemporaryDirectory(prefix="jimin-meeting-") as directory:
        source = Path(directory) / f"recording{suffix}"
        source.write_bytes(audio)
        try:
            return _transcribe_file(source, participants)
        except HTTPException:
            raise
        except Exception as error:
            # Model and provider details stay inside the trusted sidecar.
            raise HTTPException(status_code=503, detail="transcriber_unavailable") from error


def _transcribe_file(source: Path, participants: list[str]) -> Transcription:
    whisper, diarizer = _models()
    segments, _ = whisper.transcribe(
        str(source),
        language=os.getenv("JIMIN_TRANSCRIBER_LANGUAGE", "ko"),
        vad_filter=True,
        beam_size=5,
    )
    diarization = diarizer(str(source))
    diarization_tracks = list(diarization.speaker_diarization.itertracks(yield_label=True))

    result_segments: list[Segment] = []
    speaker_keys: list[str] = []
    for raw in segments:
        text = raw.text.strip()
        if not text:
            continue
        speaker_key = _speaker_for(raw.start, raw.end, diarization_tracks)
        if speaker_key not in speaker_keys:
            speaker_keys.append(speaker_key)
        probability = getattr(raw, "avg_logprob", None)
        confidence = None
        if probability is not None:
            confidence = max(0, min(100, round((1 + float(probability)) * 100)))
        result_segments.append(
            Segment(
                speakerKey=speaker_key,
                startsAtMilliseconds=round(raw.start * 1000),
                endsAtMilliseconds=max(round(raw.end * 1000), round(raw.start * 1000) + 1),
                text=text,
                confidence=confidence,
            )
        )
    if not result_segments:
        raise HTTPException(status_code=422, detail="speech_not_detected")

    speakers = [
        Speaker(
            key=key,
            displayName=participants[0]
            if len(participants) == 1 and len(speaker_keys) == 1
            else None,
        )
        for key in speaker_keys
    ]
    names = {speaker.key: speaker.display_name or speaker.key for speaker in speakers}
    transcript = "\n".join(
        f"[{_timestamp(segment.starts_at_milliseconds)}] "
        f"{names[segment.speaker_key]}: {segment.text}"
        for segment in result_segments
    )
    return Transcription(
        transcript=transcript,
        speakers=speakers,
        segments=result_segments,
    )


def _models():
    global WHISPER_MODEL, DIARIZATION_PIPELINE
    with MODEL_LOCK:
        if WHISPER_MODEL is None:
            from faster_whisper import WhisperModel

            WHISPER_MODEL = WhisperModel(
                os.getenv("JIMIN_WHISPER_MODEL", "large-v3"),
                device=os.getenv("JIMIN_TRANSCRIBER_DEVICE", "cpu"),
                compute_type=os.getenv("JIMIN_WHISPER_COMPUTE_TYPE", "int8"),
            )
        if DIARIZATION_PIPELINE is None:
            from pyannote.audio import Pipeline

            token = _hugging_face_token()
            DIARIZATION_PIPELINE = Pipeline.from_pretrained(
                "pyannote/speaker-diarization-community-1",
                token=token,
            )
        return WHISPER_MODEL, DIARIZATION_PIPELINE


def _hugging_face_token() -> str:
    token_file = os.getenv("HF_TOKEN_FILE")
    if token_file:
        path = Path(token_file)
        try:
            stat = path.stat()
            if (
                not path.is_absolute()
                or not path.is_file()
                or stat.st_size <= 0
                or stat.st_size > MAX_SECRET_FILE_BYTES
            ):
                raise ValueError
            token = path.read_text(encoding="utf-8").rstrip("\r\n")
        except (OSError, UnicodeError, ValueError) as error:
            raise HTTPException(
                status_code=503, detail="diarization_token_unavailable"
            ) from error
    else:
        token = os.getenv("HF_TOKEN", "").strip()
    if not token or "\0" in token:
        raise HTTPException(status_code=503, detail="diarization_token_missing")
    return token


def _verify_model_access(token: str) -> None:
    global MODEL_ACCESS_READY
    if MODEL_ACCESS_READY:
        return
    with MODEL_LOCK:
        if MODEL_ACCESS_READY:
            return
        request = UrlRequest(
            "https://huggingface.co/pyannote/"
            "speaker-diarization-community-1/resolve/main/config.yaml",
            headers={"Authorization": f"Bearer {token}"},
        )
        try:
            with urlopen(request, timeout=10) as response:
                if response.status != 200:
                    raise HTTPException(
                        status_code=503, detail="diarization_model_unavailable"
                    )
                response.read(1)
        except (HTTPError, URLError, TimeoutError) as error:
            raise HTTPException(
                status_code=503, detail="diarization_model_unavailable"
            ) from error
        MODEL_ACCESS_READY = True


def _speaker_for(start: float, end: float, tracks) -> str:
    best_label = "SPEAKER_00"
    best_overlap = 0.0
    for turn, _, label in tracks:
        overlap = max(0.0, min(end, turn.end) - max(start, turn.start))
        if overlap > best_overlap:
            best_overlap = overlap
            best_label = str(label)
    return best_label


def _participants(raw: str) -> list[str]:
    try:
        parsed = json.loads(raw)
    except json.JSONDecodeError:
        return []
    if not isinstance(parsed, list):
        return []
    return [
        value.strip()
        for value in parsed[:100]
        if isinstance(value, str) and 0 < len(value.strip()) <= 120
    ]


def _audio_suffix(content_type: str | None) -> str:
    value = (content_type or "").lower()
    if "mp4" in value or "m4a" in value:
        return ".m4a"
    if "ogg" in value:
        return ".ogg"
    return ".webm"


def _timestamp(milliseconds: int) -> str:
    seconds = max(0, milliseconds // 1000)
    return f"{seconds // 60:02d}:{seconds % 60:02d}"


def _fake_transcription(participants: list[str]) -> Transcription:
    speaker_name = participants[0] if len(participants) == 1 else None
    segment = Segment(
        speakerKey="SPEAKER_00",
        startsAtMilliseconds=0,
        endsAtMilliseconds=1_000,
        text="로컬 녹음 파이프라인 검증 문장입니다.",
        confidence=100,
    )
    return Transcription(
        transcript="[00:00] SPEAKER_00: 로컬 녹음 파이프라인 검증 문장입니다.",
        speakers=[Speaker(key="SPEAKER_00", displayName=speaker_name)],
        segments=[segment],
    )
