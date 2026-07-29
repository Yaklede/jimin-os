import subprocess
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from fastapi import HTTPException

import app


@dataclass(frozen=True)
class FakeTurn:
    start: float
    end: float


class FakeAnnotation:
    def __init__(self, tracks: list[tuple[float, float, str]]) -> None:
        self.tracks = tracks

    def __bool__(self) -> bool:
        raise AssertionError("pyannote annotations must not be boolean checked")

    def itertracks(self, *, yield_label: bool):
        assert yield_label
        for start, end, label in self.tracks:
            yield FakeTurn(start, end), "_", label


@dataclass(frozen=True)
class FakeDiarization:
    speaker_diarization: FakeAnnotation
    exclusive_speaker_diarization: FakeAnnotation


class FakeWhisper:
    def __init__(self, segments: list[SimpleNamespace]) -> None:
        self.segments = segments
        self.kwargs = None

    def transcribe(self, _source: str, **kwargs):
        self.kwargs = kwargs
        return iter(self.segments), SimpleNamespace()


class FakeDiarizer:
    def __init__(self, output) -> None:
        self.output = output

    def __call__(self, _source: str):
        return self.output


def word(
    start: float,
    end: float,
    text: str,
    probability: float = 0.9,
) -> SimpleNamespace:
    return SimpleNamespace(
        start=start,
        end=end,
        word=text,
        probability=probability,
    )


def whisper_segment(
    start: float,
    end: float,
    text: str,
    words,
) -> SimpleNamespace:
    return SimpleNamespace(
        start=start,
        end=end,
        text=text,
        words=words,
        avg_logprob=-0.1,
    )


