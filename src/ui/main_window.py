from __future__ import annotations

from pathlib import Path
from typing import Optional

import numpy as np
from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import QDialog, QLabel, QMainWindow, QSplitter, QVBoxLayout, QWidget

from src.config.preferences import UserPreferences
from src.ui.controls import ControlsPanel
from src.ui.frame_viewer import FrameViewer
from src.ui.readable_panel import AccessibleTextRenderer
from src.ui.settings_dialog import SettingsDialog
from src.vision.ocr_service import OCRResult


class MainWindow(QMainWindow):
    preferences_changed = Signal(object)

    def __init__(self, project_root: Path, parent: Optional[QWidget] = None) -> None:
        super().__init__(parent)
        self.project_root = project_root
        self.setWindowTitle("LexiLens")
        self._mode = "live"
        self._preferences = UserPreferences()

        self.status_label = QLabel("Starting camera...")
        self.status_label.setWordWrap(True)
        self.status_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.status_label.setStyleSheet("font-size: 18px; font-weight: 700; color: #111827;")

        self.detail_label = QLabel("")
        self.detail_label.setWordWrap(True)
        self.detail_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.detail_label.setStyleSheet("font-size: 12px; color: #6b7280;")

        self.frame_viewer = FrameViewer()
        self.readable_panel = AccessibleTextRenderer()
        font_path = project_root / "assets" / "fonts" / "OpenDyslexic-Regular.otf"
        self._open_dyslexic_loaded = self.readable_panel.load_open_dyslexic(font_path)
        if not self._open_dyslexic_loaded:
            self.detail_label.setText("Using fallback font. Add OpenDyslexic to assets/fonts for the preferred demo font.")

        self.controls = ControlsPanel()
        self.controls.settings_requested.connect(self._show_settings)

        self._build_ui()
        self.set_mode("live")

    @property
    def mode(self) -> str:
        return self._mode

    def _build_ui(self) -> None:
        root = QWidget()
        layout = QVBoxLayout(root)
        layout.setContentsMargins(14, 14, 14, 14)
        layout.setSpacing(10)

        splitter = QSplitter(Qt.Orientation.Vertical)
        splitter.addWidget(self.frame_viewer)
        splitter.addWidget(self.readable_panel)
        splitter.setStretchFactor(0, 3)
        splitter.setStretchFactor(1, 2)

        layout.addWidget(self.status_label)
        layout.addWidget(self.detail_label)
        layout.addWidget(splitter, stretch=1)
        layout.addWidget(self.controls)
        self.setCentralWidget(root)

    def apply_preferences(self, preferences: UserPreferences) -> None:
        self._preferences = preferences.copy()
        self.readable_panel.set_preferences(preferences)

    def show_frame(self, frame: np.ndarray) -> None:
        self.frame_viewer.set_frame(frame)

    def freeze_frame(self, frame: np.ndarray) -> None:
        self.frame_viewer.freeze_on_frame(frame)

    def set_status(self, message: str, error: bool = False) -> None:
        self.status_label.setText(message)
        color = "#b91c1c" if error else "#111827"
        self.status_label.setStyleSheet(f"font-size: 18px; font-weight: 700; color: {color};")

    def set_mode(self, mode: str) -> None:
        self._mode = mode
        self.controls.set_mode(mode)

    def set_ocr_busy(self) -> None:
        self.set_mode("processing")
        self.set_status("Reading the selected text...")
        self.detail_label.setText("Hold still. LexiLens is extracting the text block.")

    def set_ocr_result(self, result: OCRResult, segments: list[str]) -> None:
        self.set_mode("ready")
        self.readable_panel.set_segments(segments)
        self.set_status("Text ready.")
        self.detail_label.setText(
            f"OCR: {result.source} | {result.elapsed_seconds:.1f}s | {result.image_width}x{result.image_height}"
        )

    def show_ocr_failure(self, message: str) -> None:
        self.set_mode("failed")
        self.set_status("Could not read that selection.", error=True)
        self.detail_label.setText(message)

    def clear_all(self) -> None:
        self.frame_viewer.clear_selection()
        self.frame_viewer.unfreeze()
        self.readable_panel.clear()
        self.set_mode("live")
        self.set_status("Draw around the text you want to read.")
        if self._open_dyslexic_loaded:
            self.detail_label.setText("")
        else:
            self.detail_label.setText("Using fallback font. Add OpenDyslexic to assets/fonts for the preferred demo font.")

    def highlight_segment(self, index: int) -> None:
        self.readable_panel.highlight_segment(index)

    def _show_settings(self) -> None:
        dialog = SettingsDialog(self._preferences, self)
        if dialog.exec() == QDialog.DialogCode.Accepted:
            self.preferences_changed.emit(dialog.preferences())
