# Empty Return Desk

Tools for LA/Long Beach drayage that record proof when a marine terminal
won't take an empty container back. The proof is dated, shift-stamped
findings and screenshots of each refusal. You can use it to dispute per-diem
or detention charges.

The tool has two parts that work together:

| Part | What it does |
|------|--------------|
| **Dashboard** (`dashboard/`, Rust + egui) | Desktop app. Track containers across the 11 LA/LB terminals, record findings, keep an evidence log, copy proof packets and boards. |
| **Python engine** (`desk_browser.py`, `screenshot.py`) | Playwright browser automation. The dashboard starts it for you. You can also run it from the command line. |

```
Dashboard ──spawns──▶ desk_browser.py --terminal apm --session <id>
    │   writes data/browser_command.json   {"action": "check" | "close", ...}
    │   reads  data/browser_session.json   {status, message, results[...]}
    │
    └─spawns──▶ screenshot.py --terminal its --json   → {"screenshot": "<path>"}
```

## Sign-in is always done by you

Neither the dashboard nor the scripts read, store or type passwords. The
browser opens on the terminal's sign-in page. You sign in yourself, including
any captcha, and then start the check. Login IDs and passwords in `.env` are
not used.

## Setup

```bash
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
.venv/bin/python -m playwright install chromium
```

The dashboard uses `.venv/bin/python` automatically. If there is no `.venv`,
it falls back to `python3` on your PATH, and that Python must have Playwright
installed.

To build the dashboard you need Rust (`dashboard/rust-toolchain.toml` pins
1.95.0):

```bash
cd dashboard && cargo run --release
```

### `.env` (optional)

Only URL overrides are read, for example `TREMPOINT_URL`, `LBCT_LOGIN_URL`,
`LBCT_URL`, `ITS_EMPTY_URL`, `TRAPAC_EMPTY_URL` and `YTI_EMPTY_URL`. If a key
is missing, the built-in default URL is used. `.env` is gitignored.

## Using the dashboard

1. Enter the container, date and shift at the top. For a batch, put one
   container per line in the container list.
2. Pick a terminal on the board. The right-hand panel offers these actions:
   - **Open browser.** Opens the terminal's appointment portal in a Playwright
     window. Sign in there.
   - **Check containers.** Shown in the banner for APM (TERMPoint) and LBCT.
     It checks each container. When no return appointment is offered, a
     screenshot is saved to `screenshots/<terminal>/` and the finding is
     recorded on the board.
   - **Screenshot empty schedule.** For terminals with a public schedule (APM,
     ETS, TraPac, YTI, ITS). Captures the page without sign-in and fills in
     the screenshot path for the finding.
3. For terminals without automatic checks, look in the open browser, then set
   the finding (Closed, No slots, Dual only, Open or Could not check) and
   record it.
4. Copy a **proof packet** for one container (it lists refusals only), the
   full board, or the receiving board.

The dashboard saves its own state as JSON. The browser log is in
`data/browser.log`.

## Command-line scripts

You must give container numbers. There is no built-in test container.

```bash
.venv/bin/python APM.py MSMU1234567 TGHU7654321     # sign in, press Enter, check
.venv/bin/python LBCT.py MSMU1234567
.venv/bin/python screenshot.py                      # all public empty schedules
.venv/bin/python screenshot.py --terminal its       # one terminal
.venv/bin/python ssl_lookup.py                      # shipping line ↔ SCAC demo
```

`APM.py` and `LBCT.py` open a browser and wait for you to sign in and press
Enter. They then run the same checks as the dashboard and print each result,
with the screenshot path when a container was refused.

## Files

| File | Purpose |
|------|---------|
| `desk_browser.py` | Shared engine: terminal URLs, page classification, APM and LBCT checks, dashboard file protocol, CLI runner |
| `screenshot.py` | Public empty-schedule screenshots (`--json` output for the dashboard) |
| `APM.py`, `LBCT.py` | CLI wrappers around `desk_browser.run_cli` |
| `ssl_lookup.py` | Shipping line name ↔ SCAC code lookup |
| `dashboard/src/model.rs` | Terminals, findings, proof packet and board text |
| `dashboard/src/browser.rs` | Starts and talks to the Python processes |
| `dashboard/src/main.rs` | UI |
| `dashboard/src/store.rs` | Saved desk state |

## Tests

```bash
.venv/bin/python desk_browser.py --self-test
cd dashboard && cargo test
```
