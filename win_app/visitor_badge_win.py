"""The visitor badge on Windows: a tag in the driving machine's colour that follows the pointer while
another machine has input on this PC. What it says, its colour and where it sits come from
core.visitor.

A small frameless, click-through, topmost Qt window, moved on the frame timer to wherever the
pointer is. GUI thread only; the app calls `show_badge` from its owner signal.

Windows only for now: on Wayland a window cannot place itself or read the pointer, and X11 waits for
hardware testing, as Beamer's other overlays did.
"""

from __future__ import annotations

import sys

from PySide6.QtCore import QRectF, Qt
from PySide6.QtGui import QColor, QCursor, QFont, QFontMetrics, QGuiApplication, QPainter
from PySide6.QtWidgets import QWidget

from core import visitor
from edge_glow import keep_on_top
from frame_clock import frame_timer

SUPPORTED = sys.platform == "win32"


class VisitorBadge(QWidget):
    def __init__(self) -> None:
        super().__init__(
            None,
            Qt.WindowType.Tool
            | Qt.WindowType.FramelessWindowHint
            | Qt.WindowType.WindowStaysOnTopHint
            | Qt.WindowType.WindowTransparentForInput
            | Qt.WindowType.WindowDoesNotAcceptFocus,
        )
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setAttribute(Qt.WidgetAttribute.WA_ShowWithoutActivating)
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        self._badge: visitor.Badge | None = None
        self._font = QFont()
        self._font.setPixelSize(visitor.FONT_PX)
        self._font.setBold(True)
        self._timer = frame_timer(self, self._follow, self)

    def show_badge(self, badge: visitor.Badge | None) -> None:
        """Show `badge` at the pointer, update the one shown, or hide it when `badge` is None."""
        if badge is None or not SUPPORTED:
            self._badge = None
            self._timer.stop()
            self.hide()
            return
        if badge == self._badge and self.isVisible():
            return
        self._badge = badge
        metrics = QFontMetrics(self._font)
        width = metrics.horizontalAdvance(badge.text) + 2 * visitor.PAD_X + 1
        height = visitor.FONT_PX + 2 * visitor.PAD_Y + 4
        self.setFixedSize(width, height)
        self._follow()
        self.show()
        keep_on_top(self)
        self._timer.start()
        self.update()

    def _follow(self) -> None:
        if self._badge is None:
            return
        pos = QCursor.pos()
        screens = []
        for screen in QGuiApplication.screens():
            geometry = screen.geometry()
            screens.append((geometry.x(), geometry.y(), geometry.width(), geometry.height()))
        x, y = visitor.place((pos.x(), pos.y()), (self.width(), self.height()), screens)
        if (round(x), round(y)) != (self.x(), self.y()):
            self.move(round(x), round(y))

    def paintEvent(self, _event) -> None:
        if self._badge is None:
            return
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        rect = QRectF(0, 0, self.width(), self.height())
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QColor(self._badge.colour))
        painter.drawRoundedRect(rect, visitor.RADIUS, visitor.RADIUS)
        painter.setPen(QColor(visitor.TEXT_COLOUR))
        painter.setFont(self._font)
        painter.drawText(rect, Qt.AlignmentFlag.AlignCenter, self._badge.text)
        painter.end()
