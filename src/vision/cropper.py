from __future__ import annotations

import numpy as np
from PySide6.QtCore import QRect


class ImageCropper:
    def crop(self, frame: np.ndarray, rect: QRect, min_size: int = 12) -> np.ndarray:
        if frame is None or frame.size == 0:
            raise ValueError("No camera frame is available to crop.")

        frame_height, frame_width = frame.shape[:2]
        x1 = max(0, rect.x())
        y1 = max(0, rect.y())
        x2 = min(frame_width, rect.x() + rect.width())
        y2 = min(frame_height, rect.y() + rect.height())

        if x2 - x1 < min_size or y2 - y1 < min_size:
            raise ValueError("Selected area is too small. Draw a larger rectangle around the text block.")

        return frame[y1:y2, x1:x2].copy()
