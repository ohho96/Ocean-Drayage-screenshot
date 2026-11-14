//! What a carrier needs before it accepts a "no empty return" dispute, and
//! how much of it the desk already holds.
//!
//! MSC and ONE (US) both ask for appointment-system screenshots taken
//! - the day before the waiver date, 6:00 to 12:00 (noon), and
//! - on the waiver date, 6:00 to 9:00,
//! at every return location the line uses, showing the container. ONE counts
//! the day before as the previous *working* day; MSC as the calendar day.
//! Times are the terminal's (Pacific) time, which is this computer's clock.
//!
//! The waiver date is the Empties date: the day the box needed a return.

use std::collections::HashMap;

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, Weekday};

use crate::model::{terminal, terminals, EmptyBox, Evidence, Finding, Receiving, Terminal};

/// What a carrier needs to find the charge: per container, kept across days.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Dispute {
    /// Bill of lading number.
    #[serde(default)]
    pub bl: String,
    /// Per diem / detention invoice number (MSC per diem invoices end in "P").
    #[serde(default)]
    pub invoice: String,
    /// Invoice date, YYYY-MM-DD. Disputes are due within 30 days of it.
    #[serde(default)]
    pub invoice_date: String,
}

/// Days after the invoice date to dispute it (MSC and ONE US: 30).
pub const DISPUTE_DAYS: i64 = 30;

impl Dispute {
    pub fn is_empty(&self) -> bool {
        self.bl.trim().is_empty() && self.invoice.trim().is_empty() && self.invoice_date.trim().is_empty()
    }

    /// Last day to submit the dispute, when the invoice date is known.
    pub fn deadline(&self) -> Option<NaiveDate> {
        let date = NaiveDate::parse_from_str(self.invoice_date.trim(), "%Y-%m-%d").ok()?;
        Some(date + Duration::days(DISPUTE_DAYS))
    }

    /// "Dispute by 2026-12-14 (12 days left)", and how urgent: 0 fine, 1 within
    /// a week, 2 due today or late. None without an invoice date.
    pub fn deadline_text(&self, today: NaiveDate) -> Option<(String, u8)> {
        let due = self.deadline()?;
        let left = (due - today).num_days();
        let text = match left {
            l if l < 0 => format!("Dispute deadline {due} passed {} days ago", -l),
            0 => format!("Dispute by {due}: due today"),
            1 => format!("Dispute by {due} (1 day left)"),
            l => format!("Dispute by {due} ({l} days left)"),
        };
        let level = if left <= 0 { 2 } else if left <= 7 { 1 } else { 0 };
        Some((text, level))
    }

    /// A problem worth flagging in what was typed, if any.
    pub fn hint(&self, line: &str) -> Option<&'static str> {
        let date = self.invoice_date.trim();
        if !date.is_empty() && NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
            return Some("Invoice date as YYYY-MM-DD.");
        }
        let invoice = self.invoice.trim();
        if line.eq_ignore_ascii_case("MSC") && !invoice.is_empty() && !invoice.to_uppercase().ends_with('P') {
            return Some("MSC per diem invoice numbers end in P.");
        }
        None
    }

    /// Lines for the copied proof.
    pub fn lines(&self, today: NaiveDate) -> Vec<String> {
        let mut out = Vec::new();
        if !self.bl.trim().is_empty() {
            out.push(format!("B/L: {}", self.bl.trim()));
        }
        if !self.invoice.trim().is_empty() {
            out.push(format!("Invoice: {}", self.invoice.trim()));
        }
        if !self.invoice_date.trim().is_empty() {
            out.push(format!("Invoice date: {}", self.invoice_date.trim()));
        }
        if let Some((text, _)) = self.deadline_text(today) {
            out.push(text);
        }
        out
    }
}

/// One time window a carrier wants a screenshot in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub label: &'static str,
    pub date: NaiveDate,
    pub start_hour: u32,
    pub end_hour: u32,
}

impl Window {
    pub fn contains(&self, at: NaiveDateTime) -> bool {
        at.date() == self.date && at.hour_in(self.start_hour, self.end_hour)
    }

    pub fn describe(&self) -> String {
        format!(
            "{} {} {}–{}",
            self.label,
            self.date.format("%a %Y-%m-%d"),
            clock(self.start_hour),
            clock(self.end_hour)
        )
    }

    pub fn starts(&self) -> NaiveDateTime {
        self.date.and_hms_opt(self.start_hour, 0, 0).expect("valid hour")
    }

    pub fn ends(&self) -> NaiveDateTime {
        self.date.and_hms_opt(self.end_hour, 0, 0).expect("valid hour")
    }
}

