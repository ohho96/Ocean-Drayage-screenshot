use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Port {
    LosAngeles,
    LongBeach,
}

impl Port {
    pub fn label(self) -> &'static str {
        match self {
            Self::LosAngeles => "Port of Los Angeles",
            Self::LongBeach => "Port of Long Beach",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shift {
    #[serde(rename = "1st")]
    #[default]
    First,
    #[serde(rename = "2nd")]
    Second,
}

impl Shift {
    pub fn label(self) -> &'static str {
        match self {
            Self::First => "1st shift",
            Self::Second => "2nd shift",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::First => "1st",
            Self::Second => "2nd",
        }
    }
}

/// What the terminal showed for this empty.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Finding {
    #[default]
    NotChecked,
    Open,
    DualOnly,
    Closed,
    NoSlots,
    CouldNotCheck,
}

impl Finding {
    pub const ALL: [Self; 6] = [
        Self::NotChecked,
        Self::Closed,
        Self::NoSlots,
        Self::DualOnly,
        Self::Open,
        Self::CouldNotCheck,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::NotChecked => "Not checked",
            Self::Open => "Open",
            Self::DualOnly => "Dual only",
            Self::Closed => "Closed — no empty return",
            Self::NoSlots => "No appointment slots",
            Self::CouldNotCheck => "Could not check",
        }
    }

    /// A finding that supports "this empty could not be returned as a straight drop."
    pub fn is_refusal(self) -> bool {
        matches!(self, Self::Closed | Self::NoSlots | Self::DualOnly)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoxSize {
    Twenty,
    Forty,
    FortyHc,
    FortyFive,
}

impl BoxSize {
    pub const ALL: [Self; 4] = [Self::Twenty, Self::Forty, Self::FortyHc, Self::FortyFive];

    pub fn label(self) -> &'static str {
        match self {
            Self::Twenty => "20'",
            Self::Forty => "40'",
            Self::FortyHc => "40' HC",
            Self::FortyFive => "45'",
        }
    }
}

/// Whether a terminal is taking that empty on the selected shift.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveStatus {
    Receiving,
    DualOnly,
    NotReceiving,
}

impl ReceiveStatus {
    pub const ALL: [Self; 3] = [Self::Receiving, Self::DualOnly, Self::NotReceiving];

    pub fn label(self) -> &'static str {
        match self {
            Self::Receiving => "Receiving",
            Self::DualOnly => "Dual only",
            Self::NotReceiving => "Not receiving",
        }
    }
}

pub const LINES: &[&str] = &[
    "Maersk",
    "MSC",
    "CMA CGM",
    "COSCO",
    "Evergreen",
    "Hapag-Lloyd",
    "ONE",
    "Yang Ming",
    "HMM",
    "ZIM",
    "Wan Hai",
    "PIL",
    "SM Line",
    "Matson",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Check {
    pub finding: Finding,
    pub note: String,
    pub screenshot: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    pub id: u64,
    pub at: String,
    pub date: String,
    pub shift: Shift,
    pub container: String,
    pub terminal_id: String,
    pub terminal_name: String,
    pub finding: Finding,
    pub note: String,
    pub screenshot: String,
}

pub struct Terminal {
    pub id: &'static str,
    pub name: &'static str,
    pub port: Port,
    pub pier: &'static str,
    pub empty_url: Option<&'static str>,
    pub appt_url: &'static str,
    pub appt_system: &'static str,
    /// Playwright can check containers after login.
    pub auto_check: bool,
}

/// Login systems. Several terminals share one (eModal). Keep in step with
/// ACCOUNT_OF in desk_browser.py.
pub const ACCOUNTS: [(&str, &str); 9] = [
    ("termpoint", "TERMPoint"),
    ("lbct", "LBCT portal"),
    ("emodal", "eModal"),
    ("fenix", "Fenix portal"),
    ("voyagertrack", "VoyagerTrack"),
    ("yti_gatelink", "YTI Gate Link"),
    ("yti_lynx", "YTI LYNX"),
    ("its_tms", "ITS TMS"),
    ("etslink", "ETSLink"),
];

/// Browser id of the eModal window. eModal is one website for several
/// terminals (picked in its terminal list), so they all share one browser.
pub const EMODAL: &str = "emodal";

/// One sign-in system a terminal uses. Each website gets one browser; `id` is
/// the browser id (the terminal id when the terminal has a single portal of
/// its own, `EMODAL` for every terminal that books on eModal).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Portal {
    pub id: &'static str,
    pub terminal_id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    pub account: &'static str,
}

