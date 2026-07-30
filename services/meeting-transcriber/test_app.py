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
    def __init__(
        self,
        segments: list[SimpleNamespace],
        auxiliary_segments: list[SimpleNamespace] | None = None,
    ) -> None:
        self.responses = [segments]
        if auxiliary_segments is not None:
            self.responses.append(auxiliary_segments)
        self.kwargs = None
        self.calls: list[tuple[str, dict]] = []

    def transcribe(self, source: str, **kwargs):
        call_index = len(self.calls)
        self.calls.append((source, kwargs))
        self.kwargs = kwargs
        segments = (
            self.responses[call_index]
            if call_index < len(self.responses)
            else []
        )
        return iter(segments), SimpleNamespace()


class FakeDiarizer:
    def __init__(self, output) -> None:
        self.output = output
        self.kwargs = None

    def __call__(self, _source: str, **kwargs):
        self.kwargs = kwargs
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


def operational_missing_speaker_diarization() -> FakeDiarization:
    return FakeDiarization(
        speaker_diarization=FakeAnnotation(
            [
                (0.0, 1.0, "A"),
                (1.0, 4.071, "B"),
                (4.071, 6.0, "A"),
            ]
        ),
        exclusive_speaker_diarization=FakeAnnotation(
            [
                (0.0, 1.2, "A"),
                (1.2, 3.529, "B"),
                (3.529, 6.0, "A"),
            ]
        ),
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

    def test_multi_participant_bounds_preserve_short_regular_speaker(self) -> None:
        whisper = FakeWhisper(
            [
                whisper_segment(
                    0.0,
                    2.0,
                    "확인했습니다. 진행할게요.",
                    [
                        word(0.0, 0.8, " 확인했습니다."),
                        word(1.05, 1.6, " 진행할게요."),
                    ],
                )
            ]
        )
        diarizer = FakeDiarizer(
            FakeDiarization(
                speaker_diarization=FakeAnnotation(
                    [
                        (0.0, 1.0, "A"),
                        (1.0, 1.726, "B"),
                    ]
                ),
                exclusive_speaker_diarization=FakeAnnotation(
                    [
                        (0.0, 2.0, "A"),
                    ]
                ),
            )
        )

        with patch.object(app, "_models", return_value=(whisper, diarizer)):
            result = app._transcribe_file(
                Path("meeting.wav"),
                ["참석자 1", "참석자 2", "참석자 3"],
            )

        self.assertEqual([speaker.key for speaker in result.speakers], ["A", "B"])
        self.assertEqual(
            diarizer.kwargs,
            {
                "min_speakers": 2,
                "max_speakers": 3,
            },
        )

    def test_falls_back_to_regular_when_equal_raw_labels_collapse_after_word_assignment(
        self,
    ) -> None:
        whisper = FakeWhisper(
            [
                whisper_segment(
                    0.0,
                    2.0,
                    "확인했습니다. 진행할게요.",
                    [
                        word(0.0, 0.8, " 확인했습니다."),
                        word(1.05, 1.6, " 진행할게요."),
                    ],
                )
            ]
        )
        diarization = FakeDiarization(
            speaker_diarization=FakeAnnotation(
                [
                    (0.0, 1.0, "A"),
                    (1.0, 1.7, "B"),
                ]
            ),
            exclusive_speaker_diarization=FakeAnnotation(
                [
                    (0.0, 1.8, "A"),
                    (1.8, 2.0, "B"),
                ]
            ),
        )

        with patch.object(
            app,
            "_models",
            return_value=(whisper, FakeDiarizer(diarization)),
        ):
            result = app._transcribe_file(
                Path("meeting.wav"),
                ["참석자 1", "참석자 2", "참석자 3"],
            )

        self.assertEqual([speaker.key for speaker in result.speakers], ["A", "B"])
        self.assertEqual(
            [
                (segment.speaker_key, segment.text)
                for segment in result.segments
            ],
            [
                ("A", "확인했습니다."),
                ("B", "진행할게요."),
            ],
        )

    def test_keeps_exclusive_when_equal_raw_and_attributed_speaker_counts(
        self,
    ) -> None:
        raw_segments = [
            whisper_segment(
                0.0,
                2.0,
                "첫째 둘째 셋째",
                [
                    word(0.0, 0.4, " 첫째"),
                    word(0.85, 1.15, " 둘째"),
                    word(1.4, 1.8, " 셋째"),
                ],
            )
        ]
        diarization = FakeDiarization(
            speaker_diarization=FakeAnnotation(
                [
                    (0.0, 0.7, "A"),
                    (0.7, 2.0, "B"),
                ]
            ),
            exclusive_speaker_diarization=FakeAnnotation(
                [
                    (0.0, 1.0, "A"),
                    (1.0, 2.0, "B"),
                ]
            ),
        )

        selection = app._select_attributed_diarization(
            diarization,
            raw_segments,
        )

        self.assertEqual(selection.diarization.source, "exclusive")
        self.assertEqual(selection.regular_attributed_speaker_count, 2)
        self.assertEqual(selection.exclusive_attributed_speaker_count, 2)
        self.assertEqual(
            [
                (segment.speaker_key, segment.text)
                for segment in selection.result_segments
            ],
            [
                ("A", "첫째 둘째"),
                ("B", "셋째"),
            ],
        )

    def test_recovers_operational_missing_speaker_with_one_clipped_pass(
        self,
    ) -> None:
        whisper = FakeWhisper(
            [
                whisper_segment(
                    0.0,
                    0.8,
                    "확인했습니다.",
                    [word(0.0, 0.8, " 확인했습니다.")],
                )
            ],
            auxiliary_segments=[
                whisper_segment(
                    2.0,
                    2.5,
                    "진행할게요.",
                    [word(2.0, 2.5, " 진행할게요.", 0.8)],
                )
            ],
        )

        with patch.object(
            app,
            "_models",
            return_value=(
                whisper,
                FakeDiarizer(operational_missing_speaker_diarization()),
            ),
        ):
            result = app._transcribe_file(
                Path("meeting.wav"),
                ["참석자 1", "참석자 2", "참석자 3"],
            )

        self.assertEqual(len(whisper.calls), 2)
        self.assertEqual(whisper.calls[0][0], whisper.calls[1][0])
        self.assertTrue(whisper.calls[0][1]["vad_filter"])
        auxiliary_kwargs = whisper.calls[1][1]
        self.assertFalse(auxiliary_kwargs["vad_filter"])
        self.assertFalse(auxiliary_kwargs["condition_on_previous_text"])
        self.assertTrue(auxiliary_kwargs["word_timestamps"])
        self.assertEqual(len(auxiliary_kwargs["clip_timestamps"]), 2)
        self.assertAlmostEqual(auxiliary_kwargs["clip_timestamps"][0], 1.0)
        self.assertAlmostEqual(auxiliary_kwargs["clip_timestamps"][1], 3.729)
        self.assertEqual(
            [speaker.key for speaker in result.speakers],
            ["A", "B"],
        )
        self.assertEqual(
            [(segment.speaker_key, segment.text) for segment in result.segments],
            [("A", "확인했습니다."), ("B", "진행할게요.")],
        )

    def test_recovery_does_not_invent_text_without_core_word_evidence(
        self,
    ) -> None:
        whisper = FakeWhisper(
            [
                whisper_segment(
                    0.0,
                    0.8,
                    "확인했습니다.",
                    [word(0.0, 0.8, " 확인했습니다.")],
                )
            ],
            auxiliary_segments=[
                whisper_segment(1.0, 3.729, "padding hallucination", None),
                whisper_segment(
                    0.91,
                    0.99,
                    "코어 밖",
                    [word(0.91, 0.99, " 코어 밖")],
                ),
            ],
        )

        with patch.object(
            app,
            "_models",
            return_value=(
                whisper,
                FakeDiarizer(operational_missing_speaker_diarization()),
            ),
        ):
            result = app._transcribe_file(Path("meeting.wav"), [])

        self.assertEqual(len(whisper.calls), 2)
        self.assertEqual([speaker.key for speaker in result.speakers], ["A"])
        self.assertNotIn("padding hallucination", result.transcript)
        self.assertNotIn("코어 밖", result.transcript)

    def test_recovery_combines_missing_speakers_and_deduplicates_in_time_order(
        self,
    ) -> None:
        diarization = FakeDiarization(
            speaker_diarization=FakeAnnotation(
                [
                    (0.0, 1.0, "A"),
                    (1.0, 2.1, "B"),
                    (2.5, 3.6, "C"),
                    (3.6, 4.5, "A"),
                ]
            ),
            exclusive_speaker_diarization=FakeAnnotation(
                [
                    (0.0, 1.0, "A"),
                    (1.0, 2.1, "B"),
                    (2.5, 3.6, "C"),
                    (3.6, 4.5, "A"),
                ]
            ),
        )
        whisper = FakeWhisper(
            [
                whisper_segment(
                    0.0,
                    0.6,
                    "첫째",
                    [word(0.0, 0.6, " 첫째")],
                )
            ],
            auxiliary_segments=[
                whisper_segment(
                    1.0,
                    3.6,
                    "셋째 둘째 둘째",
                    [
                        word(2.7, 3.0, " 셋째"),
                        word(1.2, 1.5, " 둘째"),
                        word(1.2, 1.5, " 둘째"),
                    ],
                )
            ],
        )

        with patch.object(
            app,
            "_models",
            return_value=(whisper, FakeDiarizer(diarization)),
        ):
            result = app._transcribe_file(Path("meeting.wav"), [])

        self.assertEqual(len(whisper.calls), 2)
        self.assertEqual(
            [(segment.speaker_key, segment.text) for segment in result.segments],
            [("A", "첫째"), ("B", "둘째"), ("C", "셋째")],
        )
        self.assertEqual(result.transcript.count("둘째"), 1)

    def test_recovery_rejects_primary_time_overlap(self) -> None:
        primary = app.AttributedWord("A", 1.2, 1.5, " 기존", 90, 0)
        recovered = app._accepted_recovery_words(
            [
                whisper_segment(
                    1.2,
                    1.5,
                    "다른 해석",
                    [word(1.2, 1.5, " 다른 해석")],
                )
            ],
            core_tracks=[app.SpeakerTrack(1.0, 2.1, "B")],
            selected_tracks=[app.SpeakerTrack(1.0, 2.1, "B")],
            primary_words=[primary],
        )

        self.assertEqual(recovered, [])

    def test_recovery_requires_minimum_track_evidence(self) -> None:
        fragmented = [
            app.SpeakerTrack(index * 0.3, index * 0.3 + 0.18, "B")
            for index in range(6)
        ]
        two_turns = [
            app.SpeakerTrack(index * 0.5, index * 0.5 + 0.25, "B")
            for index in range(4)
        ]
        standalone = [app.SpeakerTrack(0.0, 1.0, "B")]

        self.assertFalse(app._has_recovery_track_evidence(fragmented))
        self.assertTrue(app._has_recovery_track_evidence(two_turns))
        self.assertTrue(app._has_recovery_track_evidence(standalone))

    def test_recovery_skips_auxiliary_pass_when_clip_budget_is_exceeded(
        self,
    ) -> None:
        whisper = FakeWhisper(
            [
                whisper_segment(
                    0.0,
                    0.8,
                    "확인했습니다.",
                    [word(0.0, 0.8, " 확인했습니다.")],
                )
            ],
            auxiliary_segments=[
                whisper_segment(
                    2.0,
                    2.5,
                    "실행되면 안 됨",
                    [word(2.0, 2.5, " 실행되면 안 됨")],
                )
            ],
        )

        with (
            patch.object(app, "MAX_RECOVERY_CLIP_SECONDS", 1.0),
            patch.object(
                app,
                "_models",
                return_value=(
                    whisper,
                    FakeDiarizer(operational_missing_speaker_diarization()),
                ),
            ),
        ):
            result = app._transcribe_file(Path("meeting.wav"), [])

        self.assertEqual(len(whisper.calls), 1)
        self.assertEqual([speaker.key for speaker in result.speakers], ["A"])

    def test_does_not_force_speaker_bounds_for_zero_or_one_participant(
        self,
    ) -> None:
        for participants in ([], ["참석자 1"]):
            with self.subTest(participants=participants):
                whisper = FakeWhisper(
                    [
                        whisper_segment(
                            0.0,
                            1.0,
                            "확인했습니다.",
                            [word(0.0, 1.0, " 확인했습니다.")],
                        )
                    ]
                )
                diarizer = FakeDiarizer(FakeAnnotation([(0.0, 1.0, "A")]))

                with patch.object(
                    app,
                    "_models",
                    return_value=(whisper, diarizer),
                ):
                    result = app._transcribe_file(
                        Path("meeting.wav"),
                        participants,
                    )

                self.assertEqual(diarizer.kwargs, {})
                self.assertEqual(
                    result.speakers[0].display_name,
                    participants[0] if participants else None,
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

    def test_preserves_same_label_diarization_turns_for_manual_review(self) -> None:
        whisper = FakeWhisper(
            [
                whisper_segment(
                    0.0,
                    4.0,
                    "하나 둘 셋 넷",
                    [
                        word(0.1, 0.6, " 하나"),
                        word(1.1, 1.6, " 둘"),
                        word(2.1, 2.6, " 셋"),
                        word(3.1, 3.6, " 넷"),
                    ],
                )
            ]
        )
        diarizer = FakeDiarizer(
            FakeAnnotation(
                [
                    (0.0, 0.9, "A"),
                    (1.0, 1.9, "A"),
                    (2.0, 2.9, "A"),
                    (3.0, 3.9, "A"),
                ]
            )
        )

        with patch.object(app, "_models", return_value=(whisper, diarizer)):
            result = app._transcribe_file(Path("meeting.wav"), [])

        self.assertEqual(len(result.speakers), 1)
        self.assertEqual(
            [segment.text for segment in result.segments],
            ["하나", "둘", "셋", "넷"],
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

    def test_uses_regular_tracks_when_exclusive_loses_a_speaker(self) -> None:
        diarization = FakeDiarization(
            speaker_diarization=FakeAnnotation(
                [
                    (0.0, 1.0, "A"),
                    (1.0, 2.0, "B"),
                ]
            ),
            exclusive_speaker_diarization=FakeAnnotation(
                [
                    (0.0, 2.0, "A"),
                ]
            ),
        )

        selection = app._select_diarization_tracks(diarization)

        self.assertEqual(selection.source, "regular")
        self.assertEqual(selection.regular_speaker_count, 2)
        self.assertEqual(selection.exclusive_speaker_count, 1)
        self.assertEqual(
            [track.speaker_key for track in selection.tracks],
            ["A", "B"],
        )

    def test_prefers_exclusive_tracks_when_speaker_count_is_preserved(self) -> None:
        diarization = FakeDiarization(
            speaker_diarization=FakeAnnotation(
                [
                    (0.0, 1.0, "A"),
                    (1.0, 2.0, "B"),
                ]
            ),
            exclusive_speaker_diarization=FakeAnnotation(
                [
                    (0.0, 0.8, "A"),
                    (0.8, 2.0, "B"),
                ]
            ),
        )

        selection = app._select_diarization_tracks(diarization)

        self.assertEqual(selection.source, "exclusive")
        self.assertEqual(selection.regular_speaker_count, 2)
        self.assertEqual(selection.exclusive_speaker_count, 2)

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
