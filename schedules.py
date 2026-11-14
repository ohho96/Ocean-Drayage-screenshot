"""Read public empty-receiving schedules into receiving-board rows.

Each row is {"date": "YYYY-MM-DD", "shift": "1st"|"2nd", "line": <name>,
"size": "twenty"|"forty"|"forty_hc"|"forty_five",
"status": "receiving"|"dual_only"|"not_receiving"}, the same shape the
dashboard stores. ITS, TraPac and APM (its daily PDF) are read; other
terminals return [].
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


TRAPAC_SIZES = {"20SD86": "twenty", "40SD86": "forty", "40SD96": "forty_hc", "45SD96": "forty_five"}
TRAPAC_HEADING = re.compile(r"FOR\s+\w+,\s+([A-Z]+)\s+(\d{1,2})\w*\s*-\s*SHIFT\s*(\d)", re.I)
MONTHS = {m: i for i, m in enumerate(
    ["JANUARY", "FEBRUARY", "MARCH", "APRIL", "MAY", "JUNE", "JULY", "AUGUST",
     "SEPTEMBER", "OCTOBER", "NOVEMBER", "DECEMBER"], start=1)}


def _year_for(month: int, today) -> int:
    # Headings carry no year; pick the one closest to today (handles Dec -> Jan).
    if month - today.month > 6:
        return today.year - 1
    if today.month - month > 6:
        return today.year + 1
    return today.year


def parse_trapac_blocks(blocks: list[dict], today=None) -> list[dict]:
    """Parse TraPac tables given as [{"heading": str, "rows": [[cell, ...], ...]}]."""
    from datetime import date as _date

    today = today or _date.today()
    rows: list[dict] = []
    for block in blocks:
        found = TRAPAC_HEADING.search(" ".join(block.get("heading", "").split()))
        table = block.get("rows") or []
        if not found or len(table) < 2:
            continue
        month = MONTHS.get(found.group(1).upper())
        if not month:
            continue
        day = int(found.group(2))
        date = f"{_year_for(month, today)}-{month:02d}-{day:02d}"
        shift = {"1": "1st", "2": "2nd"}.get(found.group(3))
        if not shift:
            continue
        sizes = [TRAPAC_SIZES.get("".join(cell.split()).upper()) for cell in table[0][1:]]
        for cells in table[1:]:
            code = re.search(r"\(([A-Z]{2,4})\)", cells[0].upper()) if cells else None
            line = LINE_NAMES.get(code.group(1)) if code else None
            if not line:
                continue
            for size, cell in zip(sizes, cells[1:]):
                value = " ".join(cell.upper().split())
                if value.startswith("DUAL"):
                    status = "dual_only"
                elif value.startswith("YES"):
                    status = "receiving"
                elif value.startswith("NO"):
                    status = "not_receiving"
                else:
                    status = None
                if size and status:
                    rows.append({"date": date, "shift": shift, "line": line, "size": size, "status": status})
    return rows


def read_trapac(page) -> list[dict]:
    blocks = page.evaluate(
        """() => [...document.querySelectorAll('table')].map(t => {
            let heading = '', el = t;
            for (let i = 0; i < 6 && !heading; i++) {
              el = el.previousElementSibling || el.parentElement;
              if (!el) break;
              const tx = (el.innerText || '').trim();
              if (/RECEIVING/i.test(tx) && tx.length < 200) heading = tx;
            }
            return { heading, rows: [...t.rows].map(r => [...r.cells].map(c => c.innerText.trim())) };
        })"""
    )
    return parse_trapac_blocks(blocks)


# ---- APM: weekly grid PDF linked from the Empty Receivables page ----------

APM_LINES = (
    ("MAERSK", "Maersk"), ("HAPAG", "Hapag-Lloyd"), ("EVERGREEN", "Evergreen"),
    ("COSCO", "COSCO"), ("CMA", "CMA CGM"), ("WAN HAI", "Wan Hai"), ("HYUNDAI", "HMM"),
    ("HMM", "HMM"), ("YANG MING", "Yang Ming"), ("SM LINE", "SM Line"), ("ZIM", "ZIM"),
    ("MSC", "MSC"), ("MATSON", "Matson"), ("ONE", "ONE"),
)
APM_SIZES = {"20'": "twenty", "40' STD": "forty", "40' HC": "forty_hc", "45'": "forty_five"}
APM_STATUS = {"YES": "receiving", "YES-S": "receiving", "YES-D": "dual_only", "NO": "not_receiving"}


def _apm_line(label: str) -> str | None:
    text = " ".join(label.upper().split())
    for key, name in APM_LINES:
        if key == "ONE":
            if re.search(r"\bONE\b", text):
                return name
        elif key in text:
            return name
    return None


def _sideways_labels(chars: list[dict]) -> list[tuple[float, float, str]]:
    """Line names written sideways down the left: (top, bottom, text) per block."""
    side = sorted((c for c in chars if not c.get("upright", True)), key=lambda c: c["top"])
    groups: list[list[dict]] = []
    for c in side:
        if groups and c["top"] - max(x["bottom"] for x in groups[-1]) < 12:
            groups[-1].append(c)
        else:
            groups.append([c])
    labels = []
    for group in groups:
        # Several sideways columns can share a block ("CMA-CGM / + USL + ANL").
        columns: dict[int, list[dict]] = {}
        for c in group:
            columns.setdefault(round(c["x0"] / 3), []).append(c)
        parts = []
        for _x, cs in sorted(columns.items()):
            parts.append("".join(c["text"] for c in sorted(cs, key=lambda c: -c["top"])).strip())
        top = min(c["top"] for c in group)
        bottom = max(c["bottom"] for c in group)
        labels.append((top, bottom, " ".join(p for p in parts if p)))
    return labels


def parse_apm_pdf(source) -> list[dict]:
    """Parse APM's empty receivables PDF (path or bytes)."""
    import io

    import pdfplumber

    stream = io.BytesIO(source) if isinstance(source, (bytes, bytearray)) else source
    rows: list[dict] = []
    with pdfplumber.open(stream) as pdf:
        for page in pdf.pages:
            # Upright text only; the sideways line names are read separately.
            upright = page.filter(lambda obj: obj.get("object_type") != "char" or obj.get("upright", True))
            words = upright.extract_words(keep_blank_chars=False, use_text_flow=False)
            dates = sorted(
                (w for w in words if re.fullmatch(r"\d{1,2}/\d{1,2}/\d{2,4}", w["text"])),
                key=lambda w: w["x0"],
            )
            shift_words = sorted((w for w in words if w["text"] in ("1st", "2nd", "3rd")), key=lambda w: w["x0"])
            if not dates or len(shift_words) < 3:
                continue
            header_y = max(w["bottom"] for w in shift_words)
            centers = [(w["x0"] + w["x1"]) / 2 for w in shift_words]
            date_x = [((d["x0"] + d["x1"]) / 2, d["text"]) for d in dates]

            def column_for(x: float) -> int:
                return min(range(len(centers)), key=lambda i: abs(centers[i] - x))

            def date_for(i: int) -> str:
                # The date heading nearest above this shift column.
                _x, text = min(date_x, key=lambda d: abs(d[0] - centers[i]))
                month, day, year = (int(v) for v in text.split("/"))
                year += 2000 if year < 100 else 0
                return f"{year}-{month:02d}-{day:02d}"

            labels = _sideways_labels(page.chars)
            first_column = min(w["x0"] for w in shift_words) - 6

            # Group words into rows; a label and its values can differ by a fraction of a point.
            body = sorted((w for w in words if w["top"] > header_y), key=lambda w: w["top"])
            grouped: list[list[dict]] = []
            for w in body:
                if grouped and abs(w["top"] - grouped[-1][0]["top"]) <= 3:
                    grouped[-1].append(w)
                else:
                    grouped.append([w])
            for items in grouped:
                y = sum(w["top"] for w in items) / len(items)
                label = " ".join(w["text"] for w in sorted(items, key=lambda w: w["x0"]) if w["x1"] < first_column)
                size = APM_SIZES.get(" ".join(label.upper().split()))
                if not size:
                    continue
                # Each line name is centred on its block of rows: take the nearest one.
                near = min(labels, key=lambda lab: abs((lab[0] + lab[1]) / 2 - y), default=None)
                line = _apm_line(near[2]) if near and abs((near[0] + near[1]) / 2 - y) < 60 else None
                if not line:
                    continue
                for w in items:
                    if w["x0"] < first_column:
                        continue
                    status = APM_STATUS.get(w["text"].upper())
                    i = column_for((w["x0"] + w["x1"]) / 2)
                    shift = shift_words[i]["text"]
                    if status and shift in ("1st", "2nd"):
                        rows.append({"date": date_for(i), "shift": shift, "line": line, "size": size, "status": status})
    return rows


