#!/usr/bin/env python3
"""Check pure BIO decoding without importing model libraries or loading weights."""

import ast
from pathlib import Path
import unittest


def load_decoder():
    directory = Path(__file__).resolve().parent
    source = ast.parse((directory / "student_probe_model.py").read_text())
    constants = [node for node in source.body if isinstance(node, ast.Assign)
                 and any(isinstance(target, ast.Name) and target.id in {"CATEGORIES", "BIO_LABELS"}
                         for target in node.targets)]
    trainer = ast.parse((directory / "train-student-probe.py").read_text())
    decoder = next(node for node in trainer.body if isinstance(node, ast.FunctionDef) and node.name == "decode")
    namespace = {}
    exec(compile(ast.Module(body=constants + [decoder], type_ignores=[]), "<pure-decoder>", "exec"), namespace)
    return namespace["decode"]


class DecoderRegression(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.decode = staticmethod(load_decoder())

    def test_ignored_whitespace_preserves_unicode_entity(self):
        prepared = {"labels": [-100, 3, -100, 4, -100],
                    "offsets": [(0, 0), (0, 6), (7, 7), (7, 16), (0, 0)]}
        self.assertEqual(self.decode([0, 3, 0, 4, 0], prepared), {(0, 16, "organization")})

    def test_real_o_token_closes_entity(self):
        prepared = {"labels": [3, 0, 9], "offsets": [(0, 4), (4, 5), (5, 9)]}
        self.assertEqual(self.decode([3, 0, 9], prepared), {(0, 4, "organization"), (5, 9, "event")})

    def test_new_b_does_not_merge_same_category(self):
        prepared = {"labels": [3, -100, 3], "offsets": [(0, 4), (5, 5), (5, 9)]}
        self.assertEqual(self.decode([3, 0, 3], prepared), {(0, 4, "organization"), (5, 9, "organization")})


if __name__ == "__main__":
    unittest.main()
