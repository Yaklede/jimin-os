import json
import logging
import math
import os
import subprocess
import tempfile
import unicodedata
from bisect import bisect_left, bisect_right
from dataclasses import dataclass
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
DEFAULT_DIARIZATION_MODEL_REVISION = "3533c8cf8e369892e6b79ff1bf80f7b0286a54ee"
FFMPEG_TIMEOUT_SECONDS = 15 * 60
MAX_WORD_GROUP_GAP_SECONDS = 1.5
MAX_SEGMENT_TEXT_CHARS = 8_000
MIN_RECOVERY_TRACK_SECONDS = 1.0
MIN_RECOVERY_TURN_SECONDS = 0.2
MIN_RECOVERY_LONG_TURN_SECONDS = 0.5
RECOVERY_CLIP_PADDING_SECONDS = 0.2
RECOVERY_CLIP_MERGE_GAP_SECONDS = 0.05
RECOVERY_CORE_EPSILON_SECONDS = 0.05
MAX_RECOVERY_CLIP_WINDOWS = 24
MAX_RECOVERY_CLIP_SECONDS = 60.0
MODEL_LOCK = Lock()
WHISPER_MODEL = None
DIARIZATION_PIPELINE = None
MODEL_ACCESS_READY = False
LOGGER = logging.getLogger("uvicorn.error")

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


@dataclass(frozen=True)
class AttributedWord:
    speaker_key: str
    start: float
    end: float
    text: str
    confidence: int | None
    turn_index: int = -1


@dataclass(frozen=True)
class SpeakerTrack:
    start: float
    end: float
    speaker_key: str


@dataclass(frozen=True)
class DiarizationSelection:
    tracks: list[SpeakerTrack]
    source: str
    regular_speaker_count: int
    exclusive_speaker_count: int


@dataclass(frozen=True)
class AttributedDiarizationSelection:
    diarization: DiarizationSelection
    attributed_words: list[AttributedWord]
    result_segments: list[Segment]
    regular_attributed_speaker_count: int
    exclusive_attributed_speaker_count: int


@dataclass(frozen=True)
class SpeakerRecoveryPlan:
    core_tracks: list[SpeakerTrack]
    clip_windows: list[tuple[float, float]]


class SpeakerTimeline:
    def __init__(self, tracks: list[SpeakerTrack]) -> None:
        self.tracks = tracks
        self.starts = [track.start for track in tracks]
        self.prefix_max_ends: list[float] = []
        self.prefix_max_end_indices: list[int] = []
        max_end = float("-inf")
        max_end_index = 0
        for index, track in enumerate(tracks):
            if track.end > max_end:
                max_end = track.end
                max_end_index = index
            self.prefix_max_ends.append(max_end)
            self.prefix_max_end_indices.append(max_end_index)

    def speaker_for(self, start: float, end: float) -> str:
        return self.attribution_for(start, end)[0]

    def attribution_for(self, start: float, end: float) -> tuple[str, int]:
        if not self.tracks:
            return "SPEAKER_00", -1

        first_possible_overlap = bisect_right(self.prefix_max_ends, start)
        after_last_possible_overlap = bisect_left(self.starts, end)
        best_speaker: str | None = None
        best_index = -1
        best_overlap = 0.0
        for index in range(
            first_possible_overlap,
            after_last_possible_overlap,
        ):
            track = self.tracks[index]
            overlap = max(0.0, min(end, track.end) - max(start, track.start))
            if overlap > best_overlap:
                best_overlap = overlap
                best_speaker = track.speaker_key
                best_index = index
        if best_speaker is not None:
            return best_speaker, best_index

        insertion_index = bisect_left(self.starts, start)
        nearest: list[tuple[float, int, str]] = []
        if insertion_index > 0:
            previous_index = self.prefix_max_end_indices[insertion_index - 1]
            previous = self.tracks[previous_index]
            nearest.append(
                (
                    max(0.0, start - previous.end),
                    previous_index,
                    previous.speaker_key,
                )
            )
        if insertion_index < len(self.tracks):
            upcoming = self.tracks[insertion_index]
            nearest.append(
                (max(0.0, upcoming.start - end), insertion_index, upcoming.speaker_key)
            )
        if not nearest:
            return "SPEAKER_00", -1
        _, nearest_index, nearest_speaker = min(nearest)
        return nearest_speaker, nearest_index


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
            normalized = _normalize_audio(source)
            return _transcribe_file(normalized, participants)
        except HTTPException:
            raise
        except Exception as error:
            # Model and provider details stay inside the trusted sidecar.
            raise HTTPException(status_code=503, detail="transcriber_unavailable") from error


