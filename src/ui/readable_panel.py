from __future__ import annotations

import html
from pathlib import Path
from typing import Optional

from PySide6.QtGui import QFontDatabase
from PySide6.QtWidgets import QTextBrowser, QVBoxLayout, QWidget

from src.config.preferences import UserPreferences


class AccessibleTextRenderer(QWidget):
    def __init__(self, parent: Optional[QWidget] = None) -> None:
        super().__init__(parent)
        self._preferences = UserPreferences()
        self._segments: list[str] = []
        self._current_index = -1
        self._open_dyslexic_family: Optional[str] = None

        self._browser = QTextBrowser(self)
        self._browser.setOpenExternalLinks(False)
        self._browser.setMinimumHeight(320)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.addWidget(self._browser)
        self._render()

    @property
    def open_dyslexic_available(self) -> bool:
        return self._open_dyslexic_family is not None

    def load_open_dyslexic(self, font_path: Path) -> bool:
        if not font_path.exists():
            return False
        font_id = QFontDatabase.addApplicationFont(str(font_path))
        if font_id < 0:
            return False
        families = QFontDatabase.applicationFontFamilies(font_id)
        if not families:
            return False
        self._open_dyslexic_family = families[0]
        self._render()
        return True

    def set_preferences(self, preferences: UserPreferences) -> None:
        self._preferences = preferences.copy()
        self._render()

    def set_segments(self, segments: list[str]) -> None:
        self._segments = list(segments)
        self._current_index = -1
        self._render()

    def clear(self) -> None:
        self._segments = []
        self._current_index = -1
        self._render()

    def highlight_segment(self, index: int) -> None:
        if index < 0 or index >= len(self._segments):
            self._current_index = -1
        else:
            self._current_index = index
        self._render()

    def _render(self) -> None:
        prefs = self._preferences
        high_contrast = prefs.high_contrast
        background = "#000000" if high_contrast else "#fbf7df"
        foreground = "#ffffff" if high_contrast else "#1f2933"
        muted = "#777777" if high_contrast else "#697386"
        highlight_bg = "#ffd54f" if high_contrast else "#fff3a3"
        highlight_fg = "#000000" if high_contrast else "#101820"
        border = "#00d4ff" if high_contrast else "#2563eb"
        font_family = self._effective_font_family()

        if not self._segments:
            body = (
                '<p class="placeholder">Draw a rectangle over printed text in the camera preview. '
                "LexiLens will OCR the crop and show it here in a personalized readability view.</p>"
            )
        else:
            rows = []
            for index, segment in enumerate(self._segments):
                classes = ["segment"]
                if index == self._current_index:
                    classes.append("current")
                elif prefs.line_focus and self._current_index >= 0:
                    classes.append("dim")
                rows.append(
                    f'<div class="{" ".join(classes)}" data-index="{index}">{html.escape(segment)}</div>'
                )
            body = "\n".join(rows)

        document = f"""
        <!doctype html>
        <html>
        <head>
        <style>
            body {{
                margin: 0;
                padding: 18px;
                background: {background};
                color: {foreground};
                font-family: {font_family};
                font-size: {prefs.font_size}pt;
                line-height: {prefs.line_spacing};
                word-spacing: 0.22em;
            }}
            .segment {{
                margin: 0 0 12px 0;
                padding: 8px 12px;
                border-left: 6px solid transparent;
                border-radius: 8px;
            }}
            .current {{
                background: {highlight_bg};
                color: {highlight_fg};
                border-left-color: {border};
                font-weight: 700;
            }}
            .dim {{
                color: {muted};
            }}
            .placeholder {{
                color: {muted};
                font-size: {max(16, prefs.font_size - 4)}pt;
            }}
        </style>
        </head>
        <body>{body}</body>
        </html>
        """
        self._browser.setHtml(document)
        self._browser.setStyleSheet(f"QTextBrowser {{ background: {background}; border: 1px solid #3a4553; }}")

    def _effective_font_family(self) -> str:
        requested = self._preferences.font_family
        if requested == "OpenDyslexic" and self._open_dyslexic_family:
            return f"'{self._open_dyslexic_family}', Arial, Verdana, sans-serif"
        if requested in {"Arial", "Verdana"}:
            return f"'{requested}', Arial, Verdana, sans-serif"
        return "Arial, Verdana, sans-serif"
