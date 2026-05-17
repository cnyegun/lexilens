from __future__ import annotations

import threading
from typing import Optional

import numpy as np
from PySide6.QtCore import QObject, Signal

from src.camera.webcam_controller import WebcamController
from src.config.preferences import UserPreferences, UserPreferencesStore
from src.reading.highlight_sync import HighlightSynchronizer
from src.reading.text_postprocessor import TextPostProcessor
from src.reading.tts_controller import TTSController
from src.ui.main_window import MainWindow
from src.vision.cropper import ImageCropper
from src.vision.ocr_service import OCRService, OCRServiceError


class _OCRSignals(QObject):
    result_ready = Signal(object)
    failed = Signal(object)


class AppController(QObject):
    def __init__(self, window: MainWindow, parent: Optional[QObject] = None) -> None:
        super().__init__(parent)
        self.window = window
        self.preferences_store = UserPreferencesStore()
        self.preferences = self.preferences_store.load()

        self.webcam = WebcamController()
        self.cropper = ImageCropper()
        self.ocr_service = OCRService()
        self.postprocessor = TextPostProcessor()
        self.tts = TTSController()
        self.highlight_sync = HighlightSynchronizer(self.window.readable_panel)
        self.ocr_signals = _OCRSignals()

        self._last_crop: Optional[np.ndarray] = None
        self._segments: list[str] = []
        self._ocr_running = False
        self._ocr_request_id = 0

        self.window.apply_preferences(self.preferences)
        self._connect_signals()
        if self.webcam.start():
            self.window.set_status("Camera ready. Draw a rectangle around printed text.")

    def shutdown(self) -> None:
        self.tts.stop(wait=True)
        self.webcam.stop()

    def _connect_signals(self) -> None:
        self.webcam.frame_ready.connect(self.window.show_frame)
        self.webcam.camera_error.connect(lambda message: self.window.set_status(message, error=True))

        self.window.frame_viewer.selection_completed.connect(self._handle_selection)
        controls = self.window.controls
        controls.primary_requested.connect(self._handle_primary_action)
        controls.new_selection_requested.connect(self._clear_selection)
        self.window.preferences_changed.connect(self._set_preferences)

        self.ocr_signals.result_ready.connect(self._handle_ocr_result)
        self.ocr_signals.failed.connect(self._handle_ocr_failure)

        self.tts.segment_started.connect(self.highlight_sync.highlight)
        self.tts.playback_finished.connect(self._handle_tts_finished)
        self.tts.playback_error.connect(lambda message: self.window.set_status(message, error=True))
        self.tts.state_changed.connect(self._handle_tts_state)

    def _handle_selection(self, rect) -> None:
        if self._ocr_running:
            self.window.set_status("OCR is already running. Wait for it to finish before selecting another region.")
            return

        frame = self.webcam.snapshot()
        if frame is None:
            self.window.set_status("No camera frame is available yet. Wait for the preview to start.", error=True)
            return

        try:
            self.tts.stop(wait=False)
            self.window.freeze_frame(frame)
            crop = self.cropper.crop(frame, rect)
            self._last_crop = crop
            self._run_ocr(crop)
        except Exception as exc:
            self.window.show_ocr_failure(str(exc))

    def _run_ocr(self, image: np.ndarray) -> None:
        if self._ocr_running:
            self.window.set_status("OCR is already running. Wait for it to finish before retrying.")
            return

        self._ocr_running = True
        self._ocr_request_id += 1
        request_id = self._ocr_request_id
        self.window.set_ocr_busy()
        height, width = image.shape[:2]
        self.window.detail_label.setText(f"Sending {width}x{height} crop to local GLM-OCR...")

        def worker() -> None:
            try:
                result = self.ocr_service.recognize(image)
            except OCRServiceError as exc:
                self.ocr_signals.failed.emit((request_id, str(exc)))
            except Exception as exc:
                self.ocr_signals.failed.emit((request_id, f"Unexpected OCR error: {exc}"))
            else:
                self.ocr_signals.result_ready.emit((request_id, result))

        threading.Thread(target=worker, daemon=True).start()

    def _handle_ocr_result(self, payload) -> None:
        request_id, result = payload
        if request_id != self._ocr_request_id:
            return

        self._ocr_running = False
        cleaned_text = self.postprocessor.clean(result.text)
        segments = self.postprocessor.split_segments(cleaned_text)
        self._segments = segments
        if not cleaned_text or not segments:
            self.window.show_ocr_failure(
                "OCR did not find readable text in the selected crop. Try a larger, sharper selection."
            )
            return
        self.window.set_ocr_result(result, segments)
        if self.preferences.auto_read:
            self._read_aloud()

    def _handle_ocr_failure(self, payload) -> None:
        request_id, message = payload
        if request_id != self._ocr_request_id:
            return

        self._ocr_running = False
        self.window.show_ocr_failure(message)

    def _rerun_ocr(self) -> None:
        if self._last_crop is None:
            self.window.set_status("Select a text region before re-running OCR.", error=True)
            return
        self.tts.stop(wait=False)
        self._run_ocr(self._last_crop)

    def _clear_selection(self) -> None:
        self.tts.stop(wait=False)
        self.highlight_sync.clear()
        if self._ocr_running:
            self._ocr_request_id += 1
            self._ocr_running = False
        self._last_crop = None
        self._segments = []
        self.window.clear_all()

    def _read_aloud(self) -> None:
        if not self._segments:
            self.window.set_status("There is no OCR text to read yet. Select a text block first.", error=True)
            return
        self.tts.read(self._segments, self.preferences.tts_rate)

    def _handle_primary_action(self) -> None:
        mode = self.window.mode
        if mode == "reading":
            self._stop_reading()
        elif mode == "failed":
            self._rerun_ocr()
        elif mode == "ready":
            self._read_aloud()

    def _stop_reading(self) -> None:
        self.tts.stop(wait=False)
        self.highlight_sync.clear()
        self.window.set_mode("ready" if self._segments else "live")
        self.window.set_status("Read-aloud stopped.")

    def _handle_tts_state(self, state: str) -> None:
        if state == "speaking":
            self.window.set_mode("reading")
            self.window.set_status("Reading aloud with synchronized line highlighting...")

    def _handle_tts_finished(self) -> None:
        self.highlight_sync.clear()
        self.window.set_mode("ready")
        self.window.set_status("Read-aloud finished.")

    def _set_preferences(self, preferences: UserPreferences) -> None:
        self.preferences = preferences
        self._preferences_changed()

    def _preferences_changed(self) -> None:
        self.preferences = self.preferences.clamped()
        self.window.apply_preferences(self.preferences)
        try:
            self.preferences_store.save(self.preferences)
        except OSError as exc:
            self.window.set_status(f"Could not save preferences: {exc}", error=True)
