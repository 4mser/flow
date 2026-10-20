"""The visitor badge: while another machine drives this one, a small tag in that machine's colour
follows the pointer and says whose hand it is.

FLOW's first multiplayer feature. Beamer already knows, at every moment, which peer owns this
machine's input (receiver's owner callback); this turns that into something a person sitting at the
driven machine can see, without a word on the wire: the name comes from the peer entry and the
colour from the peer's id, so both ends agree on it without telling each other.

Pure: no Qt, no AppKit, no clock. Each app draws the badge itself and asks this module what to
write, in which colour, and where.
"""

import hashlib
from dataclasses import dataclass
from typing import Optional, Sequence, Tuple

# Red, blue, green, purple, orange and pink: the six cursor colours of the first FLOW, one step
# deeper so the badge's white 12 px text keeps at least 4.5:1 on each (4.6 to 5.4, measured).
PALETTE = ("#dc2626", "#2563eb", "#15803d", "#9333ea", "#c2410c", "#db2777")
TEXT_COLOUR = "#ffffff"
MAX_NAME = 24
ELLIPSIS = "…"
# Where the badge sits from the pointer's hot spot: right of it and below the arrow, so it never
# covers what the pointer points at.
OFFSET_X = 14
OFFSET_Y = 20
FONT_PX = 12
PAD_X = 8
PAD_Y = 3
RADIUS = 6

Rect = Tuple[float, float, float, float]


@dataclass(frozen=True)
class Badge:
    peer: str
    text: str
    colour: str


def colour_for(peer_id: str) -> str:
    """The peer's colour, the same on every machine: a hash of its id picks one of PALETTE."""
    digest = hashlib.sha256((peer_id or "").encode("utf-8")).digest()
    return PALETTE[digest[0] % len(PALETTE)]


def shown_name(name: Optional[str]) -> str:
    """The name as the badge writes it: trimmed, and cut on a code point with an ellipsis."""
    text = " ".join((name or "").split())
    if len(text) > MAX_NAME:
        text = text[:MAX_NAME - 1].rstrip() + ELLIPSIS
    return text


def badge_for(peer_id: Optional[str], name: Optional[str], fallback: str) -> Optional[Badge]:
    """What to show while `peer_id` owns this machine; None when no one does."""
    if not peer_id:
        return None
    return Badge(peer=peer_id, text=shown_name(name) or fallback, colour=colour_for(peer_id))


def place(pointer: Tuple[float, float], size: Tuple[float, float], screens: Sequence[Rect],
          y_up: bool = False) -> Tuple[float, float]:
    """The badge's origin for a pointer at `pointer`: below and to the right of it, flipped to the
    other side where that would leave the screen the pointer is on, then held inside that screen.

    Screens are (x, y, width, height). With `y_up` false (Windows, Qt, X11) y grows downwards and
    the origin returned is the badge's top-left corner; with `y_up` true (AppKit's global
    coordinates) y grows upwards and the origin is the bottom-left corner, as a window's frame
    wants it.
    """
    px, py = pointer
    width, height = size
    screen = _screen_of(pointer, screens)

    x = px + OFFSET_X
    if y_up:
        y = py - OFFSET_Y - height
    else:
        y = py + OFFSET_Y
    if screen is None:
        return x, y

    sx, sy, sw, sh = screen
    if x + width > sx + sw:
        x = px - OFFSET_X - width
    if y_up:
        if y < sy:
            y = py + OFFSET_Y
    elif y + height > sy + sh:
        y = py - OFFSET_Y - height
    x = min(max(x, sx), sx + sw - width)
    y = min(max(y, sy), sy + sh - height)
    return x, y


def _screen_of(pointer: Tuple[float, float], screens: Sequence[Rect]) -> Optional[Rect]:
    px, py = pointer
    for screen in screens:
        sx, sy, sw, sh = screen
        if sx <= px < sx + sw and sy <= py < sy + sh:
            return screen
    # A pointer exactly on a screen's far edge, or between screens for a moment: the nearest one.
    if not screens:
        return None
    return min(screens, key=lambda s: _distance(pointer, s))


def _distance(pointer: Tuple[float, float], screen: Rect) -> float:
    px, py = pointer
    sx, sy, sw, sh = screen
    dx = max(sx - px, 0.0, px - (sx + sw))
    dy = max(sy - py, 0.0, py - (sy + sh))
    return dx * dx + dy * dy
