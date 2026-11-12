"""Screenshot public empty-return schedules (no sign-in needed).

    python screenshot.py                 # every terminal with a public schedule
    python screenshot.py --terminal its  # one terminal
    python screenshot.py --terminal its --json   # used by the dashboard

Images are saved to screenshots/<terminal>/schedule_YYYYMMDD_HHMMSS.png.
With --json the last line printed is {"terminal": ..., "screenshot": ...}
or {"terminal": ..., "error": ...} for each terminal.
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime

from dotenv import load_dotenv

from desk_browser import PUBLIC_PAGES, ROOT, SHOTS, env_url


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
                    out = {"terminal": terminal_id, "screenshot": path}
                    text = f"{terminal_id}: saved {path}"
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
