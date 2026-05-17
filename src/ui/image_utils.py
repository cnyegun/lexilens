from __future__ import annotations

import cv2
import numpy as np
from PySide6.QtGui import QImage, QPixmap


def ndarray_to_pixmap(image: np.ndarray) -> QPixmap:
    if image.ndim == 2:
        rgb = cv2.cvtColor(image, cv2.COLOR_GRAY2RGB)
    else:
        rgb = cv2.cvtColor(image, cv2.COLOR_BGR2RGB)

    height, width, channels = rgb.shape
    bytes_per_line = channels * width
    qimage = QImage(
        rgb.data,
        width,
        height,
        bytes_per_line,
        QImage.Format.Format_RGB888,
    ).copy()
    return QPixmap.fromImage(qimage)
