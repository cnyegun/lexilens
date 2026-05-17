from __future__ import annotations

import base64
import json
import os
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from typing import Any, Optional

import cv2
import numpy as np


class OCRServiceError(RuntimeError):
    pass


@dataclass(frozen=True)
class OCRResult:
    text: str
    source: str
    raw: Any
    elapsed_seconds: float
    image_width: int
    image_height: int


class OCRService:
    def __init__(
        self,
        model: Optional[str] = None,
        endpoint: Optional[str] = None,
        timeout_seconds: int = 90,
    ) -> None:
        self.model = model or os.getenv("LEXILENS_GLM_OCR_MODEL", "glm-ocr")
        self.endpoint = endpoint or self._default_endpoint()
        self.timeout_seconds = timeout_seconds
        self.max_image_edge = int(os.getenv("LEXILENS_OCR_MAX_IMAGE_EDGE", "960"))
        self.jpeg_quality = int(os.getenv("LEXILENS_OCR_JPEG_QUALITY", "85"))
        self.num_predict = int(os.getenv("LEXILENS_OCR_NUM_PREDICT", "1024"))
        self.num_ctx = int(os.getenv("LEXILENS_OCR_NUM_CTX", "2048"))
        self.keep_alive = os.getenv("LEXILENS_OCR_KEEP_ALIVE", "30m")

    def recognize(self, image: np.ndarray) -> OCRResult:
        if image is None or image.size == 0:
            raise OCRServiceError("OCR cannot run on an empty image.")

        resized = self._resize_for_ocr(image)
        started_at = time.perf_counter()
        payload = {
            "model": self.model,
            "prompt": os.getenv("LEXILENS_GLM_OCR_PROMPT", "Text Recognition:"),
            "images": [self._encode_image(resized)],
            "stream": False,
            "keep_alive": self.keep_alive,
            "options": {
                "temperature": 0,
                "num_predict": self.num_predict,
                "num_ctx": self.num_ctx,
                "stop": ["<|im_end|>", "<|endoftext|>"],
            },
        }
        raw = self._post_json(payload)
        elapsed_seconds = time.perf_counter() - started_at
        text = self._clean_text(str(raw.get("response", "")))
        if not text:
            raise OCRServiceError("GLM-OCR returned no readable text.")
        height, width = resized.shape[:2]
        return OCRResult(
            text=text,
            source="Local GLM-OCR",
            raw=raw,
            elapsed_seconds=elapsed_seconds,
            image_width=width,
            image_height=height,
        )

    @staticmethod
    def _default_endpoint() -> str:
        endpoint = os.getenv("LEXILENS_GLM_OCR_ENDPOINT", "").strip()
        if endpoint:
            return endpoint

        host = os.getenv("OLLAMA_HOST", "http://127.0.0.1:11434").strip().rstrip("/")
        if not host.startswith(("http://", "https://")):
            host = f"http://{host}"
        return f"{host}/api/generate"

    def _resize_for_ocr(self, image: np.ndarray) -> np.ndarray:
        if self.max_image_edge <= 0:
            return image

        height, width = image.shape[:2]
        longest_edge = max(width, height)
        if longest_edge <= self.max_image_edge:
            return image

        scale = self.max_image_edge / longest_edge
        resized_width = max(1, int(width * scale))
        resized_height = max(1, int(height * scale))
        return cv2.resize(image, (resized_width, resized_height), interpolation=cv2.INTER_AREA)

    def _encode_image(self, image: np.ndarray) -> str:
        quality = max(50, min(95, self.jpeg_quality))
        ok, buffer = cv2.imencode(".jpg", image, [int(cv2.IMWRITE_JPEG_QUALITY), quality])
        if not ok:
            raise OCRServiceError("Could not encode selected crop for OCR.")
        return base64.b64encode(buffer.tobytes()).decode("ascii")

    def _post_json(self, payload: dict[str, Any]) -> dict[str, Any]:
        request = urllib.request.Request(
            self.endpoint,
            data=json.dumps(payload).encode("utf-8"),
            method="POST",
            headers={"Content-Type": "application/json"},
        )
        try:
            with urllib.request.urlopen(request, timeout=self.timeout_seconds) as response:
                return json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as exc:
            detail = exc.read().decode("utf-8", errors="replace")
            raise OCRServiceError(f"GLM-OCR HTTP {exc.code}: {detail}") from exc
        except urllib.error.URLError as exc:
            raise OCRServiceError(f"GLM-OCR is not reachable at {self.endpoint}: {exc.reason}") from exc
        except json.JSONDecodeError as exc:
            raise OCRServiceError("GLM-OCR returned invalid JSON.") from exc

    @staticmethod
    def _clean_text(text: str) -> str:
        text = text.strip()
        for token in ("<|im_end|>", "<|endoftext|>", "<|assistant|>"):
            text = text.replace(token, "")
        if text.lower().startswith("text recognition:"):
            text = text.split(":", 1)[1]
        return text.strip()
