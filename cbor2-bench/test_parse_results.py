import contextlib
import io
import unittest

from parse_results import parse_results, print_results


class ParseResultsTests(unittest.TestCase):
    def parse(self, text):
        return parse_results(text.splitlines())

    def test_derive_does_not_replace_previous_target(self):
        results = self.parse("""alloc/decode/blob/minicbor
                        time: [90 ns 100 ns 110 ns]
derive/direct16         time: [1 µs 2 µs 3 µs]
""")
        self.assertEqual(results, {"alloc/decode/blob/minicbor": "100 ns",
                                   "derive/direct16": "2 µs"})
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            print_results(results)
        self.assertIn("| `decode/blob` | — | — | — | — | 100 ns |", output.getvalue())
        self.assertIn("| `derive/direct16` | 2 µs |", output.getvalue())

    def test_unknown_inline_and_wrapped_ids_do_not_reuse_pending_id(self):
        self.assertEqual(self.parse("""alloc/encode/blob/cbor2
future/inline time: [1 ns 2 ns 3 ns]
alloc/decode/blob/cbor2
future/wrapped
time: [4 ns 5 ns 6 ns]
"""), {})

    def test_time_line_consumes_pending_id(self):
        self.assertEqual(self.parse("""std/encode/blob/cbor2
time: [1 ns 2 ns 3 ns]
time: [4 ns 5 ns 6 ns]
"""), {"std/encode/blob/cbor2": "2 ns"})

    def test_spaces_colors_and_other_criterion_output(self):
        results = self.parse("""Benchmarking no_alloc/scan/blob/cbor2 (validate_slice): Analyzing
\x1b[32mno_alloc/scan/blob/cbor2 (validate_slice)\x1b[0m
time: [1 ns 2 ns 3 ns]
thrpt: [1 GiB/s 2 GiB/s 3 GiB/s]
change:
time: [-10% -5% +1%] (p = 0.01 < 0.05)
Found 1 outliers among 10 measurements
no_alloc/serialized_size (cbor2)/blob

time: [1.0 ns 1.5 ns 2.0 ns]
review/value_array time: [1 µs 2 µs 3 µs]
focused/raw/decode_1MiB
time: [1 ms 2 ms 3 ms]
""")
        self.assertEqual(len(results), 4)
        self.assertEqual(results["no_alloc/scan/blob/cbor2 (validate_slice)"], "2 ns")
        self.assertEqual(results["no_alloc/serialized_size (cbor2)/blob"], "1.5 ns")


if __name__ == "__main__":
    unittest.main()