trait HourIn {
    fn hour_in(&self, start: u32, end: u32) -> bool;
}

impl HourIn for NaiveDateTime {
    fn hour_in(&self, start: u32, end: u32) -> bool {
        use chrono::Timelike;
        let minutes = self.hour() * 60 + self.minute();
        minutes >= start * 60 && minutes < end * 60
    }
}

fn clock(hour: u32) -> String {
    match hour {
        0 => "12:00 a.m.".into(),
        12 => "12:00 noon".into(),
        h if h < 12 => format!("{h}:00 a.m."),
        h => format!("{}:00 p.m.", h - 12),
    }
}

/// Lines whose "day before" skips the weekend (ONE says "working day prior").
fn uses_working_day(line: &str) -> bool {
    line.eq_ignore_ascii_case("ONE")
}

/// The screenshot windows a line asks for, for one waiver date.
pub fn windows_for(line: &str, waiver: &str) -> Vec<Window> {
    let Ok(day) = NaiveDate::parse_from_str(waiver, "%Y-%m-%d") else {
        return Vec::new();
    };
    let mut before = day - Duration::days(1);
    if uses_working_day(line) {
        while matches!(before.weekday(), Weekday::Sat | Weekday::Sun) {
            before -= Duration::days(1);
        }
    }
    vec![
        Window { label: "Day before", date: before, start_hour: 6, end_hour: 12 },
        Window { label: "Waiver day", date: day, start_hour: 6, end_hour: 9 },
    ]
}

/// When a screenshot was taken: the `_YYYYMMDD_HHMMSS` stamp in the file name
/// the desk gives every screenshot, otherwise the file's modified time.
pub fn shot_time(path: &str) -> Option<NaiveDateTime> {
    let stem = std::path::Path::new(path).file_stem()?.to_string_lossy().to_string();
    let parts: Vec<&str> = stem.split('_').collect();
    for pair in parts.windows(2).rev() {
        let (day, time) = (pair[0], pair[1]);
        if day.len() == 8 && time.len() == 6 && day.chars().chain(time.chars()).all(|c| c.is_ascii_digit()) {
            if let Ok(at) = NaiveDateTime::parse_from_str(&format!("{day}{time}"), "%Y%m%d%H%M%S") {
                return Some(at);
            }
        }
    }
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(chrono::DateTime::<Local>::from(modified).naive_local())
}

/// What the desk holds for one return location in one window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mark {
    /// No screenshot of this container at this terminal in the window.
    Missing,
    /// A screenshot showing no return (no slots, closed, dual only).
    Refused { at: NaiveDateTime, path: String },
    /// A screenshot, but its finding was "could not check": look at it.
    Unclear { at: NaiveDateTime, path: String },
    /// A screenshot where a return slot was offered: no dispute here.
    Open { at: NaiveDateTime, path: String },
}

impl Mark {
    fn rank(&self) -> u8 {
        match self {
            Mark::Missing => 0,
            Mark::Open { .. } => 1,
            Mark::Unclear { .. } => 2,
            Mark::Refused { .. } => 3,
        }
    }

    pub fn is_done(&self) -> bool {
        matches!(self, Mark::Refused { .. })
    }

    pub fn word(&self) -> String {
        match self {
            Mark::Missing => "MISSING".into(),
            Mark::Refused { at, .. } => format!("no return, {}", at.format("%H:%M:%S")),
            Mark::Unclear { at, .. } => format!("unclear, {} (look at it)", at.format("%H:%M:%S")),
            Mark::Open { at, .. } => format!("slot was offered, {}", at.format("%H:%M:%S")),
        }
    }
}

/// Coverage for one container on one waiver date.
pub struct Coverage {
    pub windows: Vec<Window>,
    pub locations: Vec<&'static Terminal>,
    /// marks[location][window]
    pub marks: Vec<Vec<Mark>>,
    /// True when the locations came from the receiving board, not the user.
    pub guessed: bool,
}

impl Coverage {
    pub fn total(&self) -> usize {
        self.windows.len() * self.locations.len()
    }

    pub fn done(&self) -> usize {
        self.marks.iter().flatten().filter(|mark| mark.is_done()).count()
    }

    pub fn complete(&self) -> bool {
        self.total() > 0 && self.done() == self.total()
    }

    /// Any location where a slot was offered and nothing refused in that window.
    pub fn any_open(&self) -> bool {
        self.marks.iter().flatten().any(|mark| matches!(mark, Mark::Open { .. }))
    }

