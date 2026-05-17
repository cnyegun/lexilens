from __future__ import annotations

import platform
from typing import Optional

import cv2
import numpy as np
from PySide6.QtCore import QObject, QTimer, Signal


class WebcamController(QObject):
    frame_ready = Signal(object)
    camera_error = Signal(str)

    def __init__(
        self,
        camera_index: int = 0,
        width: int = 1280,
        height: int = 720,
        fps: int = 30,
        parent: Optional[QObject] = None,
    ) -> None:
        super().__init__(parent)
        self.camera_index = camera_index
        self.width = width
        self.height = height
        self.fps = fps
        self._capture: Optional[cv2.VideoCapture] = None
        self._timer = QTimer(self)
        self._timer.timeout.connect(self._read_frame)
        self._current_frame: Optional[np.ndarray] = None
        self._failure_count = 0

    @property
    def is_running(self) -> bool:
        return self._capture is not None and self._timer.isActive()

    def start(self) -> bool:
        if self.is_running:
            return True

        backend = cv2.CAP_DSHOW if platform.system() == "Windows" else cv2.CAP_ANY
        capture = cv2.VideoCapture(self.camera_index, backend)
        if not capture.isOpened():
            self.camera_error.emit(
                "Unable to open the laptop camera. Check camera permissions or choose another camera index."
            )
            capture.release()
            return False

        capture.set(cv2.CAP_PROP_FRAME_WIDTH, self.width)
        capture.set(cv2.CAP_PROP_FRAME_HEIGHT, self.height)
        capture.set(cv2.CAP_PROP_FPS, self.fps)

        self._capture = capture
        self._failure_count = 0
        interval_ms = max(1, int(1000 / max(1, self.fps)))
        self._timer.start(interval_ms)
        return True

    def stop(self) -> None:
        self._timer.stop()
        if self._capture is not None:
            self._capture.release()
        self._capture = None
        self._current_frame = None

    def snapshot(self) -> Optional[np.ndarray]:
        if self._current_frame is None:
            return None
        return self._current_frame.copy()

    def _read_frame(self) -> None:
        if self._capture is None:
            return

        ok, frame = self._capture.read()
        if not ok or frame is None:
            self._failure_count += 1
            if self._failure_count >= 5:
                self.camera_error.emit("Camera stream stopped returning frames.")
                self.stop()
            return

        self._failure_count = 0
        self._current_frame = frame.copy()
        self.frame_ready.emit(self._current_frame)
