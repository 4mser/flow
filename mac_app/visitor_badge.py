"""The visitor badge on the Mac: a tag in the driving machine's colour that follows the pointer while
another machine has input here. What it says, its colour and where it sits come from core.visitor.

One small borderless, click-through panel, moved on the display refresh to wherever the pointer is.
Main thread only, like every other overlay here; the app calls `show` and `hide` through
AppHelper.callAfter.
"""

import logging

import AppKit
import Quartz

from core import visitor
from frame_clock import FrameClock

LOGGER = logging.getLogger(__name__)


def _cg_colour(hex_colour, alpha=1.0):
    value = hex_colour.lstrip("#")
    red, green, blue = (int(value[i:i + 2], 16) / 255.0 for i in (0, 2, 4))
    return Quartz.CGColorCreateGenericRGB(red, green, blue, alpha)


class VisitorBadge:
    def __init__(self, logger=None):
        self.logger = logger or LOGGER
        self.badge = None
        self.panel = None
        self.background = None
        self.label = None
        self.clock = None
        self.size = (0.0, 0.0)
        self.font = AppKit.NSFont.boldSystemFontOfSize_(visitor.FONT_PX)

    def show(self, badge):
        """Show `badge` (a core.visitor.Badge) at the pointer, or update the one shown."""
        if badge is None:
            self.hide()
            return
        if badge == self.badge and self.panel is not None:
            return
        self.badge = badge
        try:
            self._build(badge)
            self._follow()
            self.panel.orderFrontRegardless()
            if self.clock is None:
                self.clock = FrameClock(self.panel.contentView(), self._follow, max_rate=60)
            self.clock.start()
        except Exception:
            self.logger.exception("The visitor badge could not be shown")

    def hide(self):
        self.badge = None
        if self.clock is not None:
            self.clock.stop()
        if self.panel is not None:
            self.panel.orderOut_(None)

    def _build(self, badge):
        text_width = AppKit.NSAttributedString.alloc().initWithString_attributes_(
            badge.text, {AppKit.NSFontAttributeName: self.font}
        ).size().width
        width = float(int(text_width + 2 * visitor.PAD_X + 1))
        height = float(visitor.FONT_PX + 2 * visitor.PAD_Y + 4)
        self.size = (width, height)
        frame = AppKit.NSMakeRect(0, 0, width, height)
        if self.panel is None:
            panel = AppKit.NSPanel.alloc().initWithContentRect_styleMask_backing_defer_(
                frame,
                AppKit.NSWindowStyleMaskBorderless | AppKit.NSWindowStyleMaskNonactivatingPanel,
                AppKit.NSBackingStoreBuffered,
                False,
            )
            # Above the menu bar and the crossing effects, as the pointer itself is.
            panel.setLevel_(AppKit.NSPopUpMenuWindowLevel)
            panel.setOpaque_(False)
            panel.setHasShadow_(True)
            panel.setBackgroundColor_(AppKit.NSColor.clearColor())
            panel.setIgnoresMouseEvents_(True)
            panel.setCollectionBehavior_(
                AppKit.NSWindowCollectionBehaviorCanJoinAllSpaces
                | AppKit.NSWindowCollectionBehaviorFullScreenAuxiliary
                | AppKit.NSWindowCollectionBehaviorStationary
                | AppKit.NSWindowCollectionBehaviorIgnoresCycle
            )
            view = AppKit.NSView.alloc().initWithFrame_(frame)
            view.setWantsLayer_(True)
            panel.setContentView_(view)
            self.background = Quartz.CALayer.layer()
            self.background.setCornerRadius_(visitor.RADIUS)
            self.label = Quartz.CATextLayer.layer()
            self.label.setFont_(self.font)
            self.label.setFontSize_(visitor.FONT_PX)
            self.label.setAlignmentMode_(Quartz.kCAAlignmentCenter)
            self.label.setTruncationMode_(Quartz.kCATruncationEnd)
            view.layer().addSublayer_(self.background)
            self.background.addSublayer_(self.label)
            self.panel = panel
        else:
            self.panel.setContentSize_((width, height))
            self.panel.contentView().setFrame_(frame)

        scale = self.panel.backingScaleFactor() or 2.0
        Quartz.CATransaction.begin()
        Quartz.CATransaction.setDisableActions_(True)
        self.background.setFrame_(((0, 0), (width, height)))
        self.background.setBackgroundColor_(_cg_colour(badge.colour))
        self.label.setFrame_(((0, visitor.PAD_Y), (width, height - 2 * visitor.PAD_Y)))
        self.label.setString_(badge.text)
        self.label.setForegroundColor_(_cg_colour(visitor.TEXT_COLOUR))
        self.label.setContentsScale_(scale)
        Quartz.CATransaction.commit()

    def _follow(self):
        if self.panel is None or self.badge is None:
            return
        point = AppKit.NSEvent.mouseLocation()
        screens = []
        for screen in AppKit.NSScreen.screens() or []:
            frame = screen.frame()
            screens.append((frame.origin.x, frame.origin.y, frame.size.width, frame.size.height))
        x, y = visitor.place((point.x, point.y), self.size, screens, y_up=True)
        self.panel.setFrameOrigin_((round(x), round(y)))