    /// Plain lines for hover text and the copied proof.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.locations.is_empty() {
            out.push("No return locations set for this line. Set them on the Empties screen.".into());
            return out;
        }
        for window in &self.windows {
            out.push(window.describe());
            for (index, terminal) in self.locations.iter().enumerate() {
                let mark = &self.marks[index][self.windows.iter().position(|w| w == window).unwrap_or(0)];
                out.push(format!("  {}: {}", terminal.name, mark.word()));
            }
        }
        out
    }
}

/// Where a line's empties go back: the user's list when set, otherwise every
/// terminal the receiving board has ever listed the line at.
pub fn locations_for(
    line: &str,
    saved: &HashMap<String, Vec<String>>,
    receivings: &[Receiving],
) -> (Vec<&'static Terminal>, bool) {
    if let Some(ids) = saved.get(&line.to_uppercase()) {
        return (ids.iter().filter_map(|id| terminal(id)).collect(), false);
    }
    let found: Vec<&'static Terminal> = terminals()
        .iter()
        .filter(|terminal| {
            receivings
                .iter()
                .any(|row| row.terminal_id == terminal.id && row.line.eq_ignore_ascii_case(line))
        })
        .collect();
    (found, true)
}

pub fn coverage(
    item: &EmptyBox,
    waiver: &str,
    locations: Vec<&'static Terminal>,
    guessed: bool,
    log: &[Evidence],
) -> Coverage {
    let windows = windows_for(&item.line, waiver);
    let marks = locations
        .iter()
        .map(|terminal| {
            windows
                .iter()
                .map(|window| best_mark(&item.container, terminal.id, window, log))
                .collect()
        })
        .collect();
    Coverage { windows, locations, marks, guessed }
}

fn best_mark(container: &str, terminal_id: &str, window: &Window, log: &[Evidence]) -> Mark {
    let mut best = Mark::Missing;
    for entry in log.iter().filter(|e| {
        e.container == container && e.terminal_id == terminal_id && !e.screenshot.trim().is_empty()
    }) {
        let path = entry.screenshot.trim().to_string();
        let Some(at) = shot_time(&path) else { continue };
        if !window.contains(at) {
            continue;
        }
        let mark = if entry.finding.is_refusal() {
            Mark::Refused { at, path }
        } else if entry.finding == Finding::Open {
            Mark::Open { at, path }
        } else {
            Mark::Unclear { at, path }
        };
        if mark.rank() > best.rank() {
            best = mark;
        }
    }
    best
}

