from __future__ import annotations

import json
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Optional


@dataclass
class UserPreferences:
    font_family: str = "OpenDyslexic"
    font_size: int = 24
    line_spacing: float = 1.6
    high_contrast: bool = False
    line_focus: bool = True
    tts_rate: int = 155
    auto_read: bool = True

    def copy(self) -> "UserPreferences":
        return UserPreferences(**asdict(self))

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "UserPreferences":
        defaults = cls()
        return cls(
            font_family=str(data.get("font_family", defaults.font_family)),
            font_size=int(data.get("font_size", defaults.font_size)),
            line_spacing=float(data.get("line_spacing", defaults.line_spacing)),
            high_contrast=bool(data.get("high_contrast", defaults.high_contrast)),
            line_focus=bool(data.get("line_focus", defaults.line_focus)),
            tts_rate=int(data.get("tts_rate", defaults.tts_rate)),
            auto_read=bool(data.get("auto_read", defaults.auto_read)),
        ).clamped()

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)

    def clamped(self) -> "UserPreferences":
        self.font_family = self.font_family if self.font_family in {"OpenDyslexic", "Arial", "Verdana"} else "Arial"
        self.font_size = max(14, min(48, int(self.font_size)))
        self.line_spacing = max(1.1, min(2.6, float(self.line_spacing)))
        self.tts_rate = max(90, min(240, int(self.tts_rate)))
        return self


class UserPreferencesStore:
    def __init__(self, path: Optional[Path] = None) -> None:
        self.path = path or (Path.home() / ".lexilens" / "preferences.json")

    def load(self) -> UserPreferences:
        if not self.path.exists():
            return UserPreferences()
        try:
            data = json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return UserPreferences()
        if not isinstance(data, dict):
            return UserPreferences()
        return UserPreferences.from_dict(data)

    def save(self, preferences: UserPreferences) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(
            json.dumps(preferences.clamped().to_dict(), indent=2, sort_keys=True),
            encoding="utf-8",
        )
