from __future__ import annotations

from typing import Optional

from PySide6.QtCore import QObject, Slot


class HighlightSynchronizer(QObject):
    def __init__(self, renderer, parent: Optional[QObject] = None) -> None:
        super().__init__(parent)
        self._renderer = renderer

    @Slot(int)
    def highlight(self, index: int) -> None:
        self._renderer.highlight_segment(index)

    @Slot()
    def clear(self) -> None:
        self._renderer.highlight_segment(-1)
