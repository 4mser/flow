import unittest

from core import visitor


class ColourTests(unittest.TestCase):
    def test_same_id_same_colour(self):
        self.assertEqual(visitor.colour_for("AAAAAAAAAAAAAAAAAAAAAA"), visitor.colour_for("AAAAAAAAAAAAAAAAAAAAAA"))

    def test_colour_is_from_palette(self):
        for ident in ("a", "b", "c", "", "xyz-123"):
            self.assertIn(visitor.colour_for(ident), visitor.PALETTE)

    def test_text_contrast_on_every_colour(self):
        def luminance(colour):
            channels = [int(colour.lstrip("#")[i:i + 2], 16) / 255 for i in (0, 2, 4)]
            channels = [c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4 for c in channels]
            return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]
        text = luminance(visitor.TEXT_COLOUR)
        for colour in visitor.PALETTE:
            fill = luminance(colour)
            ratio = (max(text, fill) + 0.05) / (min(text, fill) + 0.05)
            self.assertGreaterEqual(ratio, 4.5, colour)

    def test_ids_spread_over_palette(self):
        colours = {visitor.colour_for(f"peer-{n}") for n in range(200)}
        self.assertEqual(colours, set(visitor.PALETTE))


class NameTests(unittest.TestCase):
    def test_trims_and_collapses_space(self):
        self.assertEqual(visitor.shown_name("  Nico's   MacBook \n"), "Nico's MacBook")

    def test_cuts_long_name_with_ellipsis(self):
        text = visitor.shown_name("x" * 40)
        self.assertEqual(len(text), visitor.MAX_NAME)
        self.assertTrue(text.endswith(visitor.ELLIPSIS))

    def test_none_is_empty(self):
        self.assertEqual(visitor.shown_name(None), "")


class BadgeTests(unittest.TestCase):
    def test_no_owner_no_badge(self):
        self.assertIsNone(visitor.badge_for(None, "Mac", "another machine"))
        self.assertIsNone(visitor.badge_for("", "Mac", "another machine"))

    def test_badge_carries_name_and_colour(self):
        badge = visitor.badge_for("peer", "MacBook", "another machine")
        self.assertEqual(badge.text, "MacBook")
        self.assertEqual(badge.colour, visitor.colour_for("peer"))

    def test_missing_name_uses_fallback(self):
        self.assertEqual(visitor.badge_for("peer", "  ", "another machine").text, "another machine")


class PlaceTests(unittest.TestCase):
    SCREEN = (0, 0, 1920, 1080)

    def test_below_right_of_pointer(self):
        x, y = visitor.place((100, 100), (80, 20), [self.SCREEN])
        self.assertEqual((x, y), (100 + visitor.OFFSET_X, 100 + visitor.OFFSET_Y))

    def test_flips_left_at_right_edge(self):
        x, _ = visitor.place((1900, 100), (80, 20), [self.SCREEN])
        self.assertEqual(x, 1900 - visitor.OFFSET_X - 80)

    def test_flips_up_at_bottom_edge(self):
        _, y = visitor.place((100, 1075), (80, 20), [self.SCREEN])
        self.assertEqual(y, 1075 - visitor.OFFSET_Y - 20)

    def test_stays_on_pointer_screen(self):
        left, right = (0, 0, 1000, 800), (1000, 0, 1000, 800)
        x, _ = visitor.place((990, 100), (80, 20), [left, right])
        self.assertLessEqual(x + 80, 1000)

    def test_negative_screen_origin(self):
        screen = (-1280, 0, 1280, 1024)
        x, y = visitor.place((-10, 500), (80, 20), [screen])
        self.assertGreaterEqual(x, -1280)
        self.assertLessEqual(x + 80, 0)

    def test_y_up_places_below_pointer(self):
        # AppKit: y grows upwards, so "below" is a smaller y and the origin is the bottom edge.
        x, y = visitor.place((100, 500), (80, 20), [self.SCREEN], y_up=True)
        self.assertEqual(y, 500 - visitor.OFFSET_Y - 20)

    def test_y_up_flips_above_at_bottom(self):
        _, y = visitor.place((100, 10), (80, 20), [self.SCREEN], y_up=True)
        self.assertEqual(y, 10 + visitor.OFFSET_Y)

    def test_no_screens_still_places(self):
        self.assertEqual(visitor.place((5, 5), (80, 20), []), (5 + visitor.OFFSET_X, 5 + visitor.OFFSET_Y))


if __name__ == "__main__":
    unittest.main()
