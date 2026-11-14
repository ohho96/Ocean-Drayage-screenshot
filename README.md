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
| eModal | ETS, TraPac, TTI, ITS, PCT, Pier A (appointments) |
| ITS TMS | ITS (`tms.itslb.com`, ITS's own system) |
| ETSLink | ETS / Everport (`etslink.com`, ETS's own system) |
| Fenix portal | Fenix |
| VoyagerTrack | West Basin |
| YTI Gate Link | YTI (appointments, `cap.yti.com`) |
| YTI LYNX | YTI (terminal access, `lynx.yti.com`) |

**One browser per website.** eModal is a single site for six terminals:
Everport (ETS), ITS Long Beach, Pacific Container Terminal (PCT), SSA Pier A,
Total Terminals (TTI) and TraPac. You pick the terminal in eModal's own
terminal list. So the desk opens **one shared eModal browser** (`emodal`): opening eModal
from any of those terminals' panels starts it the first time and shows the
same window after that, and every eModal terminal on the board shows its
sign-in state. Its screenshots go to `screenshots/emodal/`. Each
terminal's own site still gets its own browser.

ITS and ETS each have their own system next to eModal: **ITS TMS** and
**ETSLink**. These are where to look for what empties they are receiving after
you sign in. Each opens in its own browser from the terminal's panel.

YTI has two sign-in systems. **Gate Link** is where YTI asks truckers to book
appointments (OOG appointments are booked by phone instead). **LYNX** is
YTI's terminal access and container system. Each has its own saved login,
its own browser and its own button in the YTI panel, and both can be open at
once.

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
- **Checking and copying:** each saved login shows its ID, with **Copy ID**,
  and the password as dots.
  - **Show** displays the password for 30 seconds. It hides sooner if you
    click Hide or leave the Logins screen.
  - **Copy password** puts it on the clipboard for 30 seconds. After that the
    clipboard is cleared, unless you've copied something else in the meantime.
- **Changing:** type a new ID or password in the Change row and press Save.
  Leave the password blank to keep the saved one. **Delete** asks you to
  confirm.
- **Passwords never appear** in status files, logs, messages or the MCP
  server. They only show on the Logins screen when you click Show.
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
`APM_EMPTY_URL`, `YTI_GATELINK_URL`, `YTI_LYNX_URL`, `ITS_TMS_URL`, `ETSLINK_URL`,
`LBCT_URL`, `ITS_EMPTY_URL`, `TRAPAC_EMPTY_URL` and `YTI_EMPTY_URL`. If a key
is missing, the built-in default URL is used. `.env` is gitignored.

## Daily empties list

Open the **Empties** screen each day and upload the empties that need a
return appointment:

- **How to upload:** drop a `.csv` or `.xlsx` file on the window, click
  **Choose file…**, or paste rows straight from Excel.
- **Columns:** container, SSL, size and type. Header names are recognised in
  many spellings (`Container #`, `Cntr No`, `SSL`, `Line`, `SCAC`, `Size`,
  `Sz/Tp`, `Type`…). Without a header, each cell is recognised by its content.
  Title rows above the header are skipped.
- **Accepted values:**
  - SSL: a name, short code or SCAC (`Yang Ming`, `YML`, `YMLU`).
  - Size and type: `40`, `40HC`, `40' HQ` or ISO codes (`22G1`, `45G1`,
    `L5G1`, `45R1`). Type is DV, HC, RF, OT, FR or TK.
- **Problems:** shown as warnings after the upload, for example an unknown
  line, a missing size, or a container listed twice.
- **History:** every upload is saved with the date, time and file name, and
  kept by date in the desk state. Use the date arrows to see another day.
  Uploading again on the same day adds to the list, and the latest row wins
  for a repeated container.

You can also type empties in under **Add containers**:

- **One:** container number, SCAC, size and type, then Enter. The SCAC can be
  3 or 4 letters (`MSC` or `MSCU`, `ONE` or `ONEY`, `HMM` or `HDMU`), and is
  filled in from the container's owner code when left blank.
- **Bulk:** paste many container numbers, one per line or separated by spaces
  or commas. Set a default SCAC, size and type for the batch. Any line can
  give its own, e.g. `ONEU1029384 ONEY 40HC`; a line that gives only a size
  gets that size's usual type (20/40 → DV, 45 → HC). A preview shows each
  container before you add it: which are new, which update one already in
  the list, unknown SCACs, and lines with no container number.

Every row in the list can be edited (SCAC, size, type) or removed.

For each empty, the screen shows where it can go back on the selected shift,
using the receiving board:

- **Single:** terminals receiving that line and size as a straight drop.
- **Dual only:** terminals that take it only on a dual transaction.
- **No terminal receiving:** hover to see which terminals said no and which
  have no information yet.

**Copy return plan** copies the list as text. **Use as container check list**
sends the containers to the APM/LBCT check.

**Dispute screenshots (waiver date windows and return locations):** the
Empties date is the waiver date: the day the box needed a return. MSC and
ONE accept a "no empty return" dispute only with screenshots taken

- the day before, 6:00 a.m. to 12:00 noon (ONE: the working day before,
  so Friday for a Monday waiver date), and
- on the waiver date, 6:00 to 9:00 a.m.,

at **every** return location the line uses, showing the container. The
**Dispute screenshots** card on Empties shows both windows for the date, and
says when one is open now and how long is left. **Return locations by line**
lists, per SSL, the terminals that need proof: it starts from every terminal
the receiving board has listed that line at, and clicking terminals turns it
into the carrier's own list (saved; **Use receiving board** goes back).

The **Proof** column counts, per container, the windows × locations that
have a screenshot showing no return (no slots, closed, dual only), e.g.
`2/4`. The time comes from the screenshot file's name stamp. Hover for each
location and window: missing, no return, unclear (look at it), or "slot
was offered" (red: there was a return, so no dispute there). **Copy proof**
adds the same list for the Board container.

**Dispute details and deadline:** under the container on the Board's
left panel, enter its **B/L**, **Invoice** number and **Inv. date**
(YYYY-MM-DD). They are kept per container. MSC and ONE take disputes only
within 30 days of the invoice date, so the panel shows "Dispute by <date>
(N days left)", amber within a week and red when due or passed; the Empties
Proof column flags "due soon" / "due!". MSC per diem invoice numbers end in
"P", and the panel warns when an MSC invoice number doesn't. **Copy proof**
includes all of it under "Dispute".

**Whole-screen evidence:** MSC and ONE want the *entire screen*: the
browser's address bar, the page, and the computer's clock with the date
(MSC: "Entire screen, including URL address"; time stamps in the terminal's
time zone). So screenshots from a terminal browser (container checks, Save
page, after sign-in) bring that browser window to the front and save every
display as it looks, as `<name>.png`. The full page (everything below the
fold too) is saved next to it as `<name>_page.png`. Before a capture:

- Allow **Screen Recording** for the app that starts the dashboard (System
  Settings > Privacy & Security > Screen & System Audio Recording), then
  restart it. Without it only the page is saved and the Browser card says so.
- Set the menu bar clock to **Show date: Always** (System Settings >
  Control Center > Clock Options) and don't auto-hide the menu bar. The
  Browser card warns when the date may be missing.
- Keep this computer on Pacific time (the terminals' time zone).
- Everything on that screen is in the picture, so keep personal windows off
  it. The address bar shows the full address, as the carriers ask.

Public schedule screenshots are taken without a visible browser and stay
page-only.

**Time stamp on evidence:** shipping lines and terminals ask for the date
and time on the proof itself, so every screenshot the desk saves (container
checks, Save page, after sign-in, public schedules) gets a banner across
the top: `Taken 2026-11-14 14:33:04 PST (UTC-08:00)`, then the terminal,
the container when there is one, and the page address (without its query
string). **Copy proof** also lists `Screenshot taken:` for each screenshot.
The banner needs Pillow (in `requirements.txt`); without it the screenshot
is saved unstamped. APM's PDF copy is kept as downloaded, with the time in
its file name.

**Capturing what terminals receive:** when any browser signs in, the desk
reads the APM, ITS and TraPac public receiving schedules into the receiving
board and saves TTI's schedule picture (at most every 30 minutes). Use **Capture receiving now** to refresh.

- **APM** posts a daily PDF grid on its [Empty Receivables page](https://www.apmterminals.com/en/los-angeles/practical-information/empty-receivables-and-open-areas).
  The desk finds that day's PDF, keeps a copy in `screenshots/apm/` as
  evidence, and reads all lines, sizes, days and 1st/2nd shifts from it. The
  values are YES, YES-S (single only), YES-D (dual only) and NO.
- **3rd shift** columns are not used, because the desk tracks 1st and 2nd
  shift.
- **Reefer, open top and flat rack** rows are not used, and the same goes for
  ITS and TraPac.

Other terminals still go on the Receiving screen by hand for now:
- **TTI** posts its empty receiving table (MSC only, OPEN/CLOSED by size
  and shift, about a week ahead) as a picture on its
  [Gate Schedule page](https://www.totalterminals.com/gate-schedule). The
  desk saves it with the other schedules after a sign-in, cropped to the
  pictures (appointment release times, empty receiving and gate weeks), in
  `screenshots/tti/schedule_<date>_<time>.png`. It can't be read into rows
  without text recognition, so enter TTI's rows by hand from the picture.
- YTI's page has no table.
- ETS refuses automated requests to its public page. Sign in to ETSLink
  instead.

For ITS TMS and ETSLink, sign in and open their receiving page, then press
**Save page** to keep it as evidence. To have the desk read those pages into
the receiving board automatically, leave the signed-in browser on that page,
and Claude can build a reader with the MCP server (`inspect_page`).

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
     ETS, TraPac, YTI, ITS, TTI). For APM it also saves and reads the daily PDF. Captures the page without sign-in and fills in
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
     LBCT, when Online), **Save page** (when Online: saves the page you're on
     as a full-page screenshot in `screenshots/<portal>/`, for example ITS
     TMS or ETSLink's empty receiving page), **Open saved** and **Close**.

   The sign-in state is checked every 3 seconds. A browser counts as needing
   sign-in when a password field (or the terminal's known login field) is
   showing, the address looks like a login page, or it is still on the page
   it opened and has never been signed in. The board row for each terminal
   also shows its browser status.

   **Check & screenshot after sign-in** (on by default; the checkbox is in
   the Browsers list and in each terminal's Browser card). When a browser
   goes from signed out to Online:

   1. The page it landed on is saved as
      `screenshots/<portal>/after_sign_in_<date>_<time>.png` (about 5
      seconds after sign-in, or straight away if the check below starts
      sooner).
   2. On terminals with an automatic check (APM and LBCT), the browser then
      looks up empty-return appointments for every container in the check
      list, the same as pressing Check containers. Each container gets a
      screenshot: `<container>_no_return_…`, `<container>_open_…` (slots
      offered) or `<container>_check_…` (unclear, look at it). The findings
      land on the board as usual. With an empty check list, only step 1
      runs.

   Other terminals only do step 1 (plus the extra pages below). This also
   happens when you sign in again after a session expired. Turning it off or
   on applies to browsers already open too.

   To save more pages at the same moment, list them in `.env`, comma
   separated, by portal id in capitals:

   ```
   ITS_TMS_AUTO_CAPTURE_URLS=https://tms.itslb.com/tms2/...,https://...
   ETSLINK_AUTO_CAPTURE_URLS=https://www.etslink.com/...
   ```

   Each one opens in a new tab of the same signed-in browser, is saved as
   `after_sign_in_<n>_<date>_<time>.png`, and the tab is closed. A page that
   asks for a sign-in instead is skipped. The Browser card shows the last
   saved file with **Open**, and **Screenshots folder** opens the folder.
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

## MCP server for Claude Code

`mcp_server.py` lets Claude Code see the desk while helping improve it, for
example to fix selectors when a terminal changes its site. It runs over stdio
(no network port) and is registered in `.mcp.json`. Claude Code asks you to
approve it the first time a session starts in this folder. Check it with
`claude mcp list`.

| Tool | What it does |
|------|--------------|
| `list_browsers` | Open browsers: sign-in state, address, tabs, last message |
| `inspect_page` | The current page of one browser: address, frames, form fields, buttons, links, table previews, visible text, and a screenshot |
| `query_selector` | Test a CSS selector in a browser (all frames) |
| `check_containers` | Run the APM/LBCT check in a browser you've signed in to |
| `screenshot_schedule` | Public schedule screenshot plus receiving rows read from it |
| `classify_text` | Which finding a page with this text would get |
| `read_browser_log` | End of `data/browsers/<terminal>.log` |
| `desk_state` | Saved checks, receiving rows and recent evidence |
| `saved_logins` | Which login systems have a saved login (yes/no only) |

**Limits, by design:**
- **Logins:** it can't read saved logins (only whether one exists) or anything typed in a field. Field lists say only whether a field has a value, and HTML snippets have every `value` removed.
- **Control:** it can't sign in, type, click or navigate. You open browsers from the dashboard and sign in yourself.
- **Containers:** `check_containers` only runs in a browser that is already signed in.

Page screenshots from `inspect_page` go to `screenshots/_inspect/`.

## Files

| File | Purpose |
|------|---------|
| `desk_browser.py` | Shared engine: terminal URLs, page classification, APM and LBCT checks, dashboard file protocol, CLI runner |
| `screenshot.py` | Public empty-schedule screenshots (`--json` output for the dashboard) |
| `schedules.py` | Reads public schedules into receiving-board rows (APM PDF, ITS, TraPac) |
| `APM.py`, `LBCT.py` | CLI wrappers around `desk_browser.run_cli` |
| `mcp_server.py` | MCP server so Claude Code can inspect browsers and the desk (`.mcp.json`) |
| `ssl_lookup.py` | Shipping line name ↔ SCAC code lookup |
| `dashboard/src/model.rs` | Terminals, findings, proof packet and board text |
| `dashboard/src/browser.rs` | Starts and talks to the Python processes |
| `dashboard/src/main.rs` | UI |
| `dashboard/src/store.rs` | Saved desk state |
| `dashboard/src/credentials.rs` | Saved logins (owner-only files) |
| `dashboard/src/upload.rs` | Reads daily empties lists (CSV, Excel, pasted) |

## Tests

```bash
.venv/bin/python desk_browser.py --self-test
.venv/bin/python schedules.py
cd dashboard && cargo test
```
