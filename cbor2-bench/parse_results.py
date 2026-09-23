#!/usr/bin/env python3
"""Parse Criterion time point estimates from a run log into Markdown tables."""
import argparse
import re

ID_RE = re.compile(r"^(alloc|std|no_alloc|focused|review|derive)/\S[^\r\n]*")
# The middle value is the slope (or mean) point estimate, not the median.
VALUE = r"[0-9]+(?:\.[0-9]+)?(?:e[+-]?[0-9]+)?\s+(?:ps|ns|µs|us|ms|s)"
TIME_RE = re.compile(rf"time:\s*\[\s*({VALUE})\s+({VALUE})\s+({VALUE})\s*\]")
ANSI_RE = re.compile(r"\x1b\[[0-9;]*m")


def parse_results(lines):
    results = {}
    pending_id = None
    for line in lines:
        line = ANSI_RE.sub("", line).strip()
        match = TIME_RE.search(line)
        if match:
            # Unknown inline IDs must never inherit the previous benchmark ID.
            bench_id = line[:match.start()].strip() or pending_id
            if bench_id and ID_RE.fullmatch(bench_id):
                results[bench_id] = match.group(2)
            pending_id = None
        elif line:
            # Unknown groups and progress/change lines also clear the identity.
            pending_id = line if ID_RE.fullmatch(line) else None
    return results


def fmt(v):
    return v if v else "—"


CRATES = ["cbor2", "ciborium", "serde_cbor", "cbor4ii", "minicbor"]
PAYLOADS = ["int_array", "log_batch", "blob"]


def table(results, prefix, ops, crates=CRATES, op_label="op"):
    header = "| " + op_label + " / payload | " + " | ".join(crates) + " |"
    sep = "|" + "---|" * (len(crates) + 1)
    rows = [header, sep]
    for op in ops:
        for p in PAYLOADS:
            cells = []
            for cr in crates:
                key = f"{prefix}/{op}/{p}/{cr}"
                cells.append(fmt(results.get(key)))
            rows.append(f"| `{op}/{p}` | " + " | ".join(cells) + " |")
    return "\n".join(rows)


def print_results(results):
    print("### alloc (`to_vec` / `from_slice`)\n")
    print(table(results, "alloc", ["encode", "decode"]))
    print("\n### std\n")
    print(table(results, "std", ["encode", "decode"]))
    print("\n### no_alloc — encode (fixed buffer, zero allocation)\n")
    print(table(results, "no_alloc", ["encode"]))
    print("\n### no_alloc — structural scan\n")
    scan = ["cbor2 (validate)", "cbor2 (validate_slice)", "minicbor (skip)"]
    hdr = "| payload | " + " | ".join(scan) + " |"
    print(hdr)
    print("|" + "---|" * (len(scan) + 1))
    for p in PAYLOADS:
        cells = [fmt(results.get(f"no_alloc/scan/{p}/{c}")) for c in scan]
        print(f"| `{p}` | " + " | ".join(cells) + " |")
    print("\n### no_alloc — `cbor2::serialized_size` (no output buffer)\n")
    print("| payload | cbor2::serialized_size |")
    print("|---|---|")
    for p in PAYLOADS:
        print(f"| `{p}` | {fmt(results.get('no_alloc/serialized_size (cbor2)/' + p))} |")

    focused = [
        (key, value) for key, value in sorted(results.items())
        if key.startswith(("focused/", "review/", "derive/"))
    ]
    if focused:
        print("\n### Focused and derive workloads\n\n| operation | estimate |\n|---|---|")
        for key, value in focused:
            print(f"| `{key}` | {value} |")

    print(f"\n_({len(results)} measurements parsed; Criterion time point estimates)_")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", help="Criterion stdout log")
    args = parser.parse_args()
    with open(args.log, encoding="utf-8") as source:
        results = parse_results(source)
    if not results:
        parser.error("no supported Criterion time estimates found")
    print_results(results)


if __name__ == "__main__":
    main()
