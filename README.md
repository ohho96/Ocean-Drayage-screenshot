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
Dashboard ──spawns one per terminal──▶ desk_browser.py --terminal apm --session <id>
    │   writes data/browsers/apm.cmd.json  {"action": "check" | "focus" | "close", ...}
    │   reads  data/browsers/apm.json      {status, login, url, tabs[...], results[...], heartbeat}
    │
    └─spawns──▶ screenshot.py --terminal its --json   → {"screenshot": "<path>"}
```

## Logins: saved by you, filled in, signed in by you

Save your terminal logins on the dashboard's **Logins** screen. There is one
login per login system, and terminals that share a system share the login:

| Login system | Terminals |
|--------------|-----------|
| TERMPoint | APM |
| LBCT portal | LBCT |
| eModal | ETS, TraPac, TTI, ITS, PCT, Pier A |
| Fenix portal | Fenix |
| VoyagerTrack | West Basin |
| YTI | YTI |

- **Where they are saved:** `~/Library/Application Support/EmptyReturnDesk/credentials/`,
  one `<system>.json` file each. The folder is owner-only (700) and the files
  are owner-only (600), so only your Mac user can read them. They are outside
  the git repo, so they can't be committed by accident. Set
  `DESK_CREDENTIALS_DIR` to use another folder.
- **What the browser does:** when a sign-in page shows up, it fills in the
  saved ID and password, in an iframe too. It **never presses Sign in**. You
  solve any captcha and sign in yourself. It also never overwrites a field
  you've started typing in, and it fills each sign-in page once, so a field
  you clear stays clear.
- **Editing:** the saved password is never shown on the Logins screen. To
  change only the ID, leave the password blank and press Save. Delete asks you
  to confirm.
- **Passwords never appear** in status files, logs or messages.
- Login IDs and passwords in `.env` are not used. Enter them on the Logins
  screen instead.

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
     window. If a login is saved, it is filled in; press Sign in there. You can have a browser open for several terminals
     at once.
   - **Check containers.** Shown in the banner for APM (TERMPoint) and LBCT.
     It checks each container. When no return appointment is offered, a
     screenshot is saved to `screenshots/<terminal>/` and the finding is
     recorded on the board.
   - **Screenshot empty schedule.** For terminals with a public schedule (APM,
     ETS, TraPac, YTI, ITS). Captures the page without sign-in and fills in
     the screenshot path for the finding. For ITS it also reads the schedule
     table (line × size × shift, both days shown) and fills the **Receiving**
     board automatically.
3. Keep an eye on the **Browsers** list under the header. It shows every
   open browser with:
   - **Status.** Starting, Needs sign-in, Online, Checking, **Signed out —
     sign in again** (it was online and the site is asking for a sign-in
     again, usually an expired session), Not responding (no update for 45
     seconds) or Error. Red means you need to do something; the header
     counts how many.
   - **Address.** The page the browser is on now. Hover to see every tab;
     click to copy the address.
   - **Show** brings that window to the front, **Check containers** (APM and
     LBCT, when Online) and **Close**.

   The sign-in state is checked every 3 seconds. A browser counts as needing
   sign-in when a password field (or the terminal's known login field) is
   showing, the address looks like a login page, or it is still on the page
   it opened and has never been signed in. The board row for each terminal
   also shows its browser status.
4. How automatic findings are decided. A container is marked refused (No
   slots or Dual only) only when the page clearly says so. It is marked Open
   only when APM actually lists appointment slots. Anything else, including
   error and session-expired pages, is marked **Could not check**, and a
   screenshot is saved so you can look and set the finding yourself.
5. For terminals without automatic checks, look in the open browser, then set
   the finding (Closed, No slots, Dual only, Open or Could not check) and
   record it.
6. Copy a **proof packet** for one container (it lists refusals only), the
   full board, or the receiving board.

The dashboard saves its own state as JSON. Each browser's log is in
`data/browsers/<terminal>.log`. Quitting the dashboard closes its browsers.

## Command-line scripts

You must give container numbers. There is no built-in test container.

```bash
.venv/bin/python APM.py MSMU1234567 TGHU7654321     # sign in, press Enter, check
.venv/bin/python LBCT.py MSMU1234567
.venv/bin/python screenshot.py                      # all public empty schedules
.venv/bin/python screenshot.py --terminal its       # one terminal
.venv/bin/python ssl_lookup.py                      # shipping line ↔ SCAC demo
```

`APM.py` and `LBCT.py` open a browser, fill in the saved login if there is
one, and wait for you to sign in and press Enter. They then run the same checks as the dashboard and print each result,
with the screenshot path when a container was refused.

## Files

| File | Purpose |
|------|---------|
| `desk_browser.py` | Shared engine: terminal URLs, page classification, APM and LBCT checks, dashboard file protocol, CLI runner |
| `screenshot.py` | Public empty-schedule screenshots (`--json` output for the dashboard) |
| `schedules.py` | Reads public schedule tables into receiving-board rows (ITS so far) |
| `APM.py`, `LBCT.py` | CLI wrappers around `desk_browser.run_cli` |
| `ssl_lookup.py` | Shipping line name ↔ SCAC code lookup |
| `dashboard/src/model.rs` | Terminals, findings, proof packet and board text |
| `dashboard/src/browser.rs` | Starts and talks to the Python processes |
| `dashboard/src/main.rs` | UI |
| `dashboard/src/store.rs` | Saved desk state |
| `dashboard/src/credentials.rs` | Saved logins (owner-only files) |

## Tests

```bash
.venv/bin/python desk_browser.py --self-test
.venv/bin/python schedules.py
cd dashboard && cargo test
```
