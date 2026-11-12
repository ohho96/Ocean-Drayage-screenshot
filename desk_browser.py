"""Terminal browser engine shared by the Empty Return Desk and the CLI scripts.

Two ways in:

* Dashboard mode (``--terminal ID --session ID``): the Rust dashboard starts
  one process per terminal, so several browsers can be open at once. Each
  opens the terminal's sign-in page and waits. The user signs in by hand,
  including any captcha. This program never reads or types a password.

  Every few seconds it writes data/browsers/<terminal>.json with the address
  of each open tab and whether the page is asking for a sign-in
  ("needs_login"), signed in ("online") or was signed in and is now asking
  again ("logged_out"). The dashboard writes commands to
  data/browsers/<terminal>.cmd.json: "check" (look up containers), "focus"
  (bring the window to the front) and "close". mcp_server.py may also send
  "inspect" (describe the current page for development); the answer goes to
  data/browsers/<terminal>.inspect.json and never includes field values. A container is screenshotted
  when the page does not clearly offer a return, and the outcome goes back in
  the status file for the desk to record.

  When a sign-in page is showing and a login is saved for that terminal's
  login system, the browser fills in the ID and password from the
  credentials folder (see ``credentials_dir``). It never presses Sign in and
  never overwrites fields the user has started typing in. The user solves
  any captcha and signs in. Passwords are never written to logs or status.

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

# Pages that are errors or sign-in prompts, not a terminal's answer.
ERROR_PHRASES = (
    "service unavailable",
    "temporarily unavailable",
    "server error",
    "something went wrong",
    "page not found",
    "access denied",
    "session expired",
    "session has expired",
    "session timed out",
    "please log in",
    "please sign in",
    "bad gateway",
    "gateway timeout",
)

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


BROWSERS = DATA / "browsers"

# Visible fields that mean "this page wants a sign-in".
LOGIN_SELECTORS = {
    "apm": 'input[name="Login_Nm"]',
    "lbct": "input.loginBoxInputUser",
}
# Which saved login each terminal's sign-in page uses. Keep in step with
# Terminal::account in dashboard/src/model.rs.
ACCOUNT_OF = {
    "apm": "termpoint",
    "lbct": "lbct",
    "fenix": "fenix",
    "wbct": "voyagertrack",
    "yti": "yti",
}
ACCOUNT_NAMES = {
    "termpoint": "TERMPoint",
    "lbct": "LBCT portal",
    "emodal": "eModal",
    "fenix": "Fenix portal",
    "voyagertrack": "VoyagerTrack",
    "yti": "YTI",
}
# Known sign-in fields: (ID field, password field).
FILL_SELECTORS = {
    "apm": ('input[name="Login_Nm"]', 'input[name="Login_Pwd"]'),
    "lbct": ("input.loginBoxInputUser", "input.loginBoxInputPassword"),
}

# Marks the ID and password fields of the first visible login form in a frame.
MARK_LOGIN_JS = """() => {
  const visible = el => !!(el.offsetWidth || el.offsetHeight || el.getClientRects().length)
    && getComputedStyle(el).visibility !== 'hidden';
  document.querySelectorAll('[data-desk-user],[data-desk-pass]').forEach(el => {
    el.removeAttribute('data-desk-user'); el.removeAttribute('data-desk-pass');
  });
  const pass = [...document.querySelectorAll('input[type=password]')].find(visible);
  if (!pass) return false;
  const scope = pass.form || document;
  const kinds = ['', 'text', 'email', 'tel'];
  let user = null;
  for (const el of scope.querySelectorAll('input')) {
    if (el === pass) break;
    if (kinds.includes((el.getAttribute('type') || '').toLowerCase()) && visible(el) && !el.disabled && !el.readOnly) user = el;
  }
  if (!user) return false;
  user.setAttribute('data-desk-user', '1');
  pass.setAttribute('data-desk-pass', '1');
  return true;
}"""


def account_for(terminal_id: str) -> str:
    return ACCOUNT_OF.get(terminal_id, "emodal")


def credentials_dir() -> Path:
    override = os.getenv("DESK_CREDENTIALS_DIR", "").strip()
    if override:
        return Path(override)
    home = Path.home()
    if sys.platform == "darwin":
        return home / "Library" / "Application Support" / "EmptyReturnDesk" / "credentials"
    return home / ".local" / "share" / "empty-return-desk" / "credentials"


def load_login(account: str) -> tuple[str, str] | None:
    """Saved (ID, password) for a login system, read fresh each time."""
    if account not in ACCOUNT_NAMES:
        return None
    try:
        data = json.loads((credentials_dir() / f"{account}.json").read_text())
    except (OSError, ValueError):
        return None
    user = str(data.get("user_id") or "").strip()
    password = str(data.get("password") or "")
    return (user, password) if user and password else None


def fill_login(page, terminal_id: str) -> str | None:
    """Fill the saved login into a visible sign-in form. Never submits.

    Returns "filled", "kept" (a form is there but the user already typed in
    it, or no login is saved), or None (no sign-in form found yet).
    """
    login = load_login(account_for(terminal_id))
    for frame in page.frames:
        try:
            user_field = pass_field = None
            known = FILL_SELECTORS.get(terminal_id)
            if known and frame == page.main_frame:
                u = frame.locator(f"{known[0]}:visible").first
                p = frame.locator(f"{known[1]}:visible").first
                if u.count() and p.count():
                    user_field, pass_field = u, p
            if user_field is None and frame.evaluate(MARK_LOGIN_JS):
                user_field = frame.locator("[data-desk-user]").first
                pass_field = frame.locator("[data-desk-pass]").first
            if user_field is None:
                continue
            if login is None:
                return "kept"
            if user_field.input_value() or pass_field.input_value():
                return "kept"
            user_field.fill(login[0])
            pass_field.fill(login[1])
            return "filled"
        except Exception:
            continue
    return None


# Describes the current page for development (mcp_server.py). Lists fields by
# name/id/label but NEVER their values; only whether a field has a value.
DESCRIBE_PAGE_JS = """(limit) => {
  const visible = el => !!(el.offsetWidth || el.offsetHeight || el.getClientRects().length);
  const text = el => (el.innerText || el.textContent || '').replace(/\\s+/g, ' ').trim().slice(0, 120);
  const labelFor = el => {
    if (el.id) { const l = document.querySelector(`label[for="${CSS.escape(el.id)}"]`); if (l) return text(l); }
    const wrap = el.closest('label'); return wrap ? text(wrap) : '';
  };
  const attrs = el => ({ id: el.id || '', name: el.getAttribute('name') || '',
    class: (el.getAttribute('class') || '').slice(0, 80) });
  const fields = [...document.querySelectorAll('input, textarea, select')].slice(0, limit).map(el => ({
    tag: el.tagName.toLowerCase(), type: (el.getAttribute('type') || '').toLowerCase(), ...attrs(el),
    placeholder: el.getAttribute('placeholder') || '', aria: el.getAttribute('aria-label') || '',
    label: labelFor(el), visible: visible(el), has_value: !!el.value,
    options: el.tagName === 'SELECT' ? [...el.options].slice(0, 15).map(o => o.text.trim()) : undefined,
  }));
  const buttons = [...document.querySelectorAll('button, input[type=submit], input[type=button], [role=button]')]
    .slice(0, limit).map(el => ({ text: text(el) || el.getAttribute('value') || '', type: el.getAttribute('type') || '',
      ...attrs(el), visible: visible(el) }));
  const links = [...document.querySelectorAll('a[href]')].filter(visible).slice(0, limit)
    .map(el => ({ text: text(el), href: el.getAttribute('href') }));
  const tables = [...document.querySelectorAll('table')].slice(0, 10).map(t => ({
    rows: t.rows.length, first_rows: [...t.rows].slice(0, 4).map(r => [...r.cells].map(c => text(c))) }));
  return { fields, buttons, links, tables };
}"""

# Outer HTML for a selector, with every value attribute removed.
QUERY_JS = """([selector, limit]) => {
  const found = [...document.querySelectorAll(selector)];
  return { count: found.length, html: found.slice(0, limit).map(el => {
    const copy = el.cloneNode(true);
    [copy, ...copy.querySelectorAll('*')].forEach(n => { if (n.removeAttribute) n.removeAttribute('value'); });
    return copy.outerHTML.slice(0, 800);
  }) };
}"""


def inspect_page(page, terminal_id: str, request: dict) -> dict:
    """Answer an "inspect" command. Read-only; never returns field values."""
    kind = request.get("kind") or "page"
    max_chars = max(500, min(int(request.get("max_chars") or 6000), 40000))
    out: dict = {
        "request_id": request.get("request_id"),
        "terminal_id": terminal_id,
        "url": page.url,
        "frames": [frame.url for frame in page.frames if frame.url and frame.url != "about:blank"],
    }
    try:
        out["title"] = page.title()
    except Exception:
        out["title"] = ""
    if kind == "query":
        selector = str(request.get("selector") or "")
        results = []
        for frame in page.frames:
            try:
                found = frame.evaluate(QUERY_JS, [selector, 5])
            except Exception as exc:
                out["error"] = str(exc).splitlines()[0]
                break
            if found["count"]:
                results.append({"frame": frame.url, **found})
        out["matches"] = results
        return out
    try:
        out["page"] = page.evaluate(DESCRIBE_PAGE_JS, 60)
    except Exception as exc:
        out["page_error"] = str(exc).splitlines()[0]
    out["text"] = page_text(page)[:max_chars]
    out["login_state"] = login_state(page, terminal_id, page.url, True)
    if request.get("screenshot", True):
        folder = SHOTS / "_inspect"
        folder.mkdir(parents=True, exist_ok=True)
        path = folder / f"{terminal_id}_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"
        try:
            page.screenshot(path=str(path), full_page=bool(request.get("full_page")))
            out["screenshot"] = str(path)
        except Exception as exc:
            out["screenshot_error"] = str(exc).splitlines()[0]
    return out


def write_inspect_answer(terminal_id: str, answer: dict) -> None:
    BROWSERS.mkdir(parents=True, exist_ok=True)
    path = BROWSERS / f"{terminal_id}.inspect.json"
    temporary = path.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(answer, indent=2))
    temporary.replace(path)


LOGIN_URL_HINTS = ("login", "signin", "sign-in", "sign_in", "/auth", "logon")
PROBE_SECONDS = 3.0


def login_state(page, terminal_id: str, start_url: str, seen_online: bool) -> str:
    """Return "needs_login" or "online" for the page the user is on."""
    try:
        selector = LOGIN_SELECTORS.get(terminal_id)
        if selector and page.locator(f"{selector}:visible").count() > 0:
            return "needs_login"
        if page.locator('input[type="password"]:visible').count() > 0:
            return "needs_login"
    except Exception:
        return "needs_login" if not seen_online else "online"
    url = (page.url or "").lower()
    if any(hint in url for hint in LOGIN_URL_HINTS):
        return "needs_login"
    # Still on the page we opened and never signed in: not signed in yet.
    if not seen_online and url.rstrip("/") == start_url.lower().rstrip("/"):
        return "needs_login"
    return "online"


def tab_info(pages) -> list[dict]:
    tabs = []
    for page in pages:
        try:
            title = page.title()
        except Exception:
            title = ""
        tabs.append({"url": page.url, "title": title})
    return tabs


class Session:
    def __init__(self, session_id: str, terminal_id: str) -> None:
        self.path = BROWSERS / f"{terminal_id}.json"
        self.command_path = BROWSERS / f"{terminal_id}.cmd.json"
        self.session_id = session_id
        self.terminal_id = terminal_id
        self.rev = 0
        self.status = "starting"
        self.message = "Opening the browser."
        self.results: list[dict] = []
        self.date = ""
        self.shift = ""
        self.login = "needs_login"
        self.seen_online = False
        self.start_url = ""
        self.tabs: list[dict] = []
        self.url = ""
        self.pending: list[dict] = []
        self.fill_key: tuple | None = None

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
            "login": self.login,
            "url": self.url,
            "tabs": self.tabs,
            "date": self.date,
            "shift": shift,
            "results": self.results,
            "heartbeat": time.time(),
        }
        BROWSERS.mkdir(parents=True, exist_ok=True)
        temporary = self.path.with_suffix(".json.tmp")
        temporary.write_text(json.dumps(payload, indent=2))
        temporary.replace(self.path)

    def next_command(self) -> dict | None:
        if self.pending:
            return self.pending.pop(0)
        return pop_command(self.command_path)

    def stop_requested(self) -> bool:
        command = pop_command(self.command_path)
        if not command:
            return False
        if command.get("action") == "close":
            return True
        self.pending.append(command)
        return False

    def try_fill(self, page) -> None:
        """Fill the saved login once per sign-in page load."""
        key = (id(page), page.url)
        if key == self.fill_key:
            return
        outcome = fill_login(page, self.terminal_id)
        if outcome is None:
            return
        self.fill_key = key
        name = ACCOUNT_NAMES[account_for(self.terminal_id)]
        if outcome == "filled":
            self.message = f"Saved {name} login filled in. Solve any captcha and press Sign in."
        elif load_login(account_for(self.terminal_id)) is None:
            self.message = f"No saved {name} login. Sign in by hand or add it under Logins."

    def probe(self, pages, current) -> None:
        """Refresh tab addresses and sign-in state; publish as a heartbeat."""
        self.tabs = tab_info(pages)
        self.url = current.url
        state = login_state(current, self.terminal_id, self.start_url, self.seen_online)
        previous = self.login
        if state == "online":
            self.seen_online = True
            self.login = "online"
            if previous != "online":
                self.message = "Signed in."
        elif self.seen_online:
            self.login = "logged_out"
            if previous != "logged_out":
                self.message = "Signed out (session expired?). Sign in again in the browser window."
        else:
            self.login = "needs_login"
        if self.login != "online":
            self.try_fill(current)
        if self.status not in {"checking", "closed", "error"}:
            self.status = "ready"
        self.publish()


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
    """Read a result page. Only a clear refusal or dual-only message counts.

    "open" is never decided from page text alone; a checker must see real
    evidence (for example APM's slot list). Error pages are never refusals.
    """
    lowered = " ".join(text.lower().split())
    for phrase in ERROR_PHRASES:
        if phrase in lowered:
            return "could_not_check", f"Page shows an error (“{phrase}”), not a terminal answer."
    if "dual only" in lowered or "dual-only" in lowered:
        return "dual_only", "Page shows dual only."
    for phrase in REFUSAL_PHRASES:
        if phrase in lowered:
            return "no_slots", f"Page says “{phrase}”."
    return "could_not_check", "No refusal message on the page. Look at the screenshot and set the finding."


def save_shot(page, terminal_id: str, container: str, finding: str = "no_return") -> str:
    folder = SHOTS / terminal_id
    folder.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    label = "check" if finding == "could_not_check" else "no_return"
    path = folder / f"{container}_{label}_{stamp}.png"
    page.screenshot(path=str(path), full_page=True)
    return str(path)


def pop_command(path: Path) -> dict | None:
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
    if finding in {"no_slots", "closed", "dual_only", "could_not_check"}:
        try:
            screenshot = save_shot(page, terminal_id, container, finding)
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
    try:
        session.command_path.unlink(missing_ok=True)
    except OSError:
        pass
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
        context = browser.new_context(viewport={"width": 1400, "height": 900})
        page = context.new_page()
        try:
            try:
                page.goto(url, wait_until="domcontentloaded", timeout=60000)
            except Exception as exc:
                session.message = f"Could not load {url}: {str(exc).splitlines()[0]}"
            session.start_url = page.url
            if terminal_id in CHECKERS:
                hint = "Sign in yourself, then check containers."
            else:
                hint = "Sign in yourself, then record what you find on the board."
            if not session.message.startswith("Could not load"):
                session.message = f"Browser is open. {hint}"
            last_probe = 0.0

            while browser.is_connected():
                pages = [p for p in context.pages if not p.is_closed()]
                if not pages:
                    break
                current = pages[-1]

                command = session.next_command()
                action = command.get("action") if command else None
                if action == "close":
                    break
                if action == "inspect":
                    try:
                        answer = inspect_page(current, terminal_id, command)
                    except Exception as exc:
                        answer = {"request_id": command.get("request_id"), "error": str(exc).splitlines()[0]}
                    write_inspect_answer(terminal_id, answer)
                if action == "focus":
                    try:
                        current.bring_to_front()
                    except Exception:
                        pass
                if action == "check":
                    containers = [
                        c for c in (normalize_container(raw) for raw in command.get("containers") or []) if c
                    ]
                    session.date = str(command.get("date") or "")
                    session.shift = str(command.get("shift") or "")
                    if not containers:
                        session.publish(message="Enter at least one container to check.")
                    else:
                        outcome = run_checks(current, terminal_id, containers, session)
                        if outcome == "closed":
                            break
                    last_probe = 0.0

                if time.time() - last_probe >= PROBE_SECONDS:
                    try:
                        session.probe(pages, current)
                    except Exception:
                        pass
                    last_probe = time.time()
                try:
                    current.wait_for_timeout(400)
                except Exception:
                    # That tab closed; the loop picks the next one or ends.
                    time.sleep(0.2)
        finally:
            try:
                browser.close()
            except Exception:
                pass
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
            for _ in range(10):
                outcome = fill_login(page, terminal_id)
                if outcome is not None:
                    break
                page.wait_for_timeout(500)
            if outcome == "filled":
                print("Saved login filled in. Solve any captcha and press Sign in.")
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
    cases = [
        ("No appointment slots for this container", "no_slots"),
        ("Dual only for 40 foot", "dual_only"),
        ("Return is not available today", "no_slots"),
        # Never "open" from text alone.
        ("Appointment available at 14:00", "could_not_check"),
        ("Container inquiry", "could_not_check"),
        # Error pages are not refusals.
        ("503 Service Unavailable", "could_not_check"),
        ("Your session has expired. No appointments shown.", "could_not_check"),
    ]
    for text, expected in cases:
        finding, _note = classify_page(text)
        assert finding == expected, (text, finding)


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