def read_apm(page) -> list[dict]:
    """Find today's Empty Receivables PDF on the page, keep a copy, and parse it."""
    from datetime import datetime

    find_links = """() => [...document.querySelectorAll('a[href]')]
        .filter(a => /\\.pdf/i.test(a.href) && /empty/i.test(a.textContent || ''))
        .map(a => a.href)"""
    links: list[str] = []
    for _ in range(30):  # the link can appear a few seconds after load
        links = page.evaluate(find_links)
        if links:
            break
        page.wait_for_timeout(500)
    if not links:
        return []
    response = None
    for _ in range(2):
        try:
            response = page.context.request.get(links[0], timeout=60000)
            if response.ok:
                break
        except Exception:
            response = None
    if response is None or not response.ok:
        return []
    body = response.body()
    try:
        from desk_browser import SHOTS

        folder = SHOTS / "apm"
        folder.mkdir(parents=True, exist_ok=True)
        (folder / f"empty_receivables_{datetime.now().strftime('%Y%m%d_%H%M%S')}.pdf").write_bytes(body)
    except OSError:
        pass
    return parse_apm_pdf(body)


def read_its(page) -> list[dict]:
    grid = page.evaluate(
        """() => [...document.querySelectorAll('table tr')].map(r =>
            [...r.children].map(c => [c.innerText, c.colSpan || 1]))"""
    )
    return parse_its_grid([[(text, int(span)) for text, span in row] for row in grid])


