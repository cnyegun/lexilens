from __future__ import annotations

from typing import Optional

from PySide6.QtCore import Signal
from PySide6.QtWidgets import QHBoxLayout, QPushButton, QWidget


class ControlsPanel(QWidget):
    primary_requested = Signal()
    new_selection_requested = Signal()
    settings_requested = Signal()

    def __init__(self, parent: Optional[QWidget] = None) -> None:
        super().__init__(parent)
        self.primary_button = QPushButton("Read Again")
        self.new_selection_button = QPushButton("New Selection")
        self.settings_button = QPushButton("Settings")

        self.primary_button.clicked.connect(self.primary_requested.emit)
        self.new_selection_button.clicked.connect(self.new_selection_requested.emit)
        self.settings_button.clicked.connect(self.settings_requested.emit)

        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.addStretch(1)
        layout.addWidget(self.primary_button)
        layout.addWidget(self.new_selection_button)
        layout.addWidget(self.settings_button)

        self.set_mode("live")

    def set_mode(self, mode: str) -> None:
        if mode == "live":
            self.primary_button.hide()
            self.new_selection_button.hide()
        elif mode == "processing":
            self.primary_button.hide()
            self.new_selection_button.show()
            self.new_selection_button.setText("Cancel")
        elif mode == "reading":
            self.primary_button.show()
            self.primary_button.setText("Stop")
            self.new_selection_button.show()
            self.new_selection_button.setText("New Selection")
        elif mode == "failed":
            self.primary_button.show()
            self.primary_button.setText("Try Again")
            self.new_selection_button.show()
            self.new_selection_button.setText("New Selection")
        else:
            self.primary_button.show()
            self.primary_button.setText("Read Again")
            self.new_selection_button.show()
            self.new_selection_button.setText("New Selection")

        self.settings_button.show()