/// A reminder about the windows for one waiver date, relative to `now`:
/// open now (with time left), coming up today, or nothing.
pub fn window_notice(windows: &[Window], now: NaiveDateTime) -> Option<(String, bool)> {
    for window in windows {
        if now >= window.starts() && now < window.ends() {
            let left = window.ends() - now;
            let text = format!(
                "{} window is open now, until {} ({}h {:02}m left). Take the screenshots now.",
                window.label,
                clock(window.end_hour),
                left.num_hours(),
                left.num_minutes() % 60
            );
            return Some((text, true));
        }
    }
    for window in windows {
        if now < window.starts() && window.date == now.date() {
            return Some((format!("{} window opens today at {}.", window.label, clock(window.start_hour)), false));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Shift;

    fn at(text: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").unwrap()
    }

    fn evidence(container: &str, terminal_id: &str, finding: Finding, screenshot: &str) -> Evidence {
        Evidence {
            id: 1,
            at: String::new(),
            date: String::new(),
            shift: Shift::First,
            container: container.into(),
            terminal_id: terminal_id.into(),
            terminal_name: String::new(),
            finding,
            note: String::new(),
            screenshot: screenshot.into(),
        }
    }

    #[test]
    fn dispute_deadline_and_hints() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 14).unwrap();
        let mut d = Dispute { invoice_date: "2026-11-01".into(), ..Default::default() };
        assert_eq!(d.deadline().unwrap().to_string(), "2026-12-01");
        let (text, level) = d.deadline_text(today).unwrap();
        assert_eq!((text.as_str(), level), ("Dispute by 2026-12-01 (17 days left)", 0));
        d.invoice_date = "2026-10-20".into();
        assert_eq!(d.deadline_text(today).unwrap().1, 1);
        d.invoice_date = "2026-10-10".into();
        let (text, level) = d.deadline_text(today).unwrap();
        assert!(text.contains("passed 5 days ago") && level == 2, "{text}");
        d.invoice_date = "11/01/2026".into();
        assert_eq!(d.hint("MSC"), Some("Invoice date as YYYY-MM-DD."));
        d.invoice_date.clear();
        d.invoice = "INV123".into();
        assert!(d.hint("MSC").is_some() && d.hint("ONE").is_none());
        d.invoice = "INV123P".into();
        assert!(d.hint("MSC").is_none());
        assert!(Dispute::default().is_empty() && Dispute::default().deadline().is_none());
    }

    #[test]
    fn day_before_is_calendar_for_msc_and_working_day_for_one() {
        // 2026-11-16 is a Monday.
        let msc = windows_for("MSC", "2026-11-16");
        assert_eq!(msc[0].date.to_string(), "2026-11-15");
        assert_eq!((msc[0].start_hour, msc[0].end_hour), (6, 12));
        assert_eq!(msc[1].date.to_string(), "2026-11-16");
        assert_eq!((msc[1].start_hour, msc[1].end_hour), (6, 9));
        let one = windows_for("ONE", "2026-11-16");
        assert_eq!(one[0].date.to_string(), "2026-11-13");
        assert!(windows_for("MSC", "not a date").is_empty());
    }

    #[test]
    fn window_edges() {
        let w = &windows_for("MSC", "2026-11-16")[1];
        assert!(!w.contains(at("2026-11-16 05:59")));
        assert!(w.contains(at("2026-11-16 06:00")));
        assert!(w.contains(at("2026-11-16 08:59")));
        assert!(!w.contains(at("2026-11-16 09:00")));
    }

    #[test]
    fn reads_time_from_desk_file_names() {
        let t = shot_time("/x/screenshots/tti/MSCU1234567_no_return_20261116_071502.png").unwrap();
        assert_eq!(t, at("2026-11-16 07:15") + Duration::seconds(2));
        let page = shot_time("/x/MSCU1234567_no_return_20261116_071502_page.png").unwrap();
        assert_eq!(page, t);
        assert!(shot_time("/no/such/file.png").is_none());
    }

    #[test]
    fn coverage_counts_refusals_in_window_per_location() {
        let item = EmptyBox { container: "MSCU1234567".into(), line: "MSC".into(), ..Default::default() };
        let tti = terminal("tti").unwrap();
        let fenix = terminal("fenix").unwrap();
        let log = vec![
            // TTI: both windows refused.
            evidence("MSCU1234567", "tti", Finding::NoSlots, "/s/MSCU1234567_no_return_20261115_100000.png"),
            evidence("MSCU1234567", "tti", Finding::Closed, "/s/MSCU1234567_no_return_20261116_063000.png"),
            // Fenix: waiver day too late (after 9), day before offered a slot.
            evidence("MSCU1234567", "fenix", Finding::NoSlots, "/s/MSCU1234567_no_return_20261116_093000.png"),
            evidence("MSCU1234567", "fenix", Finding::Open, "/s/MSCU1234567_open_20261115_080000.png"),
            // Another container does not count.
            evidence("ZIMU7654321", "fenix", Finding::NoSlots, "/s/ZIMU7654321_no_return_20261116_070000.png"),
        ];
        let cover = coverage(&item, "2026-11-16", vec![tti, fenix], false, &log);
        assert_eq!(cover.total(), 4);
        assert_eq!(cover.done(), 2);
        assert!(!cover.complete());
        assert!(cover.any_open());
        assert!(matches!(cover.marks[1][1], Mark::Missing));
        let text = cover.lines().join("\n");
        assert!(text.contains("Fenix Marine: MISSING"), "{text}");
        assert!(text.contains("slot was offered"), "{text}");
    }

    #[test]
    fn locations_from_saved_list_or_receiving_board() {
        let mut saved = HashMap::new();
        let rows = vec![Receiving {
            id: 1,
            date: "2026-11-10".into(),
            shift: Shift::First,
            terminal_id: "tti".into(),
            line: "MSC".into(),
            size: crate::model::BoxSize::Forty,
            status: crate::model::ReceiveStatus::NotReceiving,
            updated_at: String::new(),
        }];
        let (found, guessed) = locations_for("MSC", &saved, &rows);
        assert!(guessed);
        assert_eq!(found.iter().map(|t| t.id).collect::<Vec<_>>(), vec!["tti"]);
        saved.insert("MSC".to_string(), vec!["fenix".to_string(), "tti".to_string()]);
        let (found, guessed) = locations_for("msc", &saved, &rows);
        assert!(!guessed);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn notice_while_window_open() {
        let windows = windows_for("MSC", "2026-11-16");
        let (text, open) = window_notice(&windows, at("2026-11-16 07:40")).unwrap();
        assert!(open && text.starts_with("Waiver day window is open now"), "{text}");
        let (text, open) = window_notice(&windows, at("2026-11-15 05:00")).unwrap();
        assert!(!open && text.contains("opens today"), "{text}");
        assert!(window_notice(&windows, at("2026-11-16 10:00")).is_none());
    }
}