def _transcribe_file(source: Path, participants: list[str]) -> Transcription:
    whisper, diarizer = _models()
    language = os.getenv("JIMIN_TRANSCRIBER_LANGUAGE", "ko")
    raw_segments, _ = whisper.transcribe(
        str(source),
        language=language,
        vad_filter=True,
        beam_size=5,
        word_timestamps=True,
    )
    raw_segments = list(raw_segments)
    speaker_options = _diarization_speaker_options(participants)
    diarization = diarizer(str(source), **speaker_options)
    attributed_selection = _select_attributed_diarization(
        diarization,
        raw_segments,
    )
    diarization_selection = attributed_selection.diarization
    speaker_tracks = diarization_selection.tracks
    attributed_words = attributed_selection.attributed_words
    recovered_words = _recover_missing_speaker_words(
        whisper=whisper,
        source=source,
        language=language,
        diarization=diarization,
        selection=diarization_selection,
        primary_words=attributed_words,
    )
    if recovered_words:
        attributed_words = sorted(
            [*attributed_words, *recovered_words],
            key=lambda word: (word.start, word.end, word.speaker_key, word.text),
        )
        result_segments = _group_attributed_words(attributed_words)
    else:
        result_segments = attributed_selection.result_segments
    if not result_segments:
        raise HTTPException(status_code=422, detail="speech_not_detected")

    speaker_keys: list[str] = []
    for segment in result_segments:
        speaker_key = segment.speaker_key
        if speaker_key not in speaker_keys:
            speaker_keys.append(speaker_key)

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
    LOGGER.info(
        "transcription completed participants=%d speakers=%d segments=%d "
        "diarization_turns=%d diarization_source=%s "
        "regular_speakers=%d exclusive_speakers=%d "
        "regular_attributed_speakers=%d exclusive_attributed_speakers=%d "
        "recovered_words=%d "
        "min_speakers=%s max_speakers=%s",
        len(participants),
        len(speakers),
        len(result_segments),
        len(speaker_tracks),
        diarization_selection.source,
        diarization_selection.regular_speaker_count,
        diarization_selection.exclusive_speaker_count,
        attributed_selection.regular_attributed_speaker_count,
        attributed_selection.exclusive_attributed_speaker_count,
        len(recovered_words),
        speaker_options.get("min_speakers"),
        speaker_options.get("max_speakers"),
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
                revision=os.getenv(
                    "JIMIN_DIARIZATION_MODEL_REVISION",
                    DEFAULT_DIARIZATION_MODEL_REVISION,
                ),
            )
        return WHISPER_MODEL, DIARIZATION_PIPELINE


def _normalize_audio(source: Path) -> Path:
    normalized = source.with_name("recording.normalized.wav")
    try:
        subprocess.run(
            [
                "ffmpeg",
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-i",
                str(source),
                "-vn",
                "-ac",
                "1",
                "-ar",
                "16000",
                "-c:a",
                "pcm_s16le",
                str(normalized),
            ],
            check=True,
            capture_output=True,
            timeout=FFMPEG_TIMEOUT_SECONDS,
        )
    except (
        FileNotFoundError,
        subprocess.CalledProcessError,
        subprocess.TimeoutExpired,
    ) as error:
        raise HTTPException(status_code=422, detail="invalid_audio_data") from error
    if not normalized.is_file() or normalized.stat().st_size <= 44:
        raise HTTPException(status_code=422, detail="invalid_audio_data")
    return normalized


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


def _diarization_tracks(diarization) -> list[SpeakerTrack]:
    return _select_diarization_tracks(diarization).tracks


def _select_diarization_tracks(diarization) -> DiarizationSelection:
    regular, exclusive_tracks = _diarization_track_sets(diarization)

    regular_speaker_count = _speaker_count(regular)
    exclusive_speaker_count = _speaker_count(exclusive_tracks)
    if exclusive_tracks and exclusive_speaker_count >= regular_speaker_count:
        return DiarizationSelection(
            tracks=exclusive_tracks,
            source="exclusive",
            regular_speaker_count=regular_speaker_count,
            exclusive_speaker_count=exclusive_speaker_count,
        )
    return DiarizationSelection(
        tracks=regular,
        source="regular",
        regular_speaker_count=regular_speaker_count,
        exclusive_speaker_count=exclusive_speaker_count,
    )


