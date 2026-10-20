"""FLOW's design tokens: Beamer's Vernier system in FLOW's red (4MSER // NÚCLEO).

This file is the one home for every colour, size, face and spring both apps draw with. The Mac app
reads it through the root theme.py; win_app/tokens.py is a byte-identical copy because PyInstaller
only bundles what sits under the app, and win_app's tests fail if the two drift. Edit this one, then
copy it across.

`signal` is the one colour that means something is happening: the link is up, input is on Windows,
pressure is building, a permission is granted. In FLOW it is the brand red, so `fault` moved to
orange and `amber` to yellow to keep a failure from reading as success. Beamer used cockpit cyan;
reverting is `signal`, `signal_dim`, `fault` and `amber` in both palettes.

PALETTE is the dark settings window and PALETTE_LIGHT the light one, the same names in the same
roles, chosen by the `appearance` setting. The crossing effects and PALETTES take the dark `signal`
whatever the window shows: they draw on the desktop, not on the window.
"""

PALETTE = {
    "ground": "#060405",
    "panel": "#0f0b0c",
    "well": "#1b1416",
    "rule": "#2b2124",
    "edge": "#77686c",
    "ink": "#f4f5f7",
    "ink_2": "#bfb6b9",
    "ink_3": "#968b8f",
    "signal": "#ef4444",
    "signal_dim": "#4d1519",
    "amber": "#f2c94c",
    "resistance_flash": "#b77a00",
    "fault": "#ff9a52",
    "off": "#4a4144",
}

# Measured WCAG ratios, for whoever changes a value: ink on panel 17.9, ink_2 on panel 9.9 and on
# well 9.2, ink_3 on panel 6.0 and on well 5.5, edge on panel 3.7 and on well 3.4 (control
# boundaries), signal on panel 5.2, ground on signal 5.4, ink on signal_dim 13.4, amber on panel
# 12.3, fault on panel 9.3.

# Paper rather than the dark palette turned over: modules are lighter than the window, as in the dark
# one, so a module still lifts off the ground and a well still sets a control apart from its module.
# signal keeps its hue but drops to a depth that carries text on paper and a ground-coloured glyph on
# itself. Measured WCAG ratios: ink on panel 17.2, ink_2 on panel 9.8 and on well 8.0, ink_3 on
# panel 6.2 and on well 5.1, edge on panel 4.3 and on well 3.5, signal on panel 5.6 and on ground 5.1,
# ground on signal 5.1, ink on signal_dim 12.0, amber on panel 5.9, fault on panel 5.7.
PALETTE_LIGHT = {
    "ground": "#f3f1f0",
    "panel": "#fcfbfa",
    "well": "#e8e4e3",
    "rule": "#ddd8d7",
    "edge": "#7f7578",
    "ink": "#1b1718",
    "ink_2": "#47403f",
    "ink_3": "#665d5f",
    "signal": "#c81e1e",
    "signal_dim": "#f6caca",
    "amber": "#7d5d00",
    "resistance_flash": "#b55f00",
    "fault": "#a84600",
    "off": "#b0a9aa",
}

# The `appearance` setting on both apps: follow the system, or keep one palette.
APPEARANCES = ("system", "light", "dark")

UI_FAMILY = "Space Grotesk"
MONO_FAMILY = "IBM Plex Mono"
FONT_FILES = (
    "SpaceGrotesk-Variable.ttf",
    "IBMPlexMono-Regular.ttf",
    "IBMPlexMono-Bold.ttf",
)

# Points on the Mac, logical pixels on Windows. The `_narrow` sizes apply at the 640 minimum width.
TYPE = {
    "eyebrow": 10.5,
    "readout_key": 9.5,
    "small": 11.5,
    "note": 12.5,
    "body": 13.0,
    "button": 13.0,
    "status_word": 38.0,
    "status_word_narrow": 30.0,
    "readout": 34.0,
    "readout_narrow": 26.0,
    "numeral": 52.0,
    "numeral_narrow": 42.0,
    "keycap": 20.0,
    "field_mono": 13.0,
    "code": 92.0,
}

# Colour choices for the crossing feedback on both machines: `crossing.glow_colour` on the Mac and
# `glow_colour` on Windows. `signal` is the Vernier colour; the rest are Border Beam's palettes
# (libraries.dev/beam), each in the order its colours sit round a border.
PALETTES = {
    "signal": (PALETTE["signal"],),
    "colourful": ("#ff3264", "#ff7828", "#f032b4", "#b428f0", "#6446ff", "#288cff", "#1eb9aa", "#32c850"),
    "ocean": ("#288cff", "#6446ff", "#1eb9aa", "#3c6eff"),
    "sunset": ("#ff7828", "#ff3264", "#ffb43c", "#f03278"),
    "mono": ("#ebebeb", "#969696", "#ebebeb"),
    "aurora": ("#53dc91", "#9572eb", "#43c5b4"),
}

# Letter spacing as a fraction of the font size, for the uppercase tracked labels.
TRACKING = {
    "eyebrow": 0.14,
    "readout_key": 0.14,
    "selector": 0.12,
}

RADIUS = {
    "field": 4.0,
    "button": 5.0,
    "segment": 5.0,
    "keycap": 6.0,
}

MODULE_PADDING = (14.0, 16.0)
RACK_GAP = 1.0
MIN_WINDOW = {"mac": (640, 540), "windows": (640, 600)}
READING_WIDTH = 560.0
PAGE_CONTENT_WIDTH = 960.0
# The page column is this share of the free width (window minus sidebar), clamped between the
# minimum and the maximum, centred, and never wider than the free width itself.
PAGE_CONTENT_MIN = 960.0
PAGE_CONTENT_MAX = 1600.0
PAGE_CONTENT_SHARE = 0.75
SIDE_BY_SIDE_GAP = 24.0
CROSSING_ARRANGEMENT_MIN_WIDTH = 254.0
CROSSING_WAYS_MIN_WIDTH = 354.0
DESIGN_TILE_SETS_MIN_WIDTH = 258.0
DESIGN_STYLE_CONTROLS_MIN_WIDTH = 240.0

# Mass-spring parameters (mass 1) for everything that moves. The notch island integrates two of
# these, width and depth; a trackpad tick adds `tick_velocity` points per second to the width.
SPRING = {
    "island": {"stiffness": 300.0, "damping": 20.0},
    "breakthrough": {"stiffness": 600.0, "damping": 28.0, "seconds": 0.32},
    "tick_velocity": 260.0,
    "flash_seconds": 0.14,
}

ISLAND = {
    "grow_width": 40.0,
    "grow_depth": 26.0,
    "meter_segments": 4,
    "meter_after_depth": 14.0,
}


def tokens() -> dict:
    return dict(PALETTE)
