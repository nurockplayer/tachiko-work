"""Known ST_Xstring values for the independent XLSX readback oracle."""

import unittest

from rectangle_oracle import decoded_text


class XstringReadbackTests(unittest.TestCase):
    def test_single_pass_utf16_text(self):
        for source, expected in [
            ("_x000D_", "\r"),
            ("_x000d_", "\r"),
            ("_x005F_x0041_", "_x0041_"),
            ("_x0041_", "A"),
            ("_x000G_", "_x000G_"),
            ("_x00", "_x00"),
            ("_xD83D__xDE00_", "😀"),
            ("_x005F_xD800_", "_xD800_"),
        ]:
            with self.subTest(source=source):
                self.assertEqual(decoded_text(source), expected)

    def test_invalid_decoded_text_refuses(self):
        for source in ["_xD800_", "_xDC00_", "_x0001_", "_xFFFE_", "_xFFFF_"]:
            with self.subTest(source=source):
                with self.assertRaises((UnicodeDecodeError, AssertionError)):
                    decoded_text(source)


if __name__ == "__main__":
    unittest.main()