def _select_attributed_diarization(
    diarization,
    raw_segments: list,
) -> AttributedDiarizationSelection:
    regular, exclusive = _diarization_track_sets(diarization)
    regular_speaker_count = _speaker_count(regular)
    exclusive_speaker_count = _speaker_count(exclusive)
    raw_prefers_exclusive = (
        bool(exclusive) and exclusive_speaker_count >= regular_speaker_count
    )

    regular_words = _attributed_words_for_tracks(
        raw_segments,
        regular,
    )
    regular_segments = _group_attributed_words(regular_words)
    regular_attributed_speaker_count = _result_speaker_count(regular_segments)

    exclusive_words: list[AttributedWord] = []
    exclusive_segments: list[Segment] = []
    exclusive_attributed_speaker_count = 0
    if exclusive:
        exclusive_words = _attributed_words_for_tracks(
            raw_segments,
            exclusive,
        )
        exclusive_segments = _group_attributed_words(exclusive_words)
        exclusive_attributed_speaker_count = _result_speaker_count(
            exclusive_segments
        )

    use_exclusive = (
        raw_prefers_exclusive
        and exclusive_attributed_speaker_count
        >= regular_attributed_speaker_count
    )
    if use_exclusive:
        selection = DiarizationSelection(
            tracks=exclusive,
            source="exclusive",
            regular_speaker_count=regular_speaker_count,
            exclusive_speaker_count=exclusive_speaker_count,
        )
        attributed_words = exclusive_words
        result_segments = exclusive_segments
    else:
        selection = DiarizationSelection(
            tracks=regular,
            source="regular",
            regular_speaker_count=regular_speaker_count,
            exclusive_speaker_count=exclusive_speaker_count,
        )
        attributed_words = regular_words
        result_segments = regular_segments

    return AttributedDiarizationSelection(
        diarization=selection,
        attributed_words=attributed_words,
        result_segments=result_segments,
        regular_attributed_speaker_count=regular_attributed_speaker_count,
        exclusive_attributed_speaker_count=exclusive_attributed_speaker_count,
    )


def _diarization_track_sets(
    diarization,
) -> tuple[list[SpeakerTrack], list[SpeakerTrack]]:
    regular = _sorted_diarization_tracks(
        getattr(diarization, "speaker_diarization", diarization)
    )
    exclusive = getattr(diarization, "exclusive_speaker_diarization", None)
    exclusive_tracks: list[SpeakerTrack] = []
    if exclusive is not None:
        exclusive_tracks = _sorted_diarization_tracks(exclusive)
    return regular, exclusive_tracks


def _segments_for_tracks(
    raw_segments: list,
    speaker_tracks: list[SpeakerTrack],
) -> list[Segment]:
    return _group_attributed_words(
        _attributed_words_for_tracks(raw_segments, speaker_tracks)
    )


def _attributed_words_for_tracks(
    raw_segments: list,
    speaker_tracks: list[SpeakerTrack],
) -> list[AttributedWord]:
    speaker_timeline = SpeakerTimeline(speaker_tracks)
    attributed_words: list[AttributedWord] = []
    for raw in raw_segments:
        attributed_words.extend(_attribute_segment_words(raw, speaker_timeline))
    attributed_words.sort(key=lambda word: (word.start, word.end))
    return attributed_words


