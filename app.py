from __future__ import annotations

import sys
from pathlib import Path

from PySide6.QtWidgets import QApplication

from src.core.app_controller import AppController
from src.ui.main_window import MainWindow


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationName("LexiLens")
    app.setOrganizationName("LexiLens")

    project_root = Path(__file__).resolve().parent
    window = MainWindow(project_root=project_root)
    controller = AppController(window=window)

    window.resize(1320, 820)
    window.show()
    exit_code = app.exec()
    controller.shutdown()
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