/// Terminals with more than one portal. Keep in step with TERMINALS in desk_browser.py.
const EXTRA_PORTALS: &[Portal] = &[
    Portal {
        id: "yti_gatelink",
        terminal_id: "yti",
        name: "Gate Link",
        url: "https://cap.yti.com/logon",
        account: "yti_gatelink",
    },
    Portal {
        id: "yti_lynx",
        terminal_id: "yti",
        name: "LYNX",
        url: "https://lynx.yti.com/",
        account: "yti_lynx",
    },
    Portal {
        id: "its_tms",
        terminal_id: "its",
        name: "ITS TMS",
        url: "https://tms.itslb.com/tms2/Account/Login",
        account: "its_tms",
    },
    Portal {
        id: EMODAL,
        terminal_id: "its",
        name: "eModal",
        url: "https://www.emodal.com/",
        account: "emodal",
    },
    Portal {
        id: "etslink",
        terminal_id: "ets",
        name: "ETSLink",
        url: "https://www.etslink.com/",
        account: "etslink",
    },
    Portal {
        id: EMODAL,
        terminal_id: "ets",
        name: "eModal",
        url: "https://www.emodal.com/",
        account: "emodal",
    },
];

/// Terminals that book appointments on eModal, in board order.
pub fn emodal_terminals() -> Vec<&'static Terminal> {
    terminals()
        .iter()
        .filter(|terminal| terminal.portals().iter().any(|p| p.id == EMODAL))
        .collect()
}

/// Name for a browser window: the terminal (and portal), or for the shared
/// eModal window every terminal it serves.
pub fn browser_name(id: &str) -> String {
    if id == EMODAL {
        let names: Vec<&str> = emodal_terminals().iter().map(|t| t.name).collect();
        return format!("eModal ({})", names.join(", "));
    }
    match portal(id) {
        Some((t, p)) if t.portals().len() > 1 => format!("{} — {}", t.name, p.name),
        Some((t, _)) => t.name.to_string(),
        None => "Terminal".to_string(),
    }
}

/// The terminal and portal behind a browser id. For the shared eModal
/// browser this is the first eModal terminal; use `browser_name` for labels.
pub fn portal(id: &str) -> Option<(&'static Terminal, Portal)> {
    terminals()
        .iter()
        .flat_map(|terminal| terminal.portals().into_iter().map(move |p| (terminal, p)))
        .find(|(_, p)| p.id == id)
}

pub fn account_name(account: &str) -> &'static str {
    ACCOUNTS
        .iter()
        .find(|(id, _)| *id == account)
        .map(|(_, name)| *name)
        .unwrap_or("Login")
}

impl Terminal {
    /// Which saved login this terminal's sign-in page uses.
    pub fn account(&self) -> &'static str {
        match self.id {
            "apm" => "termpoint",
            "lbct" => "lbct",
            "fenix" => "fenix",
            "wbct" => "voyagertrack",
            "yti" => "yti_gatelink",
            _ => "emodal",
        }
    }

    /// Sign-in systems for this terminal, appointment system first.
    pub fn portals(&self) -> Vec<Portal> {
        let extra: Vec<Portal> = EXTRA_PORTALS
            .iter()
            .copied()
            .filter(|p| p.terminal_id == self.id)
            .collect();
        if !extra.is_empty() {
            return extra;
        }
        let account = self.account();
        vec![Portal {
            id: if account == "emodal" { EMODAL } else { self.id },
            terminal_id: self.id,
            name: self.appt_system,
            url: self.appt_url,
            account,
        }]
    }

    pub fn where_to_look(&self) -> &'static str {
        if self.empty_url.is_some() {
            "Public schedule and appointment portal"
        } else {
            "Appointment portal only — no public empty list"
        }
    }

}