def _recover_missing_speaker_words(
    *,
    whisper,
    source: Path,
    language: str,
    diarization,
    selection: DiarizationSelection,
    primary_words: list[AttributedWord],
) -> list[AttributedWord]:
    regular, exclusive = _diarization_track_sets(diarization)
    plan = _speaker_recovery_plan(
        selected_tracks=selection.tracks,
        regular_tracks=regular,
        exclusive_tracks=exclusive,
        primary_words=primary_words,
    )
    if plan is None:
        return []

    recovery_speaker_count = len(
        {track.speaker_key for track in plan.core_tracks}
    )
    recovery_clip_seconds = sum(
        end - start for start, end in plan.clip_windows
    )
    LOGGER.info(
        "speaker recovery started speakers=%d windows=%d clip_seconds=%.3f",
        recovery_speaker_count,
        len(plan.clip_windows),
        recovery_clip_seconds,
    )
    clip_timestamps = [
        timestamp
        for start, end in plan.clip_windows
        for timestamp in (start, end)
    ]
    raw_segments, _ = whisper.transcribe(
        str(source),
        language=language,
        vad_filter=False,
        beam_size=5,
        word_timestamps=True,
        condition_on_previous_text=False,
        clip_timestamps=clip_timestamps,
    )
    recovered_words = _accepted_recovery_words(
        list(raw_segments),
        core_tracks=plan.core_tracks,
        selected_tracks=selection.tracks,
        primary_words=primary_words,
    )
    LOGGER.info(
        "speaker recovery completed accepted_words=%d",
        len(recovered_words),
    )
    return recovered_words


def _speaker_recovery_plan(
    *,
    selected_tracks: list[SpeakerTrack],
    regular_tracks: list[SpeakerTrack],
    exclusive_tracks: list[SpeakerTrack],
    primary_words: list[AttributedWord],
) -> SpeakerRecoveryPlan | None:
    attributed_speakers = {
        word.speaker_key for word in primary_words if word.text.strip()
    }
    selected_speakers = dict.fromkeys(
        track.speaker_key for track in selected_tracks
    )
    core_tracks: list[SpeakerTrack] = []
    for speaker_key in selected_speakers:
        if speaker_key in attributed_speakers:
            continue
        speaker_tracks = _conservative_speaker_core_tracks(
            speaker_key,
            regular_tracks,
            exclusive_tracks,
        )
        if _has_recovery_track_evidence(speaker_tracks):
            core_tracks.extend(speaker_tracks)

    if not core_tracks:
        return None
    audio_end = max(
        (
            track.end
            for track in [*regular_tracks, *exclusive_tracks, *selected_tracks]
        ),
        default=max(track.end for track in core_tracks),
    )
    clip_windows = _recovery_clip_windows(core_tracks, audio_end=audio_end)
    if not clip_windows:
        return None
    return SpeakerRecoveryPlan(
        core_tracks=sorted(
            core_tracks,
            key=lambda track: (track.start, track.end, track.speaker_key),
        ),
        clip_windows=clip_windows,
    )


def _conservative_speaker_core_tracks(
    speaker_key: str,
    regular_tracks: list[SpeakerTrack],
    exclusive_tracks: list[SpeakerTrack],
) -> list[SpeakerTrack]:
    regular = [
        track
        for track in regular_tracks
        if track.speaker_key == speaker_key and track.end > track.start
    ]
    exclusive = [
        track
        for track in exclusive_tracks
        if track.speaker_key == speaker_key and track.end > track.start
    ]
    if regular and exclusive:
        regular_duration = sum(track.end - track.start for track in regular)
        exclusive_duration = sum(track.end - track.start for track in exclusive)
        # The shorter candidate minimizes VAD-free audio and false-positive exposure.
        return exclusive if exclusive_duration <= regular_duration else regular
    return exclusive or regular


def _has_recovery_track_evidence(tracks: list[SpeakerTrack]) -> bool:
    durations = [track.end - track.start for track in tracks]
    if sum(durations) + 1e-9 < MIN_RECOVERY_TRACK_SECONDS:
        return False
    return (
        sum(
            duration + 1e-9 >= MIN_RECOVERY_TURN_SECONDS
            for duration in durations
        )
        >= 2
        or any(
            duration + 1e-9 >= MIN_RECOVERY_LONG_TURN_SECONDS
            for duration in durations
        )
    )


def _recovery_clip_windows(
    core_tracks: list[SpeakerTrack],
    *,
    audio_end: float,
) -> list[tuple[float, float]]:
    padded = sorted(
        (
            max(0.0, track.start - RECOVERY_CLIP_PADDING_SECONDS),
            min(audio_end, track.end + RECOVERY_CLIP_PADDING_SECONDS),
        )
        for track in core_tracks
    )
    merged: list[tuple[float, float]] = []
    for start, end in padded:
        if end <= start:
            continue
        if (
            merged
            and start <= merged[-1][1] + RECOVERY_CLIP_MERGE_GAP_SECONDS
        ):
            merged[-1] = (merged[-1][0], max(merged[-1][1], end))
        else:
            merged.append((start, end))
    if len(merged) > MAX_RECOVERY_CLIP_WINDOWS:
        return []
    if sum(end - start for start, end in merged) > MAX_RECOVERY_CLIP_SECONDS:
        return []
    return merged