READERS = {"its": read_its, "trapac": read_trapac, "apm": read_apm}


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

    from datetime import date as _date

    blocks = [
        {
            "heading": "LAX EMPTY RECEIVING FOR THURSDAY, NOVEMBER 12TH - SHIFT 2",
            "rows": [
                ["SS LINE", "20SD86", "40SD86", "40SD96", "45SD96", "20RF & 40RF"],
                ["OCEAN NETWORK EXPRESS (ONE)", "DUAL TRANSACTIONS ONLY", "YES", "NO", "YES", "YES"],
                ["SOMEONE ELSE (XXX)", "YES", "YES", "YES", "YES", "YES"],
            ],
        },
        {"heading": "Cookies", "rows": [["Google", "x"]]},
    ]
    rows = parse_trapac_blocks(blocks, today=_date(2026, 11, 12))
    assert len(rows) == 4, rows
    assert rows[0] == {"date": "2026-11-12", "shift": "2nd", "line": "ONE", "size": "twenty", "status": "dual_only"}
    assert rows[2]["size"] == "forty_hc" and rows[2]["status"] == "not_receiving"
    # A January heading seen in December belongs to next year.
    jan = [{"heading": "EMPTY RECEIVING FOR FRIDAY, JANUARY 2ND - SHIFT 1", "rows": blocks[0]["rows"][:2]}]
    assert parse_trapac_blocks(jan, today=_date(2026, 12, 30))[0]["date"] == "2027-01-02"


def _self_test_apm() -> None:
    assert _apm_line("CMA-CGM + USL+ ANL") == "CMA CGM"
    assert _apm_line("MAERSK LINE") == "Maersk"
    assert _apm_line("HYUNDAI") == "HMM"
    assert _apm_line("ONE") == "ONE"
    assert _apm_line("SM LINE") == "SM Line"
    assert _apm_line("PHONE") is None
    assert APM_STATUS["YES-D"] == "dual_only" and APM_STATUS["YES-S"] == "receiving"


if __name__ == "__main__":
    _self_test_apm()
    _self_test()
    print("ok")
    sys.exit(0)
