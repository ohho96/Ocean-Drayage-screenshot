"""MCP server so Claude Code can see the Empty Return Desk while improving it.

Runs over stdio (no network port). It talks to the terminal browsers the
dashboard opened through the same files the dashboard uses
(data/browsers/<terminal>.json / .cmd.json), and to the scripts directly.

What it can do: list browsers and their sign-in state, describe or
screenshot the page a browser is on, test CSS selectors, read browser logs
and the desk's saved state, take public-schedule screenshots, test page
classification, and run a container check in a browser that is signed in.

What it cannot do, by design: read saved logins (only whether one is saved),
read what is typed in any field, sign in, type, click or navigate.

Registered for Claude Code in .mcp.json. Run by hand: .venv/bin/python mcp_server.py
"""

from __future__ import annotations

import json
import subprocess
import sys
import time
import uuid
from pathlib import Path

from mcp.server.fastmcp import FastMCP, Image

import desk_browser as desk

ROOT = Path(__file__).resolve().parent
BROWSERS = desk.BROWSERS
STALE_SECONDS = 45

mcp = FastMCP("empty-return-desk")


def _terminal(terminal: str) -> str:
    terminal = terminal.strip().lower()
    if terminal not in desk.TERMINALS:
        raise ValueError(f"Unknown terminal {terminal!r}. Use one of: {', '.join(sorted(desk.TERMINALS))}")
    return terminal


def _status(terminal: str) -> dict | None:
    try:
        return json.loads((BROWSERS / f"{terminal}.json").read_text())
    except (OSError, ValueError):
        return None


def _live(terminal: str) -> dict:
    status = _status(terminal)
    if not status or status.get("status") == "closed":
        raise RuntimeError(f"No {terminal} browser is open. Open it from the dashboard first.")
    if time.time() - float(status.get("heartbeat") or 0) > STALE_SECONDS:
        raise RuntimeError(f"The {terminal} browser is not responding (no update for {STALE_SECONDS}s).")
    return status


def _send(terminal: str, command: dict) -> None:
    BROWSERS.mkdir(parents=True, exist_ok=True)
    path = BROWSERS / f"{terminal}.cmd.json"
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(command))
    temporary.replace(path)


def _inspect(terminal: str, request: dict, timeout: float = 25.0) -> dict:
    _live(terminal)
    request_id = uuid.uuid4().hex
    answer_path = BROWSERS / f"{terminal}.inspect.json"
    answer_path.unlink(missing_ok=True)
    _send(terminal, {"action": "inspect", "request_id": request_id, **request})
    end = time.time() + timeout
    while time.time() < end:
        try:
            answer = json.loads(answer_path.read_text())
            if answer.get("request_id") == request_id:
                return answer
        except (OSError, ValueError):
            pass
        time.sleep(0.3)
    raise RuntimeError(
        "The browser did not answer in time. It may be busy checking containers; try again shortly."
    )


@mcp.tool()
def list_browsers() -> list[dict]:
    """List terminal browsers the dashboard has open: sign-in state, address, tabs, last message."""
    rows = []
    if not BROWSERS.exists():
        return rows
    for path in sorted(BROWSERS.glob("*.json")):
        if path.name.endswith((".cmd.json", ".inspect.json")):
            continue
        try:
            data = json.loads(path.read_text())
        except (OSError, ValueError):
            continue
        age = time.time() - float(data.get("heartbeat") or 0)
        state = data.get("login") or data.get("status")
        if data.get("status") == "closed":
            state = "closed"
        elif age > STALE_SECONDS:
            state = "not_responding"
        elif data.get("status") == "checking":
            state = "checking"
        rows.append(
            {
                "terminal": data.get("terminal_id") or path.stem,
                "state": state,
                "url": data.get("url", ""),
                "tabs": data.get("tabs", []),
                "message": data.get("message", ""),
                "seconds_since_update": round(age, 1),
            }
        )
    return rows


@mcp.tool()
def inspect_page(terminal: str, max_chars: int = 6000, full_page: bool = False) -> list:
    """Describe the page a terminal browser is on now: address, frames, form fields
    (name/id/label/visible/has_value, never values), buttons, links, table previews,
    visible text, and a screenshot. Use this to write or fix selectors."""
    terminal = _terminal(terminal)
    answer = _inspect(
        terminal, {"kind": "page", "max_chars": max_chars, "screenshot": True, "full_page": full_page}
    )
    shot = answer.pop("screenshot", "")
    content: list = [json.dumps(answer, indent=1)]
    if shot and Path(shot).exists():
        content.append(Image(path=shot))
    return content


@mcp.tool()
def query_selector(terminal: str, selector: str) -> dict:
    """Test a CSS selector in a terminal browser (all frames). Returns match counts and
    up to 5 outerHTML snippets per frame with every value attribute removed."""
    terminal = _terminal(terminal)
    return _inspect(terminal, {"kind": "query", "selector": selector})