/// One empty container from a daily upload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmptyBox {
    pub container: String,
    /// Shipping line, as a name from LINES when recognised.
    pub line: String,
    /// "20", "40" or "45"; empty when unknown.
    pub size: String,
    /// "DV" (dry), "HC", "RF", "OT", "FR", "TK"; empty when unknown.
    pub kind: String,
}

impl EmptyBox {
    /// The receiving-board size this box returns as, if it is a standard box.
    pub fn board_size(&self) -> Option<BoxSize> {
        match (self.size.as_str(), self.kind.as_str()) {
            ("20", "DV" | "") => Some(BoxSize::Twenty),
            ("40", "DV" | "") => Some(BoxSize::Forty),
            ("40", "HC") => Some(BoxSize::FortyHc),
            ("45", "HC" | "DV" | "") => Some(BoxSize::FortyFive),
            _ => None,
        }
    }

    pub fn size_label(&self) -> String {
        match (self.size.is_empty(), self.kind.is_empty()) {
            (true, true) => "?".into(),
            (false, true) => format!("{}'", self.size),
            (true, false) => self.kind.clone(),
            (false, false) => format!("{}' {}", self.size, self.kind),
        }
    }
}

/// One daily list of empties the user uploaded.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Upload {
    pub id: u64,
    /// The day these empties need a return appointment.
    pub date: String,
    pub uploaded_at: String,
    /// File name, or "pasted".
    pub source: String,
    pub boxes: Vec<EmptyBox>,
}

/// Every box for a date, last upload winning for a repeated container.
pub fn boxes_for_day(uploads: &[Upload], date: &str) -> Vec<EmptyBox> {
    let mut out: Vec<EmptyBox> = Vec::new();
    for upload in uploads.iter().filter(|upload| upload.date == date) {
        for item in &upload.boxes {
            if let Some(existing) = out.iter_mut().find(|b| b.container == item.container) {
                *existing = item.clone();
            } else {
                out.push(item.clone());
            }
        }
    }
    out
}

/// Source name for containers typed in on the Empties screen.
pub const MANUAL_SOURCE: &str = "added by hand";

/// Container sizes the desk tracks, in feet.
pub const BOX_SIZES: [&str; 3] = ["20", "40", "45"];

/// Container types: code and a plain name.
pub const BOX_KINDS: [(&str, &str); 6] = [
    ("DV", "Dry"),
    ("HC", "High cube"),
    ("RF", "Reefer"),
    ("OT", "Open top"),
    ("FR", "Flat rack"),
    ("TK", "Tank"),
];

/// Add a box to a date's list, or update it everywhere it already appears
/// (so the "last upload wins" view stays consistent). New boxes go into the
/// date's hand-entered list, which is created on first use. Returns true when
/// the container was new for that date.
pub fn upsert_box(
    uploads: &mut Vec<Upload>,
    date: &str,
    item: EmptyBox,
    next_id: &mut u64,
    now: &str,
) -> bool {
    let mut found = false;
    for upload in uploads.iter_mut().filter(|upload| upload.date == date) {
        for existing in upload.boxes.iter_mut().filter(|b| b.container == item.container) {
            *existing = item.clone();
            found = true;
        }
    }
    if found {
        return false;
    }
    if let Some(manual) = uploads
        .iter_mut()
        .find(|upload| upload.date == date && upload.source == MANUAL_SOURCE)
    {
        manual.boxes.push(item);
        manual.uploaded_at = now.to_string();
    } else {
        uploads.push(Upload {
            id: *next_id,
            date: date.to_string(),
            uploaded_at: now.to_string(),
            source: MANUAL_SOURCE.to_string(),
            boxes: vec![item],
        });
        *next_id += 1;
    }
    true
}

