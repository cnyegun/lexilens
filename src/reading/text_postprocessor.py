from __future__ import annotations

import re
import unicodedata


class TextPostProcessor:
    def clean(self, text: str) -> str:
        if not text:
            return ""

        normalized = unicodedata.normalize("NFKC", text)
        normalized = normalized.replace("\r\n", "\n").replace("\r", "\n")
        normalized = re.sub(r"-\n(?=\w)", "", normalized)

        lines = []
        for line in normalized.split("\n"):
            line = re.sub(r"[ \t]+", " ", line).strip()
            line = re.sub(r"\s+([,.;:!?])", r"\1", line)
            if line:
                lines.append(line)
        return "\n".join(lines)

    def split_segments(self, text: str) -> list[str]:
        cleaned = self.clean(text)
        if not cleaned:
            return []

        lines = [line.strip() for line in cleaned.split("\n") if line.strip()]
        if len(lines) > 1:
            return lines

        sentence_parts = re.split(r"(?<=[.!?])\s+", cleaned)
        segments = [part.strip() for part in sentence_parts if part.strip()]
        if len(segments) > 1:
            return segments

        return self._chunk_long_text(cleaned)

    @staticmethod
    def _chunk_long_text(text: str, max_words: int = 14) -> list[str]:
        words = text.split()
        if len(words) <= max_words:
            return [text]
        return [" ".join(words[index : index + max_words]) for index in range(0, len(words), max_words)]