@mcp.tool()
def check_containers(
    terminal: str, containers: list[str], date: str = "", shift: str = "1st", timeout_seconds: int = 240
) -> dict:
    """Run the automatic container check (APM or LBCT) in a browser the user has signed in to.
    Results are also recorded by the dashboard if it is running. Read-only lookups on the
    terminal site; never signs in."""
    terminal = _terminal(terminal)
    if terminal not in desk.CHECKERS:
        raise ValueError(f"No automatic check for {terminal}. Available: {', '.join(desk.CHECKERS)}")
    status = _live(terminal)
    if status.get("login") != "online":
        raise RuntimeError("That browser is not signed in. Ask the user to sign in first.")
    if shift not in {"1st", "2nd"}:
        raise ValueError("shift must be '1st' or '2nd'")
    cleaned = [c for c in (desk.normalize_container(c) for c in containers) if c]
    if not cleaned:
        raise ValueError("Give at least one container number.")
    start_rev = int(status.get("rev") or 0)
    _send(terminal, {"action": "check", "containers": cleaned, "date": date or time.strftime("%Y-%m-%d"), "shift": shift})
    end = time.time() + timeout_seconds
    while time.time() < end:
        time.sleep(1.0)
        status = _status(terminal) or {}
        if int(status.get("rev") or 0) > start_rev:
            return {"status": status.get("status"), "message": status.get("message"), "results": status.get("results", [])}
    raise RuntimeError("Check did not finish in time; it may still be running. See list_browsers.")


@mcp.tool()
def screenshot_schedule(terminal: str) -> list:
    """Screenshot a terminal's public empty-return schedule (no sign-in) and read any
    receiving rows from it (ITS so far). Terminals: apm, ets, trapac, yti, its."""
    terminal = terminal.strip().lower()
    if terminal not in desk.PUBLIC_PAGES:
        raise ValueError(f"No public schedule for {terminal}. Use one of: {', '.join(sorted(desk.PUBLIC_PAGES))}")
    run = subprocess.run(
        [sys.executable, str(ROOT / "screenshot.py"), "--terminal", terminal, "--json"],
        cwd=ROOT, capture_output=True, text=True, timeout=180,
    )
    lines = [line for line in run.stdout.splitlines() if line.strip().startswith("{")]
    if not lines:
        raise RuntimeError((run.stderr.strip().splitlines() or ["screenshot.py printed nothing"])[-1])
    result = json.loads(lines[-1])
    rows = result.get("receiving", [])
    summary = {k: v for k, v in result.items() if k != "receiving"}
    summary["receiving_rows"] = len(rows)
    summary["receiving_sample"] = rows[:12]
    content: list = [json.dumps(summary, indent=1)]
    shot = result.get("screenshot")
    if shot and Path(shot).exists():
        content.append(Image(path=shot))
    return content


@mcp.tool()
def classify_text(text: str) -> dict:
    """Run the page classifier on some text: what finding would a page with this text get?"""
    finding, note = desk.classify_page(text)
    return {"finding": finding, "note": note}


@mcp.tool()
def read_browser_log(terminal: str, lines: int = 80) -> str:
    """Last lines of a terminal browser's log (data/browsers/<terminal>.log)."""
    terminal = _terminal(terminal)
    path = BROWSERS / f"{terminal}.log"
    if not path.exists():
        return "(no log yet)"
    return "\n".join(path.read_text(errors="replace").splitlines()[-max(1, min(lines, 500)):])


def _state_path() -> Path:
    return desk.credentials_dir().parent / "state.json"


@mcp.tool()
def desk_state(date: str = "", recent: int = 20) -> dict:
    """Summary of the dashboard's saved state: checks and receiving rows (optionally for one
    date, YYYY-MM-DD) and the most recent evidence entries."""
    try:
        state = json.loads(_state_path().read_text())
    except (OSError, ValueError) as exc:
        return {"error": f"Could not read {_state_path()}: {exc}"}
    checks = state.get("checks", {})
    if date:
        checks = {k: v for k, v in checks.items() if k.startswith(date)}
    receivings = state.get("receivings", [])
    if date:
        receivings = [r for r in receivings if r.get("date") == date]
    return {
        "container": state.get("container"),
        "shift": state.get("shift"),
        "checks": checks,
        "receiving_rows": len(receivings),
        "receivings": receivings[:60],
        "recent_evidence": state.get("log", [])[-max(0, min(recent, 200)):],
    }


@mcp.tool()
def saved_logins() -> dict:
    """Which login systems have a saved login. Never returns IDs or passwords."""
    folder = desk.credentials_dir()
    return {account: (folder / f"{account}.json").exists() for account in desk.ACCOUNT_NAMES}


if __name__ == "__main__":
    mcp.run()
