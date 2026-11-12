"""Read public empty-receiving schedules into receiving-board rows.

Each row is {"date": "YYYY-MM-DD", "shift": "1st"|"2nd", "line": <name>,
"size": "twenty"|"forty"|"forty_hc"|"forty_five",
"status": "receiving"|"dual_only"|"not_receiving"}, the same shape the
dashboard stores. Only ITS is read so far; other terminals return [].
"""

from __future__ import annotations

import re
import sys

# Line codes used on terminal schedules -> names used on the dashboard.
LINE_NAMES = {
    "ONE": "ONE",
    "YML": "Yang Ming",
    "HMM": "HMM",
    "ZIM": "ZIM",
    "WHL": "Wan Hai",
    "HAP": "Hapag-Lloyd",
    "HLC": "Hapag-Lloyd",
    "MSC": "MSC",
    "MAE": "Maersk",
    "MSK": "Maersk",
    "CMA": "CMA CGM",
    "COS": "COSCO",
    "EMC": "Evergreen",
    "EGL": "Evergreen",
    "PIL": "PIL",
    "SML": "SM Line",
    "MAT": "Matson",
}

SIZES = {
    "20'": "twenty",
    "40' st": "forty",
    "40'": "forty",
    "40' hc": "forty_hc",
    "45'": "forty_five",
}

STATUSES = {"yes": "receiving", "dual": "dual_only", "no": "not_receiving"}

DATE_RE = re.compile(r"(\d{1,2})/(\d{1,2})/(\d{4})")


def parse_its_grid(grid: list[list[tuple[str, int]]]) -> list[dict]:
    """Parse ITS table rows given as [(cell text, colspan), ...]."""
    rows: list[dict] = []
    date = ""
    sizes: list[str | None] = []
    shifts: list[str] = []
    for cells in grid:
        if not cells:
            continue
        first = " ".join(cells[0][0].split())
        found = DATE_RE.search(first)
        if "receiving" in first.lower() and found:
            month, day, year = found.groups()
            date = f"{year}-{int(month):02d}-{int(day):02d}"
            sizes, shifts = [], []
            continue
        if any(SIZES.get(" ".join(text.lower().split())) for text, _span in cells[1:]):
            sizes = []
            for text, span in cells[1:]:
                sizes.extend([SIZES.get(" ".join(text.lower().split()))] * max(span, 1))
            continue
        if first.lower() == "shift":
            shifts = [text.strip().lower() for text, _span in cells[1:]]
            continue
        line = LINE_NAMES.get(first.upper())
        if not (date and sizes and shifts and line):
            continue
        for index, (text, _span) in enumerate(cells[1:]):
            if index >= len(sizes) or index >= len(shifts):
                break
            size = sizes[index]
            shift = shifts[index]
            status = STATUSES.get(text.strip().lower())
            if size and status and shift in {"1st", "2nd"}:
                rows.append(
                    {"date": date, "shift": shift, "line": line, "size": size, "status": status}
                )
    return rows


def read_its(page) -> list[dict]:
    grid = page.evaluate(
        """() => [...document.querySelectorAll('table tr')].map(r =>
            [...r.children].map(c => [c.innerText, c.colSpan || 1]))"""
    )
    return parse_its_grid([[(text, int(span)) for text, span in row] for row in grid])


READERS = {"its": read_its}


def read_schedule(page, terminal_id: str) -> list[dict]:
    reader = READERS.get(terminal_id)
    return reader(page) if reader else []


def _self_test() -> None:
    grid = [
        [("Empty Receiving 11/12/2026 (Subject to Change)", 11)],
        [("Size", 1), ("20'", 2), ("40' ST", 2), ("40' HC", 2), ("45'", 2), ("Special Eqp'", 2)],
        [("Shift", 1)] + [(s, 1) for s in ["1st", "2nd"] * 5],
        [("YML", 1)] + [(v, 1) for v in ["Yes", "Dual", "No", "Dual", "Yes", "Dual", "Yes", "Dual", "No", "No"]],
        [("XYZ", 1)] + [("Yes", 1)] * 10,
    ]
    rows = parse_its_grid(grid)
    assert len(rows) == 8, rows  # 4 sizes x 2 shifts; special eqp and unknown line skipped
    assert rows[0] == {
        "date": "2026-11-12", "shift": "1st", "line": "Yang Ming", "size": "twenty", "status": "receiving",
    }, rows[0]
    assert rows[1]["status"] == "dual_only" and rows[1]["shift"] == "2nd"
    assert rows[2] == {
        "date": "2026-11-12", "shift": "1st", "line": "Yang Ming", "size": "forty", "status": "not_receiving",
    }, rows[2]
    assert rows[7]["size"] == "forty_five"


if __name__ == "__main__":
    _self_test()
    print("ok")
    sys.exit(0)