/// Remove a container from every list for a date, dropping lists left empty.
/// Returns how many entries were removed.
pub fn remove_box(uploads: &mut Vec<Upload>, date: &str, container: &str) -> usize {
    let mut removed = 0;
    for upload in uploads.iter_mut().filter(|upload| upload.date == date) {
        let before = upload.boxes.len();
        upload.boxes.retain(|b| b.container != container);
        removed += before - upload.boxes.len();
    }
    uploads.retain(|upload| upload.date != date || !upload.boxes.is_empty());
    removed
}

/// Where one box can go back on a date and shift, from the receiving board.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReturnOptions {
    pub single: Vec<&'static str>,
    pub dual: Vec<&'static str>,
    pub closed: Vec<&'static str>,
    pub unknown: Vec<&'static str>,
}

pub fn return_options(item: &EmptyBox, date: &str, shift: Shift, rows: &[Receiving]) -> ReturnOptions {
    let mut options = ReturnOptions::default();
    let size = item.board_size();
    for terminal in terminals() {
        let found = size.and_then(|size| {
            rows.iter().find(|row| {
                row.date == date
                    && row.shift == shift
                    && row.terminal_id == terminal.id
                    && row.size == size
                    && row.line.eq_ignore_ascii_case(&item.line)
            })
        });
        match found.map(|row| row.status) {
            Some(ReceiveStatus::Receiving) => options.single.push(terminal.name),
            Some(ReceiveStatus::DualOnly) => options.dual.push(terminal.name),
            Some(ReceiveStatus::NotReceiving) => options.closed.push(terminal.name),
            None => options.unknown.push(terminal.name),
        }
    }
    options
}

/// Copyable plan: where each uploaded empty can return.
pub fn return_plan(date: &str, shift: Shift, boxes: &[EmptyBox], rows: &[Receiving]) -> String {
    let mut lines = vec![
        "Empty return plan".to_string(),
        format!("Date: {date}"),
        format!("Shift: {}", shift.label()),
        String::new(),
    ];
    for item in boxes {
        let options = return_options(item, date, shift, rows);
        let where_to = if !options.single.is_empty() {
            format!("Single: {}", options.single.join(", "))
        } else if !options.dual.is_empty() {
            format!("Dual only: {}", options.dual.join(", "))
        } else if item.board_size().is_none() {
            "Special equipment, check by hand".to_string()
        } else if options.closed.len() == terminals().len() {
            "No terminal receiving".to_string()
        } else {
            "No terminal known to receive (check)".to_string()
        };
        lines.push(format!(
            "{}  {}  {}  {}",
            item.container,
            item.line,
            item.size_label(),
            where_to
        ));
    }
    lines.join("\n")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Receiving {
    pub id: u64,
    pub date: String,
    pub shift: Shift,
    pub terminal_id: String,
    pub line: String,
    pub size: BoxSize,
    pub status: ReceiveStatus,
    pub updated_at: String,
}

pub fn terminals() -> &'static [Terminal] {
    &TERMINALS
}

pub fn terminal(id: &str) -> Option<&'static Terminal> {
    TERMINALS.iter().find(|terminal| terminal.id == id)
}

pub fn normalize_container(container: &str) -> String {
    container.trim().to_uppercase()
}

pub fn check_key(date: &str, shift: Shift, container: &str, terminal_id: &str) -> String {
    format!(
        "{date}|{}|{}|{terminal_id}",
        shift.key(),
        normalize_container(container)
    )
}

/// When a screenshot was saved, from the file itself, in local time with the
/// UTC offset (the same time is stamped on the image). `None` if the file is gone.
pub fn screenshot_taken(path: &str) -> Option<String> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(format_taken(chrono::DateTime::<chrono::Local>::from(modified)))
}

