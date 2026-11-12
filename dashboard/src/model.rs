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
pub const ACCOUNTS: [(&str, &str); 6] = [
    ("termpoint", "TERMPoint"),
    ("lbct", "LBCT portal"),
    ("emodal", "eModal"),
    ("fenix", "Fenix portal"),
    ("voyagertrack", "VoyagerTrack"),
    ("yti", "YTI"),
];

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
            "yti" => "yti",
            _ => "emodal",
        }
    }

    pub fn where_to_look(&self) -> &'static str {
        if self.empty_url.is_some() {
            "Public schedule and appointment portal"
        } else {
            "Appointment portal only — no public empty list"
        }
    }

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
        empty_url: Some("https://www.apmterminals.com/en/los-angeles/empty-container-return"),
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
        appt_url: "https://yti.com/",
        appt_system: "YTI",
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
        empty_url: None,
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
    use super::*;

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
