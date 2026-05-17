from __future__ import annotations

from typing import Optional

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QDoubleSpinBox,
    QFormLayout,
    QLabel,
    QSlider,
    QSpinBox,
    QVBoxLayout,
    QWidget,
)

from src.config.preferences import UserPreferences


class SettingsDialog(QDialog):
    def __init__(self, preferences: UserPreferences, parent: Optional[QWidget] = None) -> None:
        super().__init__(parent)
        self.setWindowTitle("LexiLens Settings")
        self._preferences = preferences.copy()

        self.font_combo = QComboBox()
        self.font_combo.addItems(["OpenDyslexic", "Verdana", "Arial"])
        self.font_combo.setCurrentText(self._preferences.font_family)

        self.font_size_spin = QSpinBox()
        self.font_size_spin.setRange(14, 48)
        self.font_size_spin.setSuffix(" pt")
        self.font_size_spin.setValue(self._preferences.font_size)

        self.line_spacing_spin = QDoubleSpinBox()
        self.line_spacing_spin.setRange(1.1, 2.6)
        self.line_spacing_spin.setSingleStep(0.1)
        self.line_spacing_spin.setDecimals(1)
        self.line_spacing_spin.setSuffix("x")
        self.line_spacing_spin.setValue(self._preferences.line_spacing)

        self.high_contrast_checkbox = QCheckBox("High contrast")
        self.high_contrast_checkbox.setChecked(self._preferences.high_contrast)

        self.line_focus_checkbox = QCheckBox("Line focus")
        self.line_focus_checkbox.setChecked(self._preferences.line_focus)

        self.auto_read_checkbox = QCheckBox("Read aloud automatically after OCR")
        self.auto_read_checkbox.setChecked(self._preferences.auto_read)

        self.tts_rate_slider = QSlider(Qt.Orientation.Horizontal)
        self.tts_rate_slider.setRange(90, 240)
        self.tts_rate_slider.setSingleStep(5)
        self.tts_rate_slider.setValue(self._preferences.tts_rate)
        self.tts_rate_label = QLabel(str(self._preferences.tts_rate))
        self.tts_rate_slider.valueChanged.connect(lambda value: self.tts_rate_label.setText(str(value)))

        form = QFormLayout()
        form.addRow("Font", self.font_combo)
        form.addRow("Font size", self.font_size_spin)
        form.addRow("Line spacing", self.line_spacing_spin)
        form.addRow(self.high_contrast_checkbox)
        form.addRow(self.line_focus_checkbox)
        form.addRow(self.auto_read_checkbox)
        form.addRow("Speech speed", self.tts_rate_slider)
        form.addRow("Words/min", self.tts_rate_label)

        buttons = QDialogButtonBox(QDialogButtonBox.StandardButton.Cancel | QDialogButtonBox.StandardButton.Save)
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)

        layout = QVBoxLayout(self)
        layout.addLayout(form)
        layout.addWidget(buttons)

    def preferences(self) -> UserPreferences:
        return UserPreferences(
            font_family=self.font_combo.currentText(),
            font_size=self.font_size_spin.value(),
            line_spacing=self.line_spacing_spin.value(),
            high_contrast=self.high_contrast_checkbox.isChecked(),
            line_focus=self.line_focus_checkbox.isChecked(),
            tts_rate=self.tts_rate_slider.value(),
            auto_read=self.auto_read_checkbox.isChecked(),
        ).clamped()