fn format_taken<Tz: chrono::TimeZone>(at: chrono::DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    at.format("%Y-%m-%d %H:%M:%S (UTC%:z)").to_string()
}

pub fn proof_packet(
    container: &str,
    date: &str,
    shift: Shift,
    checked_at: &str,
    checks: &HashMap<String, Check>,
) -> String {
    let container = normalize_container(container);
    let mut lines = vec![
        "Empty return check".to_string(),
        format!("Container: {container}"),
        format!("Date: {date}"),
        format!("Shift: {}", shift.label()),
        format!("Copied: {checked_at}"),
        String::new(),
        "Terminals with no straight empty return:".to_string(),
    ];

    let mut refusals = 0;
    for terminal in terminals() {
        let Some(check) = checks.get(&check_key(date, shift, &container, terminal.id)) else {
            continue;
        };
        if !check.finding.is_refusal() {
            continue;
        }
        refusals += 1;
        lines.push(String::new());
        lines.push(format!(
            "{} ({}, {})",
            terminal.name,
            terminal.pier,
            terminal.port.label()
        ));
        lines.push(format!("Finding: {}", check.finding.label()));
        lines.push(format!("Recorded: {}", check.updated_at));
        if !check.note.trim().is_empty() {
            lines.push(format!("Note: {}", check.note.trim()));
        }
        if !check.screenshot.trim().is_empty() {
            lines.push(format!("Screenshot: {}", check.screenshot.trim()));
            if let Some(taken) = screenshot_taken(check.screenshot.trim()) {
                lines.push(format!("Screenshot taken: {taken}"));
            }
        }
        if let Some(url) = terminal.empty_url {
            lines.push(format!("Schedule: {url}"));
        }
        lines.push(format!(
            "Appointments ({}): {}",
            terminal.appt_system, terminal.appt_url
        ));
    }

    if refusals == 0 {
        lines.push(String::new());
        lines.push(
            "No terminal is recorded as refusing this empty for this date and shift.".to_string(),
        );
    }

    lines.join("\n")
}

