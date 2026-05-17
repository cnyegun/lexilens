from __future__ import annotations

from typing import Optional

import numpy as np
from PySide6.QtCore import QPoint, QRect, Qt, Signal
from PySide6.QtGui import QColor, QFont, QPainter, QPen, QPixmap
from PySide6.QtWidgets import QWidget

from src.selection.rectangle_selector import RectangleSelector
from src.ui.image_utils import ndarray_to_pixmap


class FrameViewer(QWidget):
    selection_completed = Signal(object)
    selection_changed = Signal(object)

    def __init__(self, parent: Optional[QWidget] = None) -> None:
        super().__init__(parent)
        self.setMinimumSize(640, 480)
        self.setMouseTracking(True)
        self._selector = RectangleSelector()
        self._frame: Optional[np.ndarray] = None
        self._pixmap: Optional[QPixmap] = None
        self._frozen = False

    def set_frame(self, frame: np.ndarray) -> None:
        if self._frozen:
            return
        self._set_display_frame(frame)

    def freeze_on_frame(self, frame: np.ndarray) -> None:
        self._frozen = True
        self._set_display_frame(frame)

    def unfreeze(self) -> None:
        self._frozen = False
        self.update()

    def clear_selection(self) -> None:
        self._selector.clear()
        self.update()

    def _set_display_frame(self, frame: np.ndarray) -> None:
        self._frame = frame.copy()
        self._pixmap = ndarray_to_pixmap(self._frame)
        self.update()

    def paintEvent(self, event) -> None:  # noqa: N802 - Qt override
        painter = QPainter(self)
        painter.fillRect(self.rect(), QColor("#101820"))

        if self._pixmap is None:
            painter.setPen(QColor("#d8dee9"))
            painter.setFont(QFont("Arial", 16))
            painter.drawText(self.rect(), Qt.AlignmentFlag.AlignCenter, "Waiting for webcam...")
            return

        target = self._display_rect()
        painter.drawPixmap(target, self._pixmap)

        selection = self._selector.rect
        if selection is not None:
            widget_rect = self._image_rect_to_widget(selection)
            painter.setRenderHint(QPainter.RenderHint.Antialiasing, True)
            painter.fillRect(widget_rect, QColor(70, 130, 180, 45))
            pen = QPen(QColor("#00d4ff"), 3)
            painter.setPen(pen)
            painter.drawRect(widget_rect)

        if self._frozen:
            badge = QRect(target.left() + 12, target.top() + 12, 120, 32)
            painter.fillRect(badge, QColor(0, 0, 0, 170))
            painter.setPen(QColor("#ffffff"))
            painter.setFont(QFont("Arial", 11, QFont.Weight.Bold))
            painter.drawText(badge, Qt.AlignmentFlag.AlignCenter, "Captured")

    def mousePressEvent(self, event) -> None:  # noqa: N802 - Qt override
        if event.button() != Qt.MouseButton.LeftButton or self._frame is None or self._frozen:
            return
        image_point = self._widget_to_image_point(event.position().toPoint())
        if image_point is None:
            return
        self._selector.begin(image_point)
        self.selection_changed.emit(self._selector.rect)
        self.update()

    def mouseMoveEvent(self, event) -> None:  # noqa: N802 - Qt override
        if not self._selector.active:
            return
        image_point = self._widget_to_image_point(event.position().toPoint())
        if image_point is None:
            return
        rect = self._selector.update(image_point)
        self.selection_changed.emit(rect)
        self.update()

    def mouseReleaseEvent(self, event) -> None:  # noqa: N802 - Qt override
        if event.button() != Qt.MouseButton.LeftButton or not self._selector.active:
            return
        image_point = self._widget_to_image_point(event.position().toPoint())
        if image_point is not None:
            self._selector.update(image_point)
        rect = self._selector.finish()
        self.update()
        if rect is not None:
            self.selection_completed.emit(rect)

    def _display_rect(self) -> QRect:
        if self._pixmap is None:
            return QRect()
        widget_width = max(1, self.width())
        widget_height = max(1, self.height())
        pixmap_width = self._pixmap.width()
        pixmap_height = self._pixmap.height()
        scale = min(widget_width / pixmap_width, widget_height / pixmap_height)
        target_width = int(pixmap_width * scale)
        target_height = int(pixmap_height * scale)
        left = (widget_width - target_width) // 2
        top = (widget_height - target_height) // 2
        return QRect(left, top, target_width, target_height)

    def _widget_to_image_point(self, point: QPoint) -> Optional[QPoint]:
        if self._frame is None:
            return None
        target = self._display_rect()
        if target.isNull() or not target.contains(point):
            return None

        frame_height, frame_width = self._frame.shape[:2]
        x_ratio = (point.x() - target.left()) / max(1, target.width())
        y_ratio = (point.y() - target.top()) / max(1, target.height())
        x = min(frame_width - 1, max(0, int(x_ratio * frame_width)))
        y = min(frame_height - 1, max(0, int(y_ratio * frame_height)))
        return QPoint(x, y)

    def _image_rect_to_widget(self, rect: QRect) -> QRect:
        if self._frame is None:
            return QRect()
        target = self._display_rect()
        frame_height, frame_width = self._frame.shape[:2]
        x_scale = target.width() / max(1, frame_width)
        y_scale = target.height() / max(1, frame_height)
        left = target.left() + int(rect.left() * x_scale)
        top = target.top() + int(rect.top() * y_scale)
        width = int(rect.width() * x_scale)
        height = int(rect.height() * y_scale)
        return QRect(left, top, width, height)