def _accepted_recovery_words(
    raw_segments: list,
    *,
    core_tracks: list[SpeakerTrack],
    selected_tracks: list[SpeakerTrack],
    primary_words: list[AttributedWord],
) -> list[AttributedWord]:
    accepted: list[AttributedWord] = []
    for raw in raw_segments:
        for word in getattr(raw, "words", None) or []:
            text = str(getattr(word, "word", ""))
            start = getattr(word, "start", None)
            end = getattr(word, "end", None)
            if not text.strip() or start is None or end is None:
                continue
            start_value = float(start)
            end_value = max(float(end), start_value + 0.001)
            if not math.isfinite(start_value) or not math.isfinite(end_value):
                continue
            midpoint = (start_value + end_value) / 2
            speaker_key = _recovery_speaker_at_midpoint(midpoint, core_tracks)
            if speaker_key is None:
                continue
            turn_index = _speaker_turn_at_midpoint(
                midpoint,
                speaker_key,
                selected_tracks,
            )
            if turn_index is None:
                continue
            candidate = AttributedWord(
                speaker_key=speaker_key,
                start=start_value,
                end=end_value,
                text=text,
                confidence=_word_confidence(
                    getattr(word, "probability", None)
                ),
                turn_index=turn_index,
            )
            if any(
                _duplicates_primary_word(candidate, primary)
                for primary in primary_words
            ):
                continue
            if any(
                _duplicates_recovered_word(candidate, prior)
                for prior in accepted
            ):
                continue
            accepted.append(candidate)
    return sorted(
        accepted,
        key=lambda word: (word.start, word.end, word.speaker_key, word.text),
    )


def _recovery_speaker_at_midpoint(
    midpoint: float,
    core_tracks: list[SpeakerTrack],
) -> str | None:
    matches = {
        track.speaker_key
        for track in core_tracks
        if track.start - RECOVERY_CORE_EPSILON_SECONDS
        <= midpoint
        <= track.end + RECOVERY_CORE_EPSILON_SECONDS
    }
    if len(matches) != 1:
        return None
    return next(iter(matches))


def _speaker_turn_at_midpoint(
    midpoint: float,
    speaker_key: str,
    selected_tracks: list[SpeakerTrack],
) -> int | None:
    matches = [
        (
            0 if track.start <= midpoint <= track.end else 1,
            abs(midpoint - ((track.start + track.end) / 2)),
            index,
        )
        for index, track in enumerate(selected_tracks)
        if track.speaker_key == speaker_key
        and track.start - RECOVERY_CORE_EPSILON_SECONDS
        <= midpoint
        <= track.end + RECOVERY_CORE_EPSILON_SECONDS
    ]
    if not matches:
        return None
    return min(matches)[2]


def _normalized_word_text(text: str) -> str:
    normalized = unicodedata.normalize("NFKC", text).casefold()
    return "".join(normalized.split())


def _duplicates_primary_word(
    candidate: AttributedWord,
    primary: AttributedWord,
) -> bool:
    if min(candidate.end, primary.end) > max(candidate.start, primary.start):
        return True
    return (
        _normalized_word_text(candidate.text)
        == _normalized_word_text(primary.text)
        and _temporal_distance(candidate, primary)
        <= RECOVERY_CORE_EPSILON_SECONDS
    )


def _duplicates_recovered_word(
    candidate: AttributedWord,
    prior: AttributedWord,
) -> bool:
    if (
        abs(candidate.start - prior.start) <= RECOVERY_CORE_EPSILON_SECONDS
        and abs(candidate.end - prior.end) <= RECOVERY_CORE_EPSILON_SECONDS
    ):
        return True
    return (
        _normalized_word_text(candidate.text)
        == _normalized_word_text(prior.text)
        and _temporal_distance(candidate, prior)
        <= RECOVERY_CORE_EPSILON_SECONDS
    )