pub fn full_board(
    container: &str,
    date: &str,
    shift: Shift,
    checks: &HashMap<String, Check>,
) -> String {
    let container = normalize_container(container);
    let shown = if container.is_empty() {
        "—"
    } else {
        container.as_str()
    };
    let mut lines = vec![
        "Empty return board".to_string(),
        format!("Container: {shown}"),
        format!("Date: {date}"),
        format!("Shift: {}", shift.label()),
        String::new(),
    ];

    for port in [Port::LosAngeles, Port::LongBeach] {
        lines.push(port.label().to_string());
        for terminal in terminals().iter().filter(|terminal| terminal.port == port) {
            let finding = checks
                .get(&check_key(date, shift, &container, terminal.id))
                .map(|check| check.finding)
                .unwrap_or(Finding::NotChecked);
            lines.push(format!(
                "  {} ({}): {}",
                terminal.name,
                terminal.pier,
                finding.label()
            ));
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

pub fn receiving_board(date: &str, shift: Shift, rows: &[Receiving]) -> String {
    let mut lines = vec![
        "Empty receiving".to_string(),
        format!("Date: {date}"),
        format!("Shift: {}", shift.label()),
        String::new(),
    ];

    for port in [Port::LosAngeles, Port::LongBeach] {
        lines.push(port.label().to_string());
        for terminal in terminals().iter().filter(|terminal| terminal.port == port) {
            lines.push(format!("  {} ({})", terminal.name, terminal.pier));
            let mut found = false;
            for row in rows.iter().filter(|row| {
                row.date == date && row.shift == shift && row.terminal_id == terminal.id
            }) {
                found = true;
                lines.push(format!(
                    "    {} {}: {}",
                    row.line,
                    row.size.label(),
                    row.status.label()
                ));
            }
            if !found {
                lines.push("    Nothing recorded".to_string());
            }
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

const TERMINALS: [Terminal; 11] = [
    Terminal {
        id: "apm",
        name: "APM Terminals",
        port: Port::LosAngeles,
        pier: "Pier 400",
        empty_url: Some("https://www.apmterminals.com/en/los-angeles/practical-information/empty-receivables-and-open-areas"),
        appt_url: "https://termpoint.apmterminals.com/",
        appt_system: "TERMPoint",
        auto_check: true,
    },
    Terminal {
        id: "fenix",
        name: "Fenix Marine",
        port: Port::LosAngeles,
        pier: "Pier 300",
        empty_url: None,
        appt_url: "https://portal.fenixmarineservices.com/apptmt-app/home",
        appt_system: "Fenix portal",
        auto_check: false,
    },
    Terminal {
        id: "wbct",
        name: "West Basin",
        port: Port::LosAngeles,
        pier: "WBCT",
        empty_url: None,
        appt_url: "https://voyagertrack.portsamerica.com/",
        appt_system: "VoyagerTrack",
        auto_check: false,
    },
    Terminal {
        id: "ets",
        name: "Everport",
        port: Port::LosAngeles,
        pier: "ETS",
        empty_url: Some("https://www.etslink.com/"),
        appt_url: "https://www.emodal.com/",
        appt_system: "eModal",
        auto_check: false,
    },
    Terminal {
        id: "trapac",
        name: "TraPac",
        port: Port::LosAngeles,
        pier: "Los Angeles",
        empty_url: Some("https://losangeles.trapac.com/empty-returns/"),
        appt_url: "https://www.emodal.com/",
        appt_system: "eModal",
        auto_check: false,
    },
    Terminal {
        id: "yti",
        name: "Yusen",
        port: Port::LosAngeles,
        pier: "Berths 212–223",
        empty_url: Some("https://yti.com/terminal-schedules/empty-receiving-schedule/"),
        appt_url: "https://cap.yti.com/logon",
        appt_system: "Gate Link / LYNX",
        auto_check: false,
    },
    Terminal {
        id: "lbct",
        name: "LBCT",
        port: Port::LongBeach,
        pier: "Pier E",
        empty_url: None,
        appt_url: "https://portal.lbct.com/LoginWithUrl",
        appt_system: "LBCT portal",
        auto_check: true,
    },
    Terminal {
        id: "tti",
        name: "TTI",
        port: Port::LongBeach,
        pier: "Pier T",
        empty_url: Some("https://www.totalterminals.com/gate-schedule"),
        appt_url: "https://www.emodal.com/",
        appt_system: "eModal",
        auto_check: false,
    },
    Terminal {
        id: "its",
        name: "ITS",
        port: Port::LongBeach,
        pier: "Pier G",
        empty_url: Some("https://www.itslb.com/empty-receiving-updates/"),
        appt_url: "https://www.emodal.com/",
        appt_system: "eModal",
        auto_check: false,
    },
    Terminal {
        id: "pct",
        name: "PCT",
        port: Port::LongBeach,
        pier: "Pier J",
        empty_url: None,
        appt_url: "https://www.emodal.com/",
        appt_system: "eModal",
        auto_check: false,
    },
    Terminal {
        id: "piera",
        name: "Pier A",
        port: Port::LongBeach,
        pier: "SSA Pier A",
        empty_url: None,
        appt_url: "https://www.emodal.com/",
        appt_system: "eModal",
        auto_check: false,
    },
];

#[cfg(test)]
mod tests {

    #[test]
    fn screenshot_time_shows_offset_and_missing_file_is_none() {
        use chrono::TimeZone;
        let zone = chrono::FixedOffset::west_opt(8 * 3600).unwrap();
        let at = zone.with_ymd_and_hms(2026, 11, 14, 14, 29, 24).unwrap();
        assert_eq!(super::format_taken(at), "2026-11-14 14:29:24 (UTC-08:00)");
        assert!(super::screenshot_taken("/no/such/file.png").is_none());
    }

    fn empty(container: &str, line: &str, size: &str, kind: &str) -> EmptyBox {
        EmptyBox {
            container: container.into(),
            line: line.into(),
            size: size.into(),
            kind: kind.into(),
        }
    }

    #[test]
    fn upsert_adds_to_hand_list_then_updates_everywhere() {
        let mut uploads = vec![Upload {
            id: 1,
            date: "2026-11-13".into(),
            uploaded_at: "t".into(),
            source: "list.csv".into(),
            boxes: vec![empty("MSCU1234567", "MSC", "40", "HC")],
        }];
        let mut next = 2;
        assert!(upsert_box(&mut uploads, "2026-11-13", empty("ZIMU7654321", "ZIM", "20", "DV"), &mut next, "t2"));
        assert!(upsert_box(&mut uploads, "2026-11-13", empty("ONEU1111111", "ONE", "45", ""), &mut next, "t3"));
        assert_eq!(uploads.len(), 2, "one hand-entered list per date");
        assert_eq!(uploads[1].source, MANUAL_SOURCE);
        assert_eq!(next, 3);

        // Editing a box from a file updates it in place, no duplicate.
        assert!(!upsert_box(&mut uploads, "2026-11-13", empty("MSCU1234567", "MSC", "20", "DV"), &mut next, "t4"));
        let day = boxes_for_day(&uploads, "2026-11-13");
        assert_eq!(day.len(), 3);
        assert_eq!(day[0].size, "20");
    }

    #[test]
    fn remove_drops_box_and_empty_lists() {
        let mut uploads = Vec::new();
        let mut next = 1;
        upsert_box(&mut uploads, "2026-11-13", empty("ZIMU7654321", "ZIM", "20", "DV"), &mut next, "t");
        upsert_box(&mut uploads, "2026-11-14", empty("ZIMU7654321", "ZIM", "20", "DV"), &mut next, "t");
        assert_eq!(remove_box(&mut uploads, "2026-11-13", "ZIMU7654321"), 1);
        assert_eq!(uploads.len(), 1, "other dates are untouched");
        assert_eq!(uploads[0].date, "2026-11-14");
    }

    use super::*;

    #[test]
    fn return_options_split_single_and_dual() {
        let item = EmptyBox {
            container: "YMLU1234567".into(),
            line: "Yang Ming".into(),
            size: "40".into(),
            kind: "HC".into(),
        };
        let row = |terminal: &str, status| Receiving {
            id: 1,
            date: "2026-11-12".into(),
            shift: Shift::First,
            terminal_id: terminal.into(),
            line: "yang ming".into(),
            size: BoxSize::FortyHc,
            status,
            updated_at: String::new(),
        };
        let rows = vec![
            row("its", ReceiveStatus::Receiving),
            row("trapac", ReceiveStatus::DualOnly),
            row("yti", ReceiveStatus::NotReceiving),
        ];
        let options = return_options(&item, "2026-11-12", Shift::First, &rows);
        assert_eq!(options.single, vec!["ITS"]);
        assert_eq!(options.dual.len(), 1);
        assert_eq!(options.closed.len(), 1);
        assert_eq!(options.unknown.len(), terminals().len() - 3);
        assert!(return_options(&item, "2026-11-12", Shift::Second, &rows).single.is_empty());
        let plan = return_plan("2026-11-12", Shift::First, &[item], &rows);
        assert!(plan.contains("YMLU1234567  Yang Ming  40' HC  Single: ITS"));
    }

    #[test]
    fn later_upload_wins_for_same_container() {
        let a = EmptyBox { container: "A".into(), line: "ONE".into(), size: "20".into(), kind: "DV".into() };
        let mut b = a.clone();
        b.size = "40".into();
        let uploads = vec![
            Upload { id: 1, date: "d".into(), uploaded_at: String::new(), source: String::new(), boxes: vec![a] },
            Upload { id: 2, date: "d".into(), uploaded_at: String::new(), source: String::new(), boxes: vec![b] },
            Upload { id: 3, date: "other".into(), uploaded_at: String::new(), source: String::new(), boxes: vec![] },
        ];
        let day = boxes_for_day(&uploads, "d");
        assert_eq!(day.len(), 1);
        assert_eq!(day[0].size, "40");
    }

    #[test]
    fn every_terminal_has_a_known_account() {
        for terminal in terminals() {
            assert!(
                ACCOUNTS.iter().any(|(id, _)| *id == terminal.account()),
                "{}",
                terminal.id
            );
        }
        assert_eq!(terminal("its").unwrap().account(), "emodal");
        for terminal in terminals() {
            for p in terminal.portals() {
                assert!(ACCOUNTS.iter().any(|(id, _)| *id == p.account), "{}", p.id);
                if p.id != EMODAL {
                    assert_eq!(portal(p.id).unwrap().0.id, terminal.id);
                }
            }
        }
        let yti: Vec<&str> = terminal("yti").unwrap().portals().iter().map(|p| p.name).collect();
        assert_eq!(yti, vec!["Gate Link", "LYNX"]);
        let its: Vec<&str> = terminal("its").unwrap().portals().iter().map(|p| p.id).collect();
        assert_eq!(its, vec!["its_tms", EMODAL]);
        // One eModal browser for every terminal that books there.
        let shared: Vec<&str> = emodal_terminals().iter().map(|t| t.id).collect();
        for id in ["ets", "its", "pct", "piera", "tti", "trapac"] {
            assert!(shared.contains(&id), "{id} should use the eModal browser");
        }
        for id in ["apm", "lbct", "fenix", "wbct", "yti"] {
            assert!(!shared.contains(&id), "{id} has its own site");
        }
        assert!(browser_name(EMODAL).starts_with("eModal (") && browser_name(EMODAL).contains("TTI"));
        assert_eq!(portal("etslink").unwrap().0.id, "ets");
        assert_eq!(portal("apm").unwrap().1.name, "TERMPoint");
        assert_eq!(account_name("termpoint"), "TERMPoint");
    }

    #[test]
    fn packet_lists_only_refusals() {
        let mut checks = HashMap::new();
        checks.insert(
            check_key("2026-11-12", Shift::First, "msmu1", "its"),
            Check {
                finding: Finding::Closed,
                note: "HMM 40 closed".into(),
                screenshot: "its.png".into(),
                updated_at: "2026-11-12 08:00".into(),
            },
        );
        checks.insert(
            check_key("2026-11-12", Shift::First, "MSMU1", "apm"),
            Check {
                finding: Finding::Open,
                note: String::new(),
                screenshot: String::new(),
                updated_at: "2026-11-12 08:05".into(),
            },
        );

        let packet = proof_packet(
            "msmu1",
            "2026-11-12",
            Shift::First,
            "2026-11-12 09:00",
            &checks,
        );
        assert!(packet.contains("ITS"));
        assert!(packet.contains("HMM 40 closed"));
        assert!(packet.contains("its.png"));
        assert!(!packet.contains("APM Terminals"));
    }

    #[test]
    fn receiving_board_lists_each_terminal() {
        let rows = [Receiving {
            id: 1,
            date: "2026-11-12".into(),
            shift: Shift::First,
            terminal_id: "its".into(),
            line: "HMM".into(),
            size: BoxSize::Forty,
            status: ReceiveStatus::Receiving,
            updated_at: "2026-11-12 08:00".into(),
        }];
        let board = receiving_board("2026-11-12", Shift::First, &rows);
        assert!(board.contains("ITS"));
        assert!(board.contains("HMM 40': Receiving"));
        assert!(board.contains("APM Terminals"));
        assert!(board.contains("Nothing recorded"));
    }
}
