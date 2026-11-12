"""Screenshot public empty-return schedules (no sign-in needed).

    python screenshot.py                 # every terminal with a public schedule
    python screenshot.py --terminal its  # one terminal
    python screenshot.py --terminal its --json   # used by the dashboard

Images are saved to screenshots/<terminal>/schedule_YYYYMMDD_HHMMSS.png.
With --json one line is printed per terminal:
{"terminal": ..., "screenshot": ..., "receiving": [...]} or
{"terminal": ..., "error": ...}. "receiving" holds the schedule rows read
from the page (ITS today; empty for terminals without a reader yet).
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime

from dotenv import load_dotenv

from desk_browser import PUBLIC_PAGES, ROOT, SHOTS, env_url
from schedules import read_schedule


def snap_public(page, terminal_id: str, output_dir=SHOTS, full_page: bool = True) -> str:
    url = env_url(terminal_id, PUBLIC_PAGES)
    page.goto(url, wait_until="domcontentloaded", timeout=60000)
    try:
        page.wait_for_load_state("networkidle", timeout=15000)
    except Exception:
        page.wait_for_timeout(1500)
    folder = output_dir / terminal_id
    folder.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    path = folder / f"schedule_{stamp}.png"
    page.screenshot(path=str(path), full_page=full_page)
    return str(path)


def main() -> int:
    load_dotenv(ROOT / ".env")
    parser = argparse.ArgumentParser(description="Screenshot public empty-return schedules")
    parser.add_argument("--terminal", choices=sorted(PUBLIC_PAGES), help="Only this terminal")
    parser.add_argument("--output-dir", default=str(SHOTS), help="Folder for screenshots")
    parser.add_argument("--no-full-page", action="store_true", help="Viewport only")
    parser.add_argument("--json", action="store_true", help="Print one JSON line per terminal")
    args = parser.parse_args()

    from pathlib import Path

    from playwright.sync_api import sync_playwright

    targets = [args.terminal] if args.terminal else sorted(PUBLIC_PAGES)
    failed = 0
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch()
        page = browser.new_page(viewport={"width": 1400, "height": 900})
        try:
            for terminal_id in targets:
                try:
                    path = snap_public(page, terminal_id, Path(args.output_dir), not args.no_full_page)
                    try:
                        receiving = read_schedule(page, terminal_id)
                    except Exception:
                        receiving = []
                    out = {"terminal": terminal_id, "screenshot": path, "receiving": receiving}
                    text = f"{terminal_id}: saved {path}"
                    if receiving:
                        text += f" ({len(receiving)} schedule rows read)"
                except Exception as exc:
                    failed += 1
                    out = {"terminal": terminal_id, "error": str(exc).splitlines()[0]}
                    text = f"{terminal_id}: failed — {out['error']}"
                print(json.dumps(out) if args.json else text, flush=True)
        finally:
            browser.close()
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