def _temporal_distance(
    left: AttributedWord,
    right: AttributedWord,
) -> float:
    if left.end < right.start:
        return right.start - left.end
    if right.end < left.start:
        return left.start - right.end
    return 0.0


def _result_speaker_count(segments: list[Segment]) -> int:
    return len({segment.speaker_key for segment in segments})


def _speaker_count(tracks: list[SpeakerTrack]) -> int:
    return len({track.speaker_key for track in tracks})


def _diarization_speaker_options(participants: list[str]) -> dict[str, int]:
    participant_count = len(participants)
    if participant_count < 2:
        return {}
    return {
        "min_speakers": 2,
        "max_speakers": participant_count,
    }


def _sorted_diarization_tracks(annotation) -> list[SpeakerTrack]:
    return sorted(
        (
            SpeakerTrack(
                start=float(turn.start),
                end=float(turn.end),
                speaker_key=_speaker_key(label),
            )
            for turn, _, label in annotation.itertracks(yield_label=True)
        ),
        key=lambda track: (track.start, track.end, track.speaker_key),
    )


def _attribute_segment_words(
    raw, speaker_timeline: SpeakerTimeline
) -> list[AttributedWord]:
    words = getattr(raw, "words", None)
    attributed: list[AttributedWord] = []
    if words:
        for word in words:
            text = str(getattr(word, "word", ""))
            start = getattr(word, "start", None)
            end = getattr(word, "end", None)
            if not text.strip() or start is None or end is None:
                continue
            start_value = float(start)
            end_value = max(float(end), start_value + 0.001)
            speaker_key, turn_index = speaker_timeline.attribution_for(
                start_value,
                end_value,
            )
            attributed.append(
                AttributedWord(
                    speaker_key=speaker_key,
                    start=start_value,
                    end=end_value,
                    text=text,
                    confidence=_word_confidence(getattr(word, "probability", None)),
                    turn_index=turn_index,
                )
            )
    if attributed:
        return attributed

    text = str(getattr(raw, "text", "")).strip()
    if not text:
        return []
    start = float(getattr(raw, "start", 0.0))
    end = max(float(getattr(raw, "end", start + 0.001)), start + 0.001)
    speaker_key, turn_index = speaker_timeline.attribution_for(start, end)
    return [
        AttributedWord(
            speaker_key=speaker_key,
            start=start,
            end=end,
            text=f" {text}",
            confidence=_segment_confidence(getattr(raw, "avg_logprob", None)),
            turn_index=turn_index,
        )
    ]


def _group_attributed_words(words: list[AttributedWord]) -> list[Segment]:
    groups: list[list[AttributedWord]] = []
    group_text_lengths: list[int] = []
    for word in words:
        combined_text_length = (
            group_text_lengths[-1] + len(word.text)
            if groups
            else len(word.text)
        )
        if (
            groups
            and groups[-1][-1].speaker_key == word.speaker_key
            and groups[-1][-1].turn_index == word.turn_index
            and word.start - groups[-1][-1].end <= MAX_WORD_GROUP_GAP_SECONDS
            and combined_text_length <= MAX_SEGMENT_TEXT_CHARS
        ):
            groups[-1].append(word)
            group_text_lengths[-1] = combined_text_length
        else:
            groups.append([word])
            group_text_lengths.append(len(word.text))

    result: list[Segment] = []
    for group in groups:
        text = "".join(word.text for word in group).strip()
        if not text:
            continue
        confidences = [
            word.confidence for word in group if word.confidence is not None
        ]
        starts_at = round(group[0].start * 1000)
        ends_at = max(
            round(max(word.end for word in group) * 1000),
            starts_at + 1,
        )
        result.append(
            Segment(
                speakerKey=group[0].speaker_key,
                startsAtMilliseconds=starts_at,
                endsAtMilliseconds=ends_at,
                text=text,
                confidence=(
                    round(sum(confidences) / len(confidences))
                    if confidences
                    else None
                ),
            )
        )
    return result


def _word_confidence(probability) -> int | None:
    if probability is None:
        return None
    return max(0, min(100, round(float(probability) * 100)))


def _segment_confidence(avg_logprob) -> int | None:
    if avg_logprob is None:
        return None
    return max(0, min(100, round((1 + float(avg_logprob)) * 100)))


def _speaker_key(label) -> str:
    value = str(label).strip()
    return value[:80] or "SPEAKER_00"


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
