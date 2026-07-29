import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from fastapi import HTTPException

import app


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


if __name__ == "__main__":
    unittest.main()
