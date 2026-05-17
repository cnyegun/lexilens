from __future__ import annotations

from typing import Optional

from PySide6.QtCore import QPoint, QRect


class RectangleSelector:
    def __init__(self, min_size: int = 12) -> None:
        self.min_size = min_size
        self._start: Optional[QPoint] = None
        self._current: Optional[QPoint] = None
        self._active = False

    @property
    def active(self) -> bool:
        return self._active

    @property
    def rect(self) -> Optional[QRect]:
        if self._start is None or self._current is None:
            return None
        return self._normalized_rect(self._start, self._current)

    def begin(self, point: QPoint) -> None:
        self._start = QPoint(point)
        self._current = QPoint(point)
        self._active = True

    def update(self, point: QPoint) -> Optional[QRect]:
        if not self._active:
            return self.rect
        self._current = QPoint(point)
        return self.rect

    def finish(self) -> Optional[QRect]:
        self._active = False
        rect = self.rect
        if rect is None or rect.width() < self.min_size or rect.height() < self.min_size:
            self.clear()
            return None
        return rect

    def clear(self) -> None:
        self._start = None
        self._current = None
        self._active = False

    @staticmethod
    def _normalized_rect(start: QPoint, current: QPoint) -> QRect:
        left = min(start.x(), current.x())
        top = min(start.y(), current.y())
        right = max(start.x(), current.x())
        bottom = max(start.y(), current.y())
        return QRect(left, top, right - left, bottom - top)
