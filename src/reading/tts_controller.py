from __future__ import annotations

import os
import threading
import warnings
from typing import Optional

import numpy as np
from PySide6.QtCore import QObject, Signal


class TTSController(QObject):
    segment_started = Signal(int)
    segment_finished = Signal(int)
    playback_finished = Signal()
    playback_error = Signal(str)
    state_changed = Signal(str)

    def __init__(self, parent: Optional[QObject] = None) -> None:
        super().__init__(parent)
        self._segments: list[str] = []
        self._rate = 155
        self._current_index = 0
        self._state = "idle"
        self._thread: Optional[threading.Thread] = None
        self._stop_event = threading.Event()
        self._pipeline = None
        self._lock = threading.RLock()

    @property
    def state(self) -> str:
        with self._lock:
            return self._state

    def read(self, segments: list[str], rate: int, start_index: int = 0) -> None:
        clean_segments = [segment.strip() for segment in segments if segment.strip()]
        if not clean_segments:
            self.playback_error.emit("There is no OCR text to read aloud.")
            return

        self.stop(wait=True)
        with self._lock:
            self._segments = clean_segments
            self._rate = rate
            self._current_index = max(0, min(start_index, len(clean_segments) - 1))
            self._stop_event.clear()
            self._set_state_locked("speaking")

        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()

    def stop(self, wait: bool = False) -> None:
        with self._lock:
            self._stop_event.set()
            thread = self._thread
            active = self._state == "speaking"

        try:
            import sounddevice as sd

            sd.stop()
        except Exception:
            pass

        if wait and thread is not None and thread.is_alive() and thread is not threading.current_thread():
            thread.join(timeout=1.5)
        if active:
            with self._lock:
                self._current_index = 0
                self._set_state_locked("idle")

    def _run(self) -> None:
        try:
            import sounddevice as sd
            from kokoro import KPipeline
        except ImportError:
            self._fail("Kokoro TTS is not installed. Install dependencies with `pip install -r requirements.txt`.")
            return

        try:
            pipeline = self._load_pipeline(KPipeline)
            voice = os.getenv("LEXILENS_KOKORO_VOICE", "af_heart")
            speed = self._rate_to_speed(self._rate)

            with self._lock:
                start_index = self._current_index
                segments = list(self._segments)

            for index in range(start_index, len(segments)):
                if self._stop_event.is_set():
                    break

                with self._lock:
                    self._current_index = index
                self.segment_started.emit(index)

                generator = pipeline(segments[index], voice=voice, speed=speed, split_pattern=r"\n+")
                for _, _, audio in generator:
                    if self._stop_event.is_set():
                        break
                    sd.play(self._to_numpy_audio(audio), samplerate=24000, blocking=True)

                self.segment_finished.emit(index)

            stopped = self._stop_event.is_set()
            with self._lock:
                self._set_state_locked("idle")

            if not stopped:
                self.playback_finished.emit()
        except Exception as exc:
            self._fail(f"Kokoro text-to-speech failed: {exc}")

    def _load_pipeline(self, pipeline_cls):
        with self._lock:
            if self._pipeline is not None:
                return self._pipeline

        lang_code = os.getenv("LEXILENS_KOKORO_LANG", "a")
        with warnings.catch_warnings():
            warnings.filterwarnings("ignore", message="dropout option adds dropout.*")
            warnings.filterwarnings("ignore", message="`torch.nn.utils.weight_norm` is deprecated.*")
            pipeline = pipeline_cls(lang_code=lang_code, repo_id="hexgrad/Kokoro-82M")
        with self._lock:
            self._pipeline = pipeline
        return pipeline

    @staticmethod
    def _rate_to_speed(rate: int) -> float:
        return max(0.65, min(1.55, rate / 155.0))

    @staticmethod
    def _to_numpy_audio(audio) -> np.ndarray:
        if hasattr(audio, "detach"):
            audio = audio.detach().cpu().numpy()
        return np.asarray(audio, dtype=np.float32)

    def _fail(self, message: str) -> None:
        with self._lock:
            self._set_state_locked("idle")
        self.playback_error.emit(message)

    def _set_state_locked(self, state: str) -> None:
        if self._state == state:
            return
        self._state = state
        self.state_changed.emit(state)
