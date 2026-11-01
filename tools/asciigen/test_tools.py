import subprocess
import sys
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
from unittest.mock import patch

import render_contact


HERE = Path(__file__).resolve().parent


class ToolArgumentTests(unittest.TestCase):
    def run_tool(self, name, *args):
        return subprocess.run(
            [sys.executable, str(HERE / name), *args],
            cwd=HERE,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_generator_rejects_non_positive_sizes(self):
        for option in (("--width", "0"), ("--max-frames", "0")):
            with self.subTest(option=option[0]):
                result = self.run_tool("gen.py", *option)
                self.assertEqual(result.returncode, 2)

    def test_contact_sheet_rejects_non_positive_layout(self):
        result = self.run_tool("render_contact.py", "missing", "out", "--cols", "0")
        self.assertEqual(result.returncode, 2)

    def test_contact_sheet_handles_empty_candidates(self):
        output = StringIO()
        with patch.object(render_contact.Path, "iterdir", return_value=[]), patch.object(
            sys, "argv", ["render_contact.py", "unused", "unused"]
        ), redirect_stdout(output):
            result = render_contact.main()
        self.assertEqual(result, 1)
        self.assertIn("No candidate frames found", output.getvalue())


if __name__ == "__main__":
    unittest.main()