class NormalizeAudioTests(unittest.TestCase):
    def test_normalizes_input_to_mono_16khz_pcm_wav(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "recording.m4a"
            source.write_bytes(b"source")

            def create_output(command, **kwargs):
                Path(command[-1]).write_bytes(b"R" * 45)
                return subprocess.CompletedProcess(command, 0)

            with patch.object(app.subprocess, "run", side_effect=create_output) as run:
                normalized = app._normalize_audio(source)

            self.assertEqual(normalized.name, "recording.normalized.wav")
            command = run.call_args.args[0]
            self.assertEqual(command[command.index("-ac") + 1], "1")
            self.assertEqual(command[command.index("-ar") + 1], "16000")
            self.assertEqual(command[command.index("-c:a") + 1], "pcm_s16le")
            self.assertTrue(run.call_args.kwargs["check"])
            self.assertTrue(run.call_args.kwargs["capture_output"])

    def test_rejects_invalid_audio_without_leaking_ffmpeg_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "recording.m4a"
            source.write_bytes(b"invalid")
            error = subprocess.CalledProcessError(
                1,
                ["ffmpeg"],
                stderr=b"sensitive decoder details",
            )

            with patch.object(app.subprocess, "run", side_effect=error):
                with self.assertRaises(HTTPException) as context:
                    app._normalize_audio(source)

            self.assertEqual(context.exception.status_code, 422)
            self.assertEqual(context.exception.detail, "invalid_audio_data")


class SpeakerAttributionTests(unittest.TestCase):
    def test_splits_one_whisper_sentence_using_exclusive_tracks(self) -> None:
        whisper = FakeWhisper(
            [
                whisper_segment(
                    0.0,
                    2.0,
                    "안녕하세요. 네 맞아요.",
                    [
                        word(0.0, 0.35, " 안녕", 0.9),
                        word(0.35, 0.8, "하세요.", 0.8),
                        word(1.05, 1.25, " 네", 0.7),
                        word(1.25, 1.8, " 맞아요.", 0.9),
                    ],
                )
            ]
        )
        diarization = FakeDiarization(
            speaker_diarization=FakeAnnotation([(0.0, 2.0, "WRONG")]),
            exclusive_speaker_diarization=FakeAnnotation(
                [(0.0, 1.0, "A"), (1.0, 2.0, "B")]
            ),
        )

        with patch.object(
            app,
            "_models",
            return_value=(whisper, FakeDiarizer(diarization)),
        ):
            result = app._transcribe_file(Path("meeting.wav"), [])

        self.assertTrue(whisper.kwargs["word_timestamps"])
        self.assertEqual([speaker.key for speaker in result.speakers], ["A", "B"])
        self.assertEqual(
            [
                (
                    segment.speaker_key,
                    segment.text,
                    segment.starts_at_milliseconds,
                    segment.ends_at_milliseconds,
                    segment.confidence,
                )
                for segment in result.segments
            ],
            [
                ("A", "안녕하세요.", 0, 800, 85),
                ("B", "네 맞아요.", 1050, 1800, 80),
            ],
        )
        self.assertEqual(
            result.transcript,
            "[00:00] A: 안녕하세요.\n[00:01] B: 네 맞아요.",
        )

    def test_preserves_a_b_a_turn_order_and_unique_speaker_order(self) -> None:
        whisper = FakeWhisper(
            [
                whisper_segment(
                    0.0,
                    3.0,
                    "첫째 둘째 셋째",
                    [
                        word(0.0, 0.5, " 첫째"),
                        word(1.0, 1.5, " 둘째"),
                        word(2.0, 2.5, " 셋째"),
                    ],
                )
            ]
        )
        diarization = FakeDiarization(
            speaker_diarization=FakeAnnotation([]),
            exclusive_speaker_diarization=FakeAnnotation(
                [
                    (0.0, 0.8, " A "),
                    (0.8, 1.8, "B"),
                    (1.8, 3.0, " A "),
                ]
            ),
        )

        with patch.object(
            app,
            "_models",
            return_value=(whisper, FakeDiarizer(diarization)),
        ):
            result = app._transcribe_file(Path("meeting.wav"), [])

        self.assertEqual([speaker.key for speaker in result.speakers], ["A", "B"])
        self.assertEqual(
            [segment.speaker_key for segment in result.segments],
            ["A", "B", "A"],
        )

    def test_assigns_uncovered_word_to_nearest_speaker(self) -> None:
        timeline = app.SpeakerTimeline(
            app._diarization_tracks(
                FakeAnnotation(
                    [
                        (0.0, 0.8, "A"),
                        (1.2, 2.0, "B"),
                    ]
                )
            )
        )

        self.assertEqual(timeline.speaker_for(0.85, 0.95), "A")
        self.assertEqual(timeline.speaker_for(1.05, 1.15), "B")

    def test_uses_regular_tracks_when_exclusive_output_is_unavailable(self) -> None:
        regular = FakeAnnotation([(0.0, 1.0, "A")])
        diarization = SimpleNamespace(
            speaker_diarization=regular,
            exclusive_speaker_diarization=None,
        )

        self.assertEqual(
            app._diarization_tracks(diarization)[0].speaker_key,
            "A",
        )

    def test_uses_regular_tracks_when_exclusive_output_is_empty(self) -> None:
        diarization = FakeDiarization(
            speaker_diarization=FakeAnnotation([(0.0, 1.0, "A")]),
            exclusive_speaker_diarization=FakeAnnotation([]),
        )

        self.assertEqual(
            app._diarization_tracks(diarization)[0].speaker_key,
            "A",
        )

    def test_falls_back_to_segment_timestamps_without_words(self) -> None:
        whisper = FakeWhisper(
            [
                whisper_segment(0.0, 0.5, "첫 문장", None),
                whisper_segment(0.5, 1.0, "두 번째 문장", []),
            ]
        )
        diarization = FakeAnnotation([(0.0, 1.0, "A")])

        with patch.object(
            app,
            "_models",
            return_value=(whisper, FakeDiarizer(diarization)),
        ):
            result = app._transcribe_file(Path("meeting.wav"), ["지민"])

        self.assertEqual(len(result.segments), 1)
        self.assertEqual(result.segments[0].text, "첫 문장 두 번째 문장")
        self.assertEqual(result.speakers[0].display_name, "지민")

    def test_splits_long_same_speaker_turn_at_segment_contract_limit(self) -> None:
        words = [
            app.AttributedWord(
                speaker_key="A",
                start=index * 0.1,
                end=index * 0.1 + 0.05,
                text=f" {index:04d}",
                confidence=90,
            )
            for index in range(2_000)
        ]

        segments = app._group_attributed_words(words)

        self.assertGreater(len(segments), 1)
        self.assertTrue(
            all(len(segment.text) <= app.MAX_SEGMENT_TEXT_CHARS for segment in segments)
        )
        self.assertEqual(
            " ".join(segment.text for segment in segments),
            "".join(word.text for word in words).strip(),
        )

    def test_uses_latest_end_when_word_timestamps_overlap(self) -> None:
        segments = app._group_attributed_words(
            [
                app.AttributedWord("A", 0.0, 1.0, " 첫째", 90),
                app.AttributedWord("A", 0.8, 0.9, " 둘째", 80),
            ]
        )

        self.assertEqual(len(segments), 1)
        self.assertEqual(segments[0].ends_at_milliseconds, 1_000)

    def test_rejects_transcript_when_all_word_text_is_empty(self) -> None:
        whisper = FakeWhisper(
            [whisper_segment(0.0, 1.0, " ", [word(0.0, 1.0, " ")])]
        )
        diarization = FakeAnnotation([(0.0, 1.0, "A")])

        with patch.object(
            app,
            "_models",
            return_value=(whisper, FakeDiarizer(diarization)),
        ):
            with self.assertRaises(HTTPException) as context:
                app._transcribe_file(Path("meeting.wav"), [])

        self.assertEqual(context.exception.status_code, 422)
        self.assertEqual(context.exception.detail, "speech_not_detected")


if __name__ == "__main__":
    unittest.main()
