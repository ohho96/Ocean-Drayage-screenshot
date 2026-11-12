"""Terminal browser engine shared by the Empty Return Desk and the CLI scripts.

Two ways in:

* Dashboard mode (``--terminal ID --session ID``): the Rust dashboard starts
  this process. It opens the terminal's sign-in page and waits. The user signs
  in by hand, including any captcha. This program never reads or types a
  password. The dashboard then writes data/browser_command.json with action
  "check". This process screenshots a container only when the page shows no
  return appointment, and writes the outcome to data/browser_session.json for
  the desk to record.

* CLI mode (``run_cli``): APM.py and LBCT.py open the same browser, wait for
  the user to sign in and press Enter, then run the same checks and print the
  results.

Automatic container checks are implemented for APM (TERMPoint) and LBCT.
Other terminals open so the user can sign in and record the finding by hand.
Public empty-return schedules (no sign-in) are captured by screenshot.py.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
from datetime import datetime
from pathlib import Path

from dotenv import load_dotenv

ROOT = Path(__file__).resolve().parent
DATA = ROOT / "data"
SHOTS = ROOT / "screenshots"

TERMINALS = {
    "apm": {"url_key": "TREMPOINT_URL", "fallback": "https://termpoint.apmterminals.com/"},
    "fenix": {
        "url_key": "FENIX_APPT_URL",
        "fallback": "https://portal.fenixmarineservices.com/apptmt-app/home",
    },
    "wbct": {"url_key": "WBCT_APPT_URL", "fallback": "https://voyagertrack.portsamerica.com/"},
    "ets": {"url_key": "ETS_APPT_URL", "fallback": "https://www.emodal.com/"},
    "trapac": {"url_key": "TRAPAC_APPT_URL", "fallback": "https://www.emodal.com/"},
    "yti": {"url_key": "YTI_APPT_URL", "fallback": "https://yti.com/"},
    "lbct": {"url_key": "LBCT_LOGIN_URL", "fallback": "https://portal.lbct.com/LoginWithUrl"},
    "tti": {"url_key": "TTI_APPT_URL", "fallback": "https://www.emodal.com/"},
    "its": {"url_key": "ITS_APPT_URL", "fallback": "https://www.emodal.com/"},
    "pct": {"url_key": "PCT_APPT_URL", "fallback": "https://www.emodal.com/"},
    "piera": {"url_key": "PIER_A_APPT_URL", "fallback": "https://www.emodal.com/"},
}

# Public empty-return schedules. No sign-in. Keep in step with empty_url in
# dashboard/src/model.rs.
PUBLIC_PAGES = {
    "apm": {
        "url_key": "TERMINAL_URL",
        "fallback": "https://www.apmterminals.com/en/los-angeles/empty-container-return",
    },
    "ets": {"url_key": "ETS_EMPTY_URL", "fallback": "https://www.etslink.com/"},
    "trapac": {
        "url_key": "TRAPAC_EMPTY_URL",
        "fallback": "https://losangeles.trapac.com/empty-returns/",
    },
    "yti": {
        "url_key": "YTI_EMPTY_URL",
        "fallback": "https://yti.com/terminal-schedules/empty-receiving-schedule/",
    },
    "its": {
        "url_key": "ITS_EMPTY_URL",
        "fallback": "https://www.itslb.com/empty-receiving-updates/",
    },
}

REFUSAL_PHRASES = (
    "no appointment",
    "no appointments",
    "no slot",
    "no slots",
    "not available",
    "no availability",
    "unavailable",
    "not accepting",
    "no empty",
)


class Session:
    def __init__(self, session_id: str, terminal_id: str) -> None:
        self.path = DATA / "browser_session.json"
        self.command_path = DATA / "browser_command.json"
        self.session_id = session_id
        self.terminal_id = terminal_id
        self.rev = 0
        self.status = "starting"
        self.message = "Opening the browser."
        self.results: list[dict] = []
        self.date = ""
        self.shift = ""

    def publish(
        self,
        status: str | None = None,
        message: str | None = None,
        results: list[dict] | None = None,
        bump: bool = False,
    ) -> None:
        if status is not None:
            self.status = status
        if message is not None:
            self.message = message
        if results is not None:
            self.results = results
        if bump:
            self.rev += 1
        shift = self.shift if self.shift in {"1st", "2nd"} else None
        payload = {
            "rev": self.rev,
            "session_id": self.session_id,
            "status": self.status,
            "terminal_id": self.terminal_id,
            "message": self.message,
            "date": self.date,
            "shift": shift,
            "results": self.results,
            "heartbeat": time.time(),
        }
        DATA.mkdir(parents=True, exist_ok=True)
        temporary = self.path.with_suffix(".json.tmp")
        temporary.write_text(json.dumps(payload, indent=2))
        temporary.replace(self.path)

    def stop_requested(self) -> bool:
        command = pop_command()
        return bool(command and command.get("action") == "close")


class Console:
    """Stand-in for Session when a CLI script drives the checks."""

    def __init__(self) -> None:
        self.date = ""
        self.shift = ""

    def publish(self, status: str | None = None, message: str | None = None, **_ignored) -> None:
        if message:
            print(message)

    def stop_requested(self) -> bool:
        return False


def normalize_container(raw: str) -> str:
    return "".join(str(raw).split()).upper()


def env_url(terminal_id: str, table: dict | None = None) -> str:
    spec = (table or TERMINALS)[terminal_id]
    value = os.getenv(spec["url_key"], "").strip()
    return value or spec["fallback"]


def classify_page(text: str) -> tuple[str, str]:
    lowered = " ".join(text.lower().split())
    if "dual only" in lowered or "dual-only" in lowered:
        return "dual_only", "Page shows dual only."
    for phrase in REFUSAL_PHRASES:
        if phrase in lowered:
            return "no_slots", f"Page says “{phrase}”."
    if "available" in lowered:
        return "open", "A return slot is offered."
    return "could_not_check", "Could not tell whether a return appointment is offered."


def save_shot(page, terminal_id: str, container: str) -> str:
    folder = SHOTS / terminal_id
    folder.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    path = folder / f"{container}_no_return_{stamp}.png"
    page.screenshot(path=str(path), full_page=True)
    return str(path)


def pop_command() -> dict | None:
    path = DATA / "browser_command.json"
    if not path.exists():
        return None
    try:
        raw = path.read_text()
    except OSError:
        return None
    try:
        path.unlink(missing_ok=True)
    except OSError:
        return None
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        return None
    return data if isinstance(data, dict) else None


def settle(page) -> None:
    try:
        page.wait_for_load_state("networkidle", timeout=15000)
    except Exception:
        page.wait_for_timeout(800)


def page_text(page) -> str:
    try:
        return page.locator("body").inner_text(timeout=8000)
    except Exception:
        return ""


def refusal_result(page, terminal_id: str, container: str, finding: str, note: str) -> dict:
    screenshot = ""
    if finding in {"no_slots", "closed", "dual_only"}:
        try:
            screenshot = save_shot(page, terminal_id, container)
        except Exception:
            screenshot = ""
    return {
        "container": container,
        "finding": finding,
        "note": note,
        "screenshot": screenshot,
    }


def check_apm(page, containers: list[str], session) -> tuple[list[dict], bool]:
    if page.locator('input[name="Login_Nm"]').count() > 0:
        raise RuntimeError("Still on the TERMPoint login page. Sign in, then check again.")

    results = []
    for container in containers:
        if session.stop_requested():
            return results, True
        session.publish(status="checking", message=f"Checking {container} on APM.")
        try:
            results.append(_one_apm(page, container))
        except Exception as exc:
            results.append(
                {
                    "container": container,
                    "finding": "could_not_check",
                    "note": str(exc),
                    "screenshot": "",
                }
            )
    return results, False


def _one_apm(page, container: str) -> dict:
    base = env_url("apm").rstrip("/")
    schedule = page.locator('a[href="/ScheduleAppointment"]')
    if schedule.count() > 0:
        schedule.first.click()
        settle(page)
    elif page.locator("#ApptTypeDdDv").count() == 0:
        page.goto(f"{base}/ScheduleAppointment", wait_until="domcontentloaded", timeout=30000)
        settle(page)

    dropdown = page.locator("#ApptTypeDdDv .ui.search.selection.dropdown")
    dropdown.first.click(timeout=10000)
    page.wait_for_timeout(500)
    empty = page.locator('div[radio="container_ero"][name="MD"]')
    empty.first.click(timeout=10000)
    settle(page)

    field = page.locator('textarea[name="containerName"]')
    field.first.wait_for(state="visible", timeout=10000)
    field.first.fill("")
    field.first.fill(container)

    submit = page.get_by_role("button", name=re.compile(r"submit|continue|next", re.I))
    if submit.count() == 0:
        submit = page.locator("button.ui.primary.button")
    submit.first.click(timeout=10000)
    settle(page)

    text = page_text(page)
    finding, note = classify_page(text)
    if finding in {"no_slots", "dual_only"}:
        return refusal_result(page, "apm", container, finding, note)

    icon = page.locator("i.icon-rc-datepicker_calendar")
    icon.first.click(timeout=8000)
    page.locator(".react-datepicker-container").first.wait_for(state="visible", timeout=8000)
    day = str(datetime.now().day)
    page.locator(".react-datepicker-picker.day.current span").filter(
        has_text=re.compile(rf"^{day}$")
    ).first.click(timeout=8000)
    page.wait_for_timeout(500)

    page.locator("#ipgrid_0_slot").click(timeout=8000)
    menu = page.locator("#ipgrid_0_slot .menu")
    menu.wait_for(state="visible", timeout=8000)
    items = menu.locator(".item")
    labels = []
    for index in range(items.count()):
        label = " ".join(items.nth(index).inner_text().split())
        if label and label.lower() not in {"select", "select slot", "-"}:
            labels.append(label)

    if not labels or any("no appointment" in label.lower() or "no slot" in label.lower() for label in labels):
        return refusal_result(
            page,
            "apm",
            container,
            "no_slots",
            "No empty-return appointment slots were offered.",
        )
    shown = ", ".join(labels[:6])
    return {
        "container": container,
        "finding": "open",
        "note": f"Slots offered: {shown}",
        "screenshot": "",
    }


def check_lbct(page, containers: list[str], session) -> tuple[list[dict], bool]:
    if page.locator("input.loginBoxInputUser").count() > 0:
        raise RuntimeError("Still on the LBCT login page. Sign in, then check again.")

    search = os.getenv("LBCT_URL", "https://portal.lbct.com/CargoSearch").strip()
    if not search:
        search = "https://portal.lbct.com/CargoSearch"
    results = []
    for container in containers:
        if session.stop_requested():
            return results, True
        session.publish(status="checking", message=f"Checking {container} on LBCT.")
        try:
            results.append(_one_lbct(page, container, search))
        except Exception as exc:
            results.append(
                {
                    "container": container,
                    "finding": "could_not_check",
                    "note": str(exc),
                    "screenshot": "",
                }
            )
    return results, False


def _one_lbct(page, container: str, search: str) -> dict:
    page.goto(search, wait_until="domcontentloaded", timeout=45000)
    field = page.locator('input[name="containerNumber"]')
    field.first.wait_for(state="visible", timeout=15000)
    field.first.fill("")
    field.first.fill(container)
    page.locator('button[type="submit"]').first.click(timeout=10000)
    settle(page)
    finding, note = classify_page(page_text(page))
    if finding == "open":
        return {
            "container": container,
            "finding": "open",
            "note": note,
            "screenshot": "",
        }
    if finding == "could_not_check":
        return {
            "container": container,
            "finding": "could_not_check",
            "note": note,
            "screenshot": "",
        }
    return refusal_result(page, "lbct", container, finding, note)


CHECKERS = {
    "apm": check_apm,
    "lbct": check_lbct,
}


def run_checks(page, terminal_id: str, containers: list[str], session) -> str:
    checker = CHECKERS.get(terminal_id)
    if checker is None:
        session.publish(
            status="waiting_for_login",
            message="Sign-in is open. Automatic container check is not set up for this terminal yet. Record the finding on the board.",
        )
        return "waiting"
    try:
        results, closed = checker(page, containers, session)
    except Exception as exc:
        session.publish(status="waiting_for_login", message=str(exc))
        return "waiting"
    if closed:
        session.publish(
            status="closed",
            message="Browser closed.",
            results=results,
            bump=True,
        )
        return "closed"
    refused = sum(1 for row in results if row["finding"] in {"no_slots", "closed", "dual_only"})
    session.publish(
        status="done",
        message=f"Checked {len(results)} containers. {refused} had no straight empty return.",
        results=results,
        bump=True,
    )
    return "done"


def serve(terminal_id: str, session_id: str) -> int:
    session = Session(session_id, terminal_id)
    session.publish(bump=True)
    try:
        from playwright.sync_api import sync_playwright
    except ImportError:
        session.publish(
            status="error",
            message="Playwright is not installed. Run: pip install -r requirements.txt && playwright install chromium",
            bump=True,
        )
        return 1

    url = env_url(terminal_id)
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=False)
        page = browser.new_page(viewport={"width": 1400, "height": 900})
        try:
            page.goto(url, wait_until="domcontentloaded", timeout=60000)
            message = "Browser is open. Sign in yourself, then check containers."
            if terminal_id not in CHECKERS:
                message = "Browser is open. Sign in, then record what you find on the board. Automatic slot check is ready for APM and LBCT."
            session.publish(status="waiting_for_login", message=message)

            while browser.is_connected():
                command = pop_command()
                if command and command.get("action") == "close":
                    session.publish(status="closed", message="Browser closed.", bump=True)
                    break
                if command and command.get("action") == "check":
                    containers = []
                    for raw in command.get("containers") or []:
                        container = normalize_container(raw)
                        if container:
                            containers.append(container)
                    session.date = str(command.get("date") or "")
                    session.shift = str(command.get("shift") or "")
                    if not containers:
                        session.publish(message="Enter at least one container to check.")
                    else:
                        outcome = run_checks(page, terminal_id, containers, session)
                        if outcome == "closed":
                            break
                try:
                    page.wait_for_timeout(400)
                except Exception:
                    session.publish(status="closed", message="Browser window was closed.", bump=True)
                    break
        finally:
            try:
                browser.close()
            except Exception:
                pass
    if session.status != "closed":
        session.publish(status="closed", message="Browser closed.", bump=True)
    return 0


def run_cli(terminal_id: str, containers: list[str]) -> int:
    """Open the terminal sign-in page, let the user sign in, then check containers.

    Used by APM.py and LBCT.py. Containers must be given; there is no default.
    """
    load_dotenv(ROOT / ".env")
    containers = [c for c in (normalize_container(raw) for raw in containers) if c]
    if not containers:
        print("Give at least one container number.", file=sys.stderr)
        return 2
    if terminal_id not in CHECKERS:
        print(f"No automatic check for {terminal_id}.", file=sys.stderr)
        return 2

    from playwright.sync_api import sync_playwright

    console = Console()
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=False)
        page = browser.new_page(viewport={"width": 1400, "height": 900})
        try:
            url = env_url(terminal_id)
            print(f"Opening {url}")
            page.goto(url, wait_until="domcontentloaded", timeout=60000)
            input("Sign in in the browser window (including any captcha), then press Enter here... ")
            results, _closed = CHECKERS[terminal_id](page, containers, console)
        finally:
            try:
                browser.close()
            except Exception:
                pass

    for row in results:
        line = f"{row['container']}: {row['finding']}  {row['note']}"
        if row.get("screenshot"):
            line += f"\n    screenshot: {row['screenshot']}"
        print(line)
    return 0


def _self_test() -> None:
    finding, _note = classify_page("No appointment slots for this container")
    assert finding == "no_slots", finding
    finding, _note = classify_page("Dual only for 40 foot")
    assert finding == "dual_only", finding
    finding, _note = classify_page("Appointment available at 14:00")
    assert finding == "open", finding
    finding, _note = classify_page("Return is not available today")
    assert finding == "no_slots", finding
    finding, _note = classify_page("Container inquiry")
    assert finding == "could_not_check", finding


def main() -> int:
    load_dotenv(ROOT / ".env")
    parser = argparse.ArgumentParser(description="Open a terminal browser for the Empty Return Desk")
    parser.add_argument("--terminal", choices=sorted(TERMINALS))
    parser.add_argument("--session", default="")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        _self_test()
        print("ok")
        return 0
    if not args.terminal or not args.session:
        parser.error("--terminal and --session are required")
    try:
        return serve(args.terminal, args.session)
    except KeyboardInterrupt:
        return 0


if __name__ == "__main__":
    sys.exit(main())
