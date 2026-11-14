mod browser;
mod credentials;
mod empties_ui;
mod model;
mod proof;
mod snapshot;
mod store;
mod upload;

use std::collections::HashMap;

use browser::{now_secs, repo_root, BrowserManager, BrowserState, ImportBatch, ScheduleShot};
use eframe::egui::{
    self, Align, Button, Color32, ComboBox, CornerRadius, CursorIcon, Frame, Layout, Margin,
    RichText, ScrollArea, Sense, Stroke, TextEdit, Vec2,
};
use credentials::{credentials_dir, Login};
use model::{
    boxes_for_day, return_options, return_plan, Upload,
    account_name, browser_name, check_key, portal, full_board, normalize_container, proof_packet, receiving_board, terminal, terminals,
    BoxSize, Check, Evidence, Finding, Port, ReceiveStatus, Receiving, Shift, Terminal, ACCOUNTS, EMODAL, LINES,
};
use store::Saved;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([980.0, 640.0])
            .with_title("Empty Return Desk"),
        ..Default::default()
    };

    eframe::run_native(
        "Empty Return Desk",
        options,
        Box::new(|cc| {
            apply_style(&cc.egui_ctx);
            Ok(Box::new(Desk::new()))
        }),
    )
}

fn apply_style(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        use egui::{FontFamily, FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
            (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        ]
        .into();
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(10.0, 4.0);
        style.spacing.interact_size = Vec2::new(28.0, 26.0);
        style.spacing.window_margin = Margin::same(12);

        let visuals = &mut style.visuals;
        visuals.window_fill = BG;
        visuals.panel_fill = PANEL;
        visuals.extreme_bg_color = BG;
        visuals.faint_bg_color = SURFACE;
        visuals.code_bg_color = BG;
        visuals.override_text_color = Some(TEXT);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        for (widget, fill) in [
            (&mut visuals.widgets.inactive, SURFACE_HI),
            (&mut visuals.widgets.hovered, Color32::from_rgb(52, 58, 70)),
            (&mut visuals.widgets.active, Color32::from_rgb(62, 70, 84)),
            (&mut visuals.widgets.open, SURFACE_HI),
        ] {
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
            widget.corner_radius = CornerRadius::same(6);
        }
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(78, 86, 100));
        visuals.selection.bg_fill = Color32::from_rgb(72, 90, 118);
        visuals.selection.stroke = Stroke::new(1.0, ACCENT);
        visuals.hyperlink_color = ACCENT;
        visuals.window_corner_radius = CornerRadius::same(10);
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Empties,
    Board,
    Receiving,
    Evidence,
    Logins,
}

/// Edit fields for one login system on the Logins screen. The saved password
/// is never loaded into the form; a blank password keeps the saved one.
#[derive(Default)]
struct LoginDraft {
    user_id: String,
    password: String,
    show: bool,
    confirm_delete: bool,
    /// Saved password while the user has it shown, and when to hide it again.
    revealed: Option<(String, f64)>,
}

/// How long a shown password stays visible, and a copied one stays on the clipboard.
const SECRET_SECONDS: f64 = 30.0;

/// What the desk shows about a saved login, without the password.
struct SavedLogin {
    user_id: String,
    updated_at: String,
}

struct Desk {
    container: String,
    date: String,
    shift: Shift,
    screen: Screen,
    selected: String,
    problems_only: bool,
    this_container_only: bool,
    draft_finding: Finding,
    draft_note: String,
    draft_shot: String,
    draft_scope: String,
    checks: HashMap<String, Check>,
    log: Vec<Evidence>,
    next_id: u64,
    status: String,
    path: std::path::PathBuf,
    containers_text: String,
    receivings: Vec<Receiving>,
    draft_line: String,
    draft_size: BoxSize,
    draft_receive: ReceiveStatus,
    browser: BrowserManager,
    schedule_shot: Option<ScheduleShot>,
    /// Schedules waiting to be captured, one at a time: (terminal, automatic).
    schedule_queue: std::collections::VecDeque<(String, bool)>,
    shot_is_auto: bool,
    /// When each terminal's receiving schedule was last captured.
    last_capture: HashMap<String, (f64, String)>,
    uploads: Vec<Upload>,
    paste_text: String,
    upload_warnings: Vec<String>,
    login_dir: std::path::PathBuf,
    saved_logins: HashMap<String, SavedLogin>,
    login_drafts: HashMap<String, LoginDraft>,
    /// A password copied to the clipboard and when to clear it.
    clipboard_secret: Option<(String, f64)>,
    /// Developer aid: render one screen to a file and quit (DESK_SNAPSHOT).
    snapshot: Option<snapshot::Snapshot>,
    /// Add/edit/filter state for the Empties container list.
    box_form: empties_ui::BoxForm,
    /// Save each terminal's page automatically right after sign-in.
    auto_capture: bool,
    /// Return locations per line (see store::Saved::return_locations).
    return_locations: HashMap<String, Vec<String>>,
    /// Dispute details per container (see store::Saved::disputes).
    disputes: HashMap<String, proof::Dispute>,
}

impl Desk {
    fn new() -> Self {
        let mut desk = Self::build();
        desk.browser.auto_capture = desk.auto_capture;
        desk.refresh_logins();
        if let Some(shot) = &desk.snapshot {
            desk.screen = match shot.screen.as_str() {
                "empties" => Screen::Empties,
                "receiving" => Screen::Receiving,
                "evidence" => Screen::Evidence,
                "logins" => Screen::Logins,
                _ => Screen::Board,
            };
        }
        desk
    }

    fn build() -> Self {
        let path = store::state_path();
        let root = repo_root();
        let (saved, status) = match store::load(&path) {
            Ok(saved) => (saved, String::new()),
            Err(err) => (Saved::default(), err),
        };
        let next_id = saved
            .next_id
            .max(saved.log.iter().map(|entry| entry.id).max().unwrap_or(0) + 1)
            .max(saved.receivings.iter().map(|row| row.id).max().unwrap_or(0) + 1)
            .max(saved.uploads.iter().map(|upload| upload.id).max().unwrap_or(0) + 1);

        Self {
            container: saved.container,
            date: today(),
            shift: saved.shift,
            screen: Screen::Board,
            selected: "its".to_string(),
            problems_only: false,
            this_container_only: false,
            draft_finding: Finding::NotChecked,
            draft_note: String::new(),
            draft_shot: String::new(),
            draft_scope: String::new(),
            checks: saved.checks,
            log: saved.log,
            next_id,
            status,
            path,
            containers_text: saved.containers,
            receivings: saved.receivings,
            draft_line: "Maersk".to_string(),
            draft_size: BoxSize::Forty,
            draft_receive: ReceiveStatus::Receiving,
            browser: BrowserManager::new(root),
            schedule_shot: None,
            schedule_queue: std::collections::VecDeque::new(),
            shot_is_auto: false,
            last_capture: HashMap::new(),
            uploads: saved.uploads,
            paste_text: String::new(),
            upload_warnings: Vec::new(),
            login_dir: credentials_dir(),
            saved_logins: HashMap::new(),
            login_drafts: HashMap::new(),
            clipboard_secret: None,
            snapshot: snapshot::Snapshot::from_env(),
            box_form: empties_ui::BoxForm::default(),
            auto_capture: saved.auto_capture,
            return_locations: saved.return_locations,
            disputes: saved.disputes,
        }
    }

    /// Re-read which logins are saved (IDs only) and reset the edit forms.
    fn refresh_logins(&mut self) {
        self.saved_logins.clear();
        for (account, _) in ACCOUNTS {
            if let Some(login) = credentials::load(&self.login_dir, account) {
                self.saved_logins.insert(
                    account.to_string(),
                    SavedLogin {
                        user_id: login.user_id.clone(),
                        updated_at: login.updated_at.clone(),
                    },
                );
                let draft = self.login_drafts.entry(account.to_string()).or_default();
                draft.user_id = login.user_id;
            }
        }
    }

    fn save_login(&mut self, account: &str) {
        let Some(draft) = self.login_drafts.get(account) else {
            return;
        };
        let user_id = draft.user_id.trim().to_string();
        if user_id.is_empty() {
            self.status = "Enter the login ID.".to_string();
            return;
        }
        let password = if draft.password.is_empty() {
            match credentials::load(&self.login_dir, account) {
                Some(existing) => existing.password,
                None => {
                    self.status = "Enter the password.".to_string();
                    return;
                }
            }
        } else {
            draft.password.clone()
        };
        let login = Login {
            account: account.to_string(),
            user_id,
            password,
            updated_at: timestamp(),
        };
        match credentials::save(&self.login_dir, &login) {
            Ok(()) => {
                if let Some(draft) = self.login_drafts.get_mut(account) {
                    draft.password.clear();
                    draft.show = false;
                }
                self.status = format!("{} login saved.", account_name(account));
            }
            Err(err) => self.status = format!("Could not save login: {err}"),
        }
        self.refresh_logins();
    }

    fn copy_login_id(&mut self, ui: &egui::Ui, account: &str) {
        match credentials::load(&self.login_dir, account) {
            Some(login) => {
                ui.ctx().copy_text(login.user_id);
                self.status = format!("{} ID copied.", account_name(account));
            }
            None => self.status = "No saved login.".to_string(),
        }
    }

    fn copy_login_password(&mut self, ui: &egui::Ui, account: &str) {
        match credentials::load(&self.login_dir, account) {
            Some(login) => {
                ui.ctx().copy_text(login.password.clone());
                self.clipboard_secret = Some((login.password, now_secs() + SECRET_SECONDS));
                self.status = format!(
                    "{} password copied. The clipboard is cleared in {} seconds.",
                    account_name(account),
                    SECRET_SECONDS as u64
                );
            }
            None => self.status = "No saved login.".to_string(),
        }
    }

    fn reveal_password(&mut self, account: &str) {
        match credentials::load(&self.login_dir, account) {
            Some(login) => {
                let draft = self.login_drafts.entry(account.to_string()).or_default();
                draft.revealed = Some((login.password, now_secs() + SECRET_SECONDS));
            }
            None => self.status = "No saved login.".to_string(),
        }
    }

    /// Hide shown passwords when their time is up or the user leaves Logins,
    /// and clear a copied password from the clipboard if it is still there.
    fn expire_secrets(&mut self) {
        let now = now_secs();
        let leaving = self.screen != Screen::Logins;
        for draft in self.login_drafts.values_mut() {
            if draft
                .revealed
                .as_ref()
                .is_some_and(|(_, until)| leaving || now >= *until)
            {
                draft.revealed = None;
            }
        }
        if let Some((secret, until)) = &self.clipboard_secret {
            if now >= *until {
                clear_clipboard_if(secret);
                self.clipboard_secret = None;
            }
        }
    }

    fn delete_login(&mut self, account: &str) {
        match credentials::delete(&self.login_dir, account) {
            Ok(()) => {
                self.login_drafts.remove(account);
                self.status = format!("{} login deleted.", account_name(account));
            }
            Err(err) => self.status = format!("Could not delete login: {err}"),
        }
        self.refresh_logins();
    }

    fn snapshot(&self) -> Saved {
        Saved {
            container: self.container.clone(),
            shift: self.shift,
            checks: self.checks.clone(),
            log: self.log.clone(),
            next_id: self.next_id,
            containers: self.containers_text.clone(),
            receivings: self.receivings.clone(),
            uploads: self.uploads.clone(),
            auto_capture: self.auto_capture,
            return_locations: self.return_locations.clone(),
            disputes: self.disputes.clone(),
        }
    }

    fn set_auto_capture(&mut self, on: bool) {
        self.auto_capture = on;
        self.browser.set_auto_capture(on);
        self.persist();
        self.status = if on {
            "After each sign-in: save the page, then look for empty appointments (APM, LBCT) with screenshots.".to_string()
        } else {
            "Automatic check and screenshots after sign-in are off.".to_string()
        };
    }

    fn persist(&mut self) {
        match store::save(&self.path, &self.snapshot()) {
            Ok(()) => {}
            Err(err) => self.status = format!("Could not save: {err}"),
        }
    }

    fn sync_draft(&mut self) {
        let scope = format!("{}|{}", self.scope_prefix(), self.selected);
        if scope == self.draft_scope {
            return;
        }
        self.draft_scope = scope;
        if let Some(check) = self.checks.get(&self.current_key()) {
            self.draft_finding = check.finding;
            self.draft_note = check.note.clone();
            self.draft_shot = check.screenshot.clone();
        } else {
            self.draft_finding = Finding::NotChecked;
            self.draft_note.clear();
            self.draft_shot.clear();
        }
    }

    fn scope_prefix(&self) -> String {
        format!(
            "{}|{}|{}",
            self.date,
            self.shift.key(),
            normalize_container(&self.container)
        )
    }

    fn current_key(&self) -> String {
        check_key(&self.date, self.shift, &self.container, &self.selected)
    }

    fn finding_for(&self, terminal_id: &str) -> Finding {
        self.checks
            .get(&check_key(
                &self.date,
                self.shift,
                &self.container,
                terminal_id,
            ))
            .map(|check| check.finding)
            .unwrap_or(Finding::NotChecked)
    }

    fn tally(&self) -> [usize; 4] {
        let mut no_return = 0;
        let mut dual = 0;
        let mut open = 0;
        let mut not_checked = 0;
        for terminal in terminals() {
            match self.finding_for(terminal.id) {
                Finding::Closed | Finding::NoSlots => no_return += 1,
                Finding::DualOnly => dual += 1,
                Finding::Open => open += 1,
                Finding::NotChecked | Finding::CouldNotCheck => not_checked += 1,
            }
        }
        [no_return, dual, open, not_checked]
    }

    fn shift_date(&mut self, days: i64) {
        let parsed = chrono::NaiveDate::parse_from_str(&self.date, "%Y-%m-%d")
            .unwrap_or_else(|_| chrono::Local::now().date_naive());
        self.date = (parsed + chrono::Duration::days(days))
            .format("%Y-%m-%d")
            .to_string();
    }

    fn record(&mut self) {
        let container = normalize_container(&self.container);
        if container.is_empty() {
            self.status = "Enter a container number before recording.".to_string();
            return;
        }
        if self.draft_finding == Finding::NotChecked {
            self.status = "Pick a finding before recording.".to_string();
            return;
        }
        let Some(terminal) = terminal(&self.selected) else {
            return;
        };
        let at = timestamp();
        let check = Check {
            finding: self.draft_finding,
            note: self.draft_note.trim().to_string(),
            screenshot: self.draft_shot.trim().to_string(),
            updated_at: at.clone(),
        };
        self.checks.insert(
            check_key(&self.date, self.shift, &container, terminal.id),
            check,
        );
        self.log.push(Evidence {
            id: self.next_id,
            at,
            date: self.date.clone(),
            shift: self.shift,
            container,
            terminal_id: terminal.id.to_string(),
            terminal_name: terminal.name.to_string(),
            finding: self.draft_finding,
            note: self.draft_note.trim().to_string(),
            screenshot: self.draft_shot.trim().to_string(),
        });
        self.next_id += 1;
        self.container = normalize_container(&self.container);
        self.persist();
        self.status = format!(
            "Recorded {} for {}: {}.",
            terminal.name,
            self.container,
            self.draft_finding.label()
        );
    }

    fn container_list(&self) -> Vec<String> {
        self.containers_text
            .lines()
            .map(normalize_container)
            .filter(|container| !container.is_empty())
            .collect()
    }

    fn open_browser(&mut self, terminal_id: &str) {
        match self.browser.open(terminal_id) {
            Ok(()) => {
                self.status = "Browser opening. Sign in there, then check containers.".to_string();
            }
            Err(err) => self.status = err,
        }
    }

    fn check_containers(&mut self, terminal_id: &str) {
        let mut containers = self.container_list();
        if containers.is_empty() {
            let one = normalize_container(&self.container);
            if !one.is_empty() {
                containers.push(one);
            }
        }
        if containers.is_empty() {
            self.status = "Enter at least one container to check.".to_string();
            return;
        }
        match self
            .browser
            .check(terminal_id, &containers, &self.date, self.shift)
        {
            Ok(()) => self.status = format!("Checking {} containers.", containers.len()),
            Err(err) => self.status = err,
        }
    }

    /// Right after a sign-in: look up empty-return appointments for the check
    /// list on terminals that have an automatic check. The browser saves the
    /// page it landed on first, then a screenshot for each container.
    fn check_after_sign_in(&mut self, terminal_id: &str) {
        let Some((known, _)) = portal(terminal_id) else {
            return;
        };
        if !known.auto_check {
            return;
        }
        let containers = self.container_list();
        if containers.is_empty() {
            self.status = format!(
                "{} signed in. Add containers to the check list to look for empty appointments automatically.",
                known.name
            );
            return;
        }
        match self
            .browser
            .check(terminal_id, &containers, &self.date, self.shift)
        {
            Ok(()) => {
                self.status = format!(
                    "{} signed in. Looking for empty appointments for {} containers.",
                    known.name,
                    containers.len()
                )
            }
            Err(err) => self.status = err,
        }
    }

    fn focus_browser(&mut self, terminal_id: &str) {
        if let Err(err) = self.browser.focus(terminal_id) {
            self.status = err;
        }
    }

    fn close_browser(&mut self, terminal_id: &str) {
        if let Err(err) = self.browser.close(terminal_id) {
            self.status = err;
            return;
        }
        self.status = "Closing the browser.".to_string();
    }

    fn import_results(&mut self, batch: ImportBatch) {
        let Some((known, _)) = portal(&batch.terminal_id) else {
            self.status = "Browser returned a terminal this desk does not know.".to_string();
            return;
        };
        let date = if batch.date.is_empty() {
            self.date.clone()
        } else {
            batch.date
        };
        let shift = batch.shift.unwrap_or(self.shift);
        let mut recorded = 0;
        for result in batch.results {
            let container = normalize_container(&result.container);
            if container.is_empty() || result.finding == Finding::NotChecked {
                continue;
            }
            let note = result.note.trim().to_string();
            let screenshot = result.screenshot.trim().to_string();
            let at = timestamp();
            self.checks.insert(
                check_key(&date, shift, &container, known.id),
                Check {
                    finding: result.finding,
                    note: note.clone(),
                    screenshot: screenshot.clone(),
                    updated_at: at.clone(),
                },
            );
            self.log.push(Evidence {
                id: self.next_id,
                at,
                date: date.clone(),
                shift,
                container,
                terminal_id: known.id.to_string(),
                terminal_name: known.name.to_string(),
                finding: result.finding,
                note,
                screenshot,
            });
            self.next_id += 1;
            recorded += 1;
        }
        self.persist();
        self.status = format!("Recorded {recorded} checks for {}.", known.name);
    }

    fn add_receiving(&mut self, terminal_id: &str) {
        let line = self.draft_line.trim().to_string();
        if line.is_empty() {
            self.status = "Enter a shipping line.".to_string();
            return;
        }
        let (date, shift, size, status) =
            (self.date.clone(), self.shift, self.draft_size, self.draft_receive);
        self.upsert_receiving(&date, shift, terminal_id, line, size, status);
        self.persist();
        self.status = "Receiving location saved.".to_string();
    }

    /// Add or update one receiving row, keyed by date, shift, terminal, line and size.
    fn upsert_receiving(
        &mut self,
        date: &str,
        shift: Shift,
        terminal_id: &str,
        line: String,
        size: BoxSize,
        status: ReceiveStatus,
    ) {
        if let Some(row) = self.receivings.iter_mut().find(|row| {
            row.date == date
                && row.shift == shift
                && row.terminal_id == terminal_id
                && row.line.eq_ignore_ascii_case(&line)
                && row.size == size
        }) {
            row.status = status;
            row.line = line;
            row.updated_at = timestamp();
        } else {
            let id = self.next_id;
            self.next_id += 1;
            self.receivings.push(Receiving {
                id,
                date: date.to_string(),
                shift,
                terminal_id: terminal_id.to_string(),
                line,
                size,
                status,
                updated_at: timestamp(),
            });
        }
    }

    fn remove_receiving(&mut self, id: u64) {
        self.receivings.retain(|row| row.id != id);
        self.persist();
    }

    /// Queue a public-schedule capture. Automatic captures don't touch the finding form.
    fn shoot_schedule(&mut self, terminal_id: &str, auto: bool) {
        let running = self
            .schedule_shot
            .as_ref()
            .is_some_and(|job| job.terminal_id == terminal_id);
        if running || self.schedule_queue.iter().any(|(id, _)| id == terminal_id) {
            return;
        }
        self.schedule_queue.push_back((terminal_id.to_string(), auto));
        self.start_next_schedule();
    }

    fn start_next_schedule(&mut self) {
        if self.schedule_shot.is_some() {
            return;
        }
        while let Some((terminal_id, auto)) = self.schedule_queue.pop_front() {
            match ScheduleShot::start(self.browser.root(), &terminal_id) {
                Ok(job) => {
                    let name = terminal(&terminal_id).map(|t| t.name).unwrap_or("terminal");
                    self.status = format!("Capturing {name} empty receiving schedule.");
                    self.schedule_shot = Some(job);
                    self.shot_is_auto = auto;
                    return;
                }
                Err(err) => self.status = err,
            }
        }
    }

    /// Capture every terminal whose schedule the desk can read, unless done in the last 30 minutes.
    fn capture_receiving(&mut self, force: bool) {
        let now = now_secs();
        for id in SCHEDULE_READERS {
            let fresh = self
                .last_capture
                .get(*id)
                .is_some_and(|(at, _)| now - at < 30.0 * 60.0);
            if force || !fresh {
                self.shoot_schedule(id, true);
            }
        }
    }

    fn poll_schedule_shot(&mut self) {
        let Some(job) = self.schedule_shot.as_ref() else {
            self.start_next_schedule();
            return;
        };
        let Some(outcome) = job.poll() else {
            return;
        };
        let terminal_id = job.terminal_id.clone();
        let auto = self.shot_is_auto;
        self.schedule_shot = None;
        let name = terminal(&terminal_id).map(|t| t.name).unwrap_or("terminal");
        match outcome {
            Ok(shot) => {
                let path = shot.screenshot;
                let count = shot.receiving.len();
                for row in shot.receiving {
                    self.upsert_receiving(
                        &row.date, row.shift, &terminal_id, row.line, row.size, row.status,
                    );
                }
                self.last_capture
                    .insert(terminal_id.clone(), (now_secs(), timestamp()));
                if count > 0 {
                    self.persist();
                }
                let rows = if count > 0 {
                    format!(" {count} receiving rows updated.")
                } else {
                    String::new()
                };
                if !auto && self.selected == terminal_id {
                    self.draft_shot = path.clone();
                    self.status =
                        format!("Schedule saved to {path}.{rows} Set the finding and record it.");
                } else {
                    self.status = format!("{name} schedule captured.{rows}");
                }
            }
            Err(err) => self.status = format!("{name} schedule: {err}"),
        }
        self.start_next_schedule();
    }

    fn add_upload(&mut self, source: String, parsed: upload::Parsed) {
        self.upload_warnings = parsed.warnings;
        if parsed.boxes.is_empty() {
            self.status = format!("No containers found in {source}.");
            return;
        }
        let count = parsed.boxes.len();
        self.uploads.push(Upload {
            id: self.next_id,
            date: self.date.clone(),
            uploaded_at: timestamp(),
            source: source.clone(),
            boxes: parsed.boxes,
        });
        self.next_id += 1;
        self.persist();
        let warn = if self.upload_warnings.is_empty() {
            String::new()
        } else {
            format!(" {} warnings below.", self.upload_warnings.len())
        };
        self.status = format!("Saved {count} empties from {source} for {}.{warn}", self.date);
    }

    fn upload_file(&mut self, path: &std::path::Path) {
        let source = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        match upload::parse_file(path) {
            Ok(parsed) => self.add_upload(source, parsed),
            Err(err) => self.status = err,
        }
    }

    fn upload_pasted(&mut self) {
        let text = std::mem::take(&mut self.paste_text);
        let parsed = upload::parse_text(&text);
        if parsed.boxes.is_empty() {
            self.paste_text = text;
        }
        self.add_upload("pasted".to_string(), parsed);
    }

    fn remove_upload(&mut self, id: u64) {
        self.uploads.retain(|upload| upload.id != id);
        self.persist();
        self.status = "Upload removed.".to_string();
    }

    fn copy_proof(&self, ui: &egui::Ui) {
        let mut text = proof_packet(
            &self.container,
            &self.date,
            self.shift,
            &timestamp(),
            &self.checks,
        );
        let today = chrono::Local::now().date_naive();
        if let Some(dispute) = self.disputes.get(&normalize_container(&self.container)) {
            let lines = dispute.lines(today);
            if !lines.is_empty() {
                text.push_str("\n\nDispute\n");
                text.push_str(&lines.join("\n"));
            }
        }
        text.push_str(&self.coverage_text());
        ui.ctx().copy_text(text);
    }

    /// Carrier screenshot windows for the Board container, when it is on this
    /// date's Empties list (that date is its waiver date).
    fn coverage_text(&self) -> String {
        let container = normalize_container(&self.container);
        let Some(item) = boxes_for_day(&self.uploads, &self.date)
            .into_iter()
            .find(|item| item.container == container)
        else {
            return String::new();
        };
        let (locations, guessed) =
            proof::locations_for(&item.line, &self.return_locations, &self.receivings);
        let cover = proof::coverage(&item, &self.date, locations, guessed, &self.log);
        let mut lines = vec![
            String::new(),
            String::new(),
            format!(
                "Carrier screenshot windows (waiver date {}, line {}): {} of {} covered",
                self.date,
                if item.line.is_empty() { "unknown" } else { item.line.as_str() },
                cover.done(),
                cover.total()
            ),
        ];
        let names: Vec<&str> = cover.locations.iter().map(|t| t.name).collect();
        if !names.is_empty() {
            lines.push(format!(
                "Return locations: {}{}",
                names.join(", "),
                if cover.guessed { " (from the receiving board; confirm against the carrier's list)" } else { "" }
            ));
        }
        lines.extend(cover.lines());
        lines.join("\n")
    }

    fn copy_board(&self, ui: &egui::Ui) {
        ui.ctx().copy_text(full_board(
            &self.container,
            &self.date,
            self.shift,
            &self.checks,
        ));
    }

    fn copy_receiving(&self, ui: &egui::Ui) {
        ui.ctx()
            .copy_text(receiving_board(&self.date, self.shift, &self.receivings));
    }
}

impl eframe::App for Desk {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(shot) = &mut self.snapshot {
            shot.tick(ui.ctx());
        }
        self.poll_schedule_shot();
        self.expire_secrets();
        let secrets_pending = self.clipboard_secret.is_some()
            || self.login_drafts.values().any(|draft| draft.revealed.is_some());
        if self.browser.any_open() || self.schedule_shot.is_some() || secrets_pending {
            // Keep polling the Python side even when the mouse is still.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(500));
        }
        for batch in self.browser.poll() {
            self.import_results(batch);
        }
        let signed_in = self.browser.take_newly_online();
        if !signed_in.is_empty() {
            // Someone just signed in: capture what each terminal is receiving.
            self.capture_receiving(false);
            if self.auto_capture {
                for terminal_id in &signed_in {
                    self.check_after_sign_in(terminal_id);
                }
            }
        }
        for file in ui.ctx().input(|input| input.raw.dropped_files.clone()) {
            let path = file.path().to_path_buf();
            if !path.as_os_str().is_empty() {
                self.screen = Screen::Empties;
                self.upload_file(&path);
            }
        }
        if let Some(message) = self.browser.take_notices().pop() {
            self.status = message;
        }
        let bar = Frame::new().fill(PANEL).inner_margin(Margin::symmetric(16, 10));
        let side = Frame::new().fill(PANEL).inner_margin(Margin::symmetric(14, 8));
        let main = Frame::new().fill(BG).inner_margin(Margin::symmetric(20, 4));
        egui::Panel::top("header")
            .frame(bar)
            .show(ui, |ui| header(self, ui));
        self.sync_draft();
        egui::Panel::bottom("footer")
            .frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(16, 8)))
            .show(ui, |ui| footer(self, ui));

        if self.screen == Screen::Board {
            egui::Panel::left("summary")
                .frame(side)
                .default_size(232.0)
                .resizable(false)
                .show(ui, |ui| summary(self, ui));
            egui::Panel::right("detail")
                .frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(16, 8)))
                .default_size(370.0)
                .min_size(320.0)
                .show(ui, |ui| detail(self, ui));
            egui::CentralPanel::default()
                .frame(main)
                .show(ui, |ui| board(self, ui));
        } else {
            egui::CentralPanel::default().frame(main).show(ui, |ui| {
                // Keep long screens readable on wide windows.
                let size = Vec2::new(ui.available_width().min(1100.0), ui.available_height());
                ui.allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
                    ui.set_max_width(size.x);
                    match self.screen {
                        Screen::Receiving => receiving(self, ui),
                        Screen::Empties => empties_ui::empties(self, ui),
                        Screen::Logins => logins(self, ui),
                        _ => evidence(self, ui),
                    }
                });
            });
        }
    }
}

fn header(desk: &mut Desk, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(4.0, 22.0), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(2), ACCENT);
        ui.label(RichText::new("Empty Return Desk").size(17.0).strong())
            .on_hover_text("Which empties each terminal is taking, and proof when a return is refused");
        ui.add_space(18.0);

        let today_count = boxes_for_day(&desk.uploads, &desk.date).len();
        let evidence_count = desk.log.len();
        let logins = format!("{}/{}", desk.saved_logins.len(), ACCOUNTS.len());
        screen_tab(ui, &mut desk.screen, Screen::Empties, "Empties", Some(today_count.to_string()));
        screen_tab(ui, &mut desk.screen, Screen::Board, "Board", None);
        screen_tab(ui, &mut desk.screen, Screen::Receiving, "Receiving", None);
        screen_tab(ui, &mut desk.screen, Screen::Evidence, "Evidence", Some(evidence_count.to_string()));
        screen_tab(ui, &mut desk.screen, Screen::Logins, "Logins", Some(logins));

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let previous_shift = desk.shift;
            ComboBox::from_id_salt("shift")
                .selected_text(desk.shift.label())
                .width(96.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut desk.shift, Shift::First, Shift::First.label());
                    ui.selectable_value(&mut desk.shift, Shift::Second, Shift::Second.label());
                });
            if desk.shift != previous_shift {
                desk.persist();
            }

            if desk.date != today() && ui.button("Today").clicked() {
                desk.date = today();
            }
            if ui.button("▶").on_hover_text("Next day").clicked() {
                desk.shift_date(1);
            }
            ui.label(RichText::new(&desk.date).monospace().strong());
            if ui.button("◀").on_hover_text("Previous day").clicked() {
                desk.shift_date(-1);
            }
            ui.add_space(10.0);

            let before = desk.container.clone();
            ui.add(
                TextEdit::singleline(&mut desk.container)
                    .desired_width(140.0)
                    .font(egui::TextStyle::Monospace)
                    .hint_text("e.g. MSMU1234567"),
            );
            if desk.container != before {
                desk.persist();
            }
            ui.label(RichText::new("Container").small().color(MUTED));
        });
    });
    browser_list(desk, ui);
}

/// A top-level navigation tab: text with an accent underline when selected.
fn screen_tab(
    ui: &mut egui::Ui,
    current: &mut Screen,
    screen: Screen,
    label: &str,
    count: Option<String>,
) {
    let selected = *current == screen;
    let color = if selected { TEXT } else { MUTED };
    let mut text = egui::text::LayoutJob::default();
    RichText::new(label)
        .size(14.0)
        .strong()
        .color(color)
        .append_to(&mut text, ui.style(), egui::FontSelection::Default, Align::Center);
    if let Some(count) = count {
        RichText::new(format!("  {count}"))
            .size(11.5)
            .color(if selected { ACCENT } else { MUTED })
            .append_to(&mut text, ui.style(), egui::FontSelection::Default, Align::Center);
    }
    let response = ui.add(Button::new(text).frame(false).min_size(Vec2::new(0.0, 28.0)));
    if selected {
        let rect = response.rect;
        let line = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 4.0, rect.bottom() + 3.0),
            egui::pos2(rect.right() - 4.0, rect.bottom() + 5.0),
        );
        ui.painter().rect_filled(line, CornerRadius::same(1), ACCENT);
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if response.clicked() {
        *current = screen;
    }
}

fn state_color(state: BrowserState) -> Color32 {
    match state {
        BrowserState::Online => OPEN,
        BrowserState::Starting | BrowserState::Checking => ACCENT,
        BrowserState::NeedsLogin
        | BrowserState::SignedOut
        | BrowserState::NotResponding
        | BrowserState::Error => REFUSAL,
    }
}

fn short_url(url: &str, max: usize) -> String {
    let trimmed = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    if trimmed.chars().count() <= max {
        trimmed.to_string()
    } else {
        let head: String = trimmed.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

enum BrowserAction {
    Show,
    Check,
    Capture,
    OpenCapture(String),
    Close,
}

/// Every open terminal browser: sign-in state, address, and controls.
fn browser_list(desk: &mut Desk, ui: &mut egui::Ui) {
    if !desk.browser.any_open() {
        return;
    }
    let now = now_secs();
    let attention = desk
        .browser
        .slots
        .iter()
        .filter(|slot| slot.state(now).needs_attention())
        .count();
    let mut actions: Vec<(BrowserAction, String)> = Vec::new();
    let mut toggle_auto: Option<bool> = None;

    ui.add_space(8.0);
    card()
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("BROWSERS ({})", desk.browser.slots.len()))
                        .small()
                        .strong()
                        .color(MUTED),
                );
                if attention > 0 {
                    ui.label(
                        RichText::new(format!("{attention} need you"))
                            .small()
                            .strong()
                            .color(REFUSAL),
                    );
                } else {
                    ui.label(RichText::new("all signed in").small().color(OPEN));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut on = desk.auto_capture;
                    if ui
                        .checkbox(&mut on, RichText::new("Check & screenshot after sign-in").small())
                        .on_hover_text(AUTO_HELP)
                        .changed()
                    {
                        toggle_auto = Some(on);
                    }
                });
            });
            egui::Grid::new("browser-list")
                .num_columns(5)
                .spacing(Vec2::new(12.0, 6.0))
                .show(ui, |ui| {
                    for slot in &desk.browser.slots {
                        let state = slot.state(now);
                        let found = portal(&slot.terminal_id);
                        let name = browser_name(&slot.terminal_id);
                        let auto = slot.terminal_id != EMODAL
                            && found.map(|(t, _)| t.auto_check).unwrap_or(false);

                        ui.horizontal(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                            ui.painter()
                                .circle_filled(rect.center(), 5.0, state_color(state));
                            ui.label(RichText::new(name).strong());
                        });
                        ui.label(RichText::new(state.label()).color(state_color(state)));

                        let address = if slot.url.is_empty() {
                            "—".to_string()
                        } else {
                            short_url(&slot.url, 64)
                        };
                        let mut hover = slot.url.clone();
                        if slot.tabs.len() > 1 {
                            hover = slot
                                .tabs
                                .iter()
                                .map(|tab| {
                                    if tab.title.is_empty() {
                                        tab.url.clone()
                                    } else {
                                        format!("{} — {}", tab.title, tab.url)
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                        }
                        ui.horizontal(|ui| {
                            let link = ui
                                .add(
                                    egui::Label::new(
                                        RichText::new(address).monospace().small(),
                                    )
                                    .sense(Sense::click()),
                                )
                                .on_hover_text(format!("{hover}\n\nClick to copy the address."));
                            if link.clicked() && !slot.url.is_empty() {
                                ui.ctx().copy_text(slot.url.clone());
                            }
                            if slot.tabs.len() > 1 {
                                ui.label(
                                    RichText::new(format!("+{} tabs", slot.tabs.len() - 1))
                                        .small()
                                        .color(MUTED),
                                );
                            }
                        });

                        let seen = match slot.seconds_since_seen(now) {
                            Some(secs) => format!("seen {secs}s ago"),
                            None => "starting".to_string(),
                        };
                        ui.label(RichText::new(seen).small().color(MUTED))
                            .on_hover_text(slot.message.clone());

                        ui.horizontal(|ui| {
                            if slot.is_closing() {
                                ui.label(RichText::new("Closing…").small().color(MUTED));
                                return;
                            }
                            if ui.button("Show").on_hover_text("Bring this browser window to the front").clicked() {
                                actions.push((BrowserAction::Show, slot.terminal_id.clone()));
                            }
                            if auto
                                && state == BrowserState::Online
                                && ui.button("Check containers").clicked()
                            {
                                actions.push((BrowserAction::Check, slot.terminal_id.clone()));
                            }
                            if state == BrowserState::Online
                                && ui
                                    .button("Save page")
                                    .on_hover_text("Save the page this browser is on (full page), e.g. the empty receiving page")
                                    .clicked()
                            {
                                actions.push((BrowserAction::Capture, slot.terminal_id.clone()));
                            }
                            if !slot.last_capture.is_empty()
                                && ui
                                    .small_button("Open saved")
                                    .on_hover_text(slot.last_capture.clone())
                                    .clicked()
                            {
                                actions.push((
                                    BrowserAction::OpenCapture(slot.last_capture.clone()),
                                    slot.terminal_id.clone(),
                                ));
                            }
                            if ui.button("Close").clicked() {
                                actions.push((BrowserAction::Close, slot.terminal_id.clone()));
                            }
                        });
                        ui.end_row();

                        if state.needs_attention() && !slot.message.is_empty() {
                            ui.label("");
                            ui.label(
                                RichText::new(slot.message.clone())
                                    .small()
                                    .color(MUTED),
                            );
                            ui.end_row();
                        }
                    }
                });
        });

    if let Some(on) = toggle_auto {
        desk.set_auto_capture(on);
    }
    for (action, terminal_id) in actions {
        match action {
            BrowserAction::Show => desk.focus_browser(&terminal_id),
            BrowserAction::Check => desk.check_containers(&terminal_id),
            BrowserAction::Capture => {
                if let Err(err) = desk.browser.capture(&terminal_id) {
                    desk.status = err;
                }
            }
            BrowserAction::OpenCapture(path) => open_path(&path),
            BrowserAction::Close => desk.close_browser(&terminal_id),
        }
    }
}

/// B/L, invoice number and invoice date for the Board container, with the
/// 30-day dispute deadline.
fn dispute_fields(desk: &mut Desk, ui: &mut egui::Ui, container: &str) {
    let mut dispute = desk.disputes.get(container).cloned().unwrap_or_default();
    let before = dispute.clone();
    let line = boxes_for_day(&desk.uploads, &desk.date)
        .into_iter()
        .find(|item| item.container == container)
        .map(|item| item.line)
        .unwrap_or_default();
    ui.add_space(8.0);
    egui::Grid::new("dispute-fields")
        .num_columns(2)
        .spacing(Vec2::new(8.0, 4.0))
        .show(ui, |ui| {
            for (label, value, hint) in [
                ("B/L", &mut dispute.bl, "Bill of lading no."),
                ("Invoice", &mut dispute.invoice, "Per diem invoice no."),
                ("Inv. date", &mut dispute.invoice_date, "YYYY-MM-DD"),
            ] {
                ui.label(RichText::new(label).small().color(MUTED));
                ui.add(TextEdit::singleline(value).hint_text(hint).desired_width(f32::INFINITY));
                ui.end_row();
            }
        });
    if let Some(hint) = dispute.hint(&line) {
        ui.label(RichText::new(hint).small().color(DUAL));
    }
    if let Some((text, level)) = dispute.deadline_text(chrono::Local::now().date_naive()) {
        let color = match level {
            0 => MUTED,
            1 => DUAL,
            _ => REFUSAL,
        };
        ui.label(RichText::new(text).small().strong().color(color));
    }
    if dispute != before {
        if dispute.is_empty() {
            desk.disputes.remove(container);
        } else {
            desk.disputes.insert(container.to_string(), dispute);
        }
        desk.persist();
    }
}

fn summary(desk: &mut Desk, ui: &mut egui::Ui) {
    ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(10.0);
        section_label(ui, "This check");
        let container = normalize_container(&desk.container);
        if container.is_empty() {
            ui.label(RichText::new("No container").size(16.0).strong().color(MUTED));
        } else {
            ui.label(RichText::new(&container).size(16.0).monospace().strong());
        }
        ui.label(
            RichText::new(format!("{}  ·  {}", desk.date, desk.shift.label()))
                .small()
                .color(MUTED),
        );
        if !container.is_empty() {
            dispute_fields(desk, ui, &container);
        }
        ui.add_space(12.0);

        let [no_return, dual, open, not_checked] = desk.tally();
        let gap = ui.spacing().item_spacing.x;
        let half = (ui.available_width() - gap) / 2.0;
        for row in [
            [(no_return, "No return", REFUSAL), (dual, "Dual only", DUAL)],
            [(open, "Open", OPEN), (not_checked, "Unchecked", MUTED)],
        ] {
            ui.horizontal(|ui| {
                for (value, label, color) in row {
                    stat(ui, half, value, label, color);
                }
            });
        }

        ui.add_space(10.0);
        ui.checkbox(&mut desk.problems_only, "Problems only")
            .on_hover_text("Show only terminals recorded as closed, no slots, or dual only.");

        ui.add_space(18.0);
        section_label(ui, &format!("Containers to check · {}", desk.container_list().len()));
        ui.label(
            RichText::new("One per line. Used when the browser checks appointments.")
                .small()
                .color(MUTED),
        );
        ui.add_space(2.0);
        let before = desk.containers_text.clone();
        ui.add(
            TextEdit::multiline(&mut desk.containers_text)
                .desired_rows(6)
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Monospace)
                .hint_text("MSMU1234567"),
        );
        if desk.containers_text != before {
            desk.persist();
        }
    });
}

fn stat(ui: &mut egui::Ui, width: f32, value: usize, label: &str, color: Color32) {
    let active = value > 0 && color != MUTED;
    Frame::new()
        .fill(if active { tint(color, 0.12) } else { SURFACE })
        .stroke(Stroke::new(1.0, if active { tint(color, 0.35) } else { BORDER }))
        .corner_radius(10)
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(width - 20.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(
                    RichText::new(value.to_string())
                        .size(22.0)
                        .color(if value > 0 { color } else { MUTED })
                        .strong(),
                );
                ui.label(RichText::new(label).small().color(MUTED));
            });
        });
}

fn board(desk: &mut Desk, ui: &mut egui::Ui) {
    page_title(
        ui,
        "Terminals",
        "Open a terminal's browser, sign in, then check the containers that have no return appointment.",
    );
    if normalize_container(&desk.container).is_empty() {
        Frame::new()
            .fill(tint(ACCENT, 0.12))
            .stroke(Stroke::new(1.0, tint(ACCENT, 0.4)))
            .corner_radius(8)
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new("Enter a container number at the top before you record a proof.")
                        .color(ACCENT),
                );
            });
        ui.add_space(6.0);
    }

    ScrollArea::vertical().show(ui, |ui| {
        for port in [Port::LosAngeles, Port::LongBeach] {
            let rows: Vec<&Terminal> = terminals()
                .iter()
                .filter(|terminal| terminal.port == port)
                .filter(|terminal| {
                    !desk.problems_only || desk.finding_for(terminal.id).is_refusal()
                })
                .collect();
            if rows.is_empty() {
                continue;
            }
            ui.add_space(8.0);
            section_label(ui, port.label());
            ui.add_space(2.0);
            for terminal in rows {
                terminal_row(desk, ui, terminal);
            }
        }
        if desk.problems_only && desk.tally()[0] + desk.tally()[1] == 0 {
            ui.add_space(8.0);
            ui.label(
                RichText::new("Nothing is recorded as unavailable for this container, date, and shift.")
                    .color(MUTED),
            );
        }
        ui.add_space(12.0);
    });
}

fn terminal_row(desk: &mut Desk, ui: &mut egui::Ui, terminal: &Terminal) {
    let finding = desk.finding_for(terminal.id);
    let selected = desk.selected == terminal.id;
    let browser_state = terminal
        .portals()
        .iter()
        .find_map(|p| desk.browser.get(p.id))
        .map(|slot| slot.state(now_secs()));
    let id = ui.make_persistent_id(("terminal-row", terminal.id));
    let hovered = ui.ctx().read_response(id).is_some_and(|r| r.hovered());
    let fill = if selected {
        Color32::from_rgb(40, 47, 60)
    } else if hovered {
        Color32::from_rgb(36, 41, 49)
    } else {
        SURFACE
    };
    let response = Frame::new()
        .fill(fill)
        .stroke(Stroke::new(
            1.0,
            if selected { tint(ACCENT, 0.55) } else { BORDER },
        ))
        .corner_radius(8)
        .inner_margin(Margin::symmetric(12, 8))
        .outer_margin(Margin::symmetric(0, 3))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(4.0, 30.0), Sense::hover());
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(2), finding_color(finding));
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(terminal.name).size(14.0).strong());
                        ui.label(RichText::new(terminal.pier).small().color(MUTED));
                    });
                    ui.label(RichText::new(terminal.where_to_look()).small().color(MUTED));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    pill(ui, finding.label(), finding_color(finding));
                    if let Some(state) = browser_state {
                        ui.label(
                            RichText::new(state.label())
                                .small()
                                .color(state_color(state)),
                        );
                        dot(ui, state_color(state));
                    }
                });
            });
        })
        .response;
    let response = ui.interact(response.rect, id, Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if response.clicked() {
        desk.selected = terminal.id.to_string();
    }
}

fn detail(desk: &mut Desk, ui: &mut egui::Ui) {
    let Some(terminal) = terminal(&desk.selected) else {
        ui.add_space(12.0);
        ui.label(RichText::new("Select a terminal.").color(MUTED));
        return;
    };
    let terminal_id = terminal.id;
    let name = terminal.name;
    let pier = terminal.pier;
    let port = terminal.port.label();
    let look = terminal.where_to_look();
    let empty_url = terminal.empty_url;
    let guidance = if terminal.auto_check {
        "Open the browser and sign in. Then check containers. A screenshot is saved when no return appointment is offered, and the finding is recorded here."
    } else if empty_url.is_some() {
        "Open the browser and sign in, then record the finding. The public empty schedule is the other place to see whether this line is being received."
    } else {
        "This terminal does not post a public empty list. Open the browser, sign in, try an empty return, and record that no slot is offered."
    };
    let accounts: Vec<&str> = terminal.portals().iter().map(|p| p.account).collect();
    let saved: Vec<&str> = accounts
        .iter()
        .filter(|a| desk.saved_logins.contains_key(**a))
        .map(|a| account_name(a))
        .collect();
    let missing: Vec<&str> = accounts
        .iter()
        .filter(|a| !desk.saved_logins.contains_key(**a))
        .map(|a| account_name(a))
        .collect();
    let login_line = match (saved.is_empty(), missing.is_empty()) {
        (false, true) => format!(
            "Saved {} login will be filled in on the sign-in page. Solve any captcha and press Sign in yourself.",
            saved.join(" / ")
        ),
        (true, _) => format!(
            "No saved {} login. Sign in by hand, or add it under Logins.",
            missing.join(" / ")
        ),
        (false, false) => format!(
            "Saved: {}. Not saved: {} (add it under Logins).",
            saved.join(", "),
            missing.join(", ")
        ),
    };

    ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(name).size(20.0).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let finding = desk.finding_for(terminal_id);
                pill(ui, finding.label(), finding_color(finding));
            });
        });
        ui.label(RichText::new(format!("{pier}  ·  {port}  ·  {look}")).small().color(MUTED));
        ui.add_space(8.0);
        ui.label(guidance);
        ui.add_space(10.0);

        // Browsers: one block per sign-in system.
        let portals = terminal.portals();
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            section_label(ui, "Browser");
            ui.label(RichText::new(&login_line).small().color(MUTED));
            ui.add_space(4.0);
            for p in &portals {
                let open_state = desk.browser.get(p.id).map(|slot| slot.state(now_secs()));
                let has_login = desk.saved_logins.contains_key(p.account);
                ui.horizontal(|ui| {
                    let title = ui.label(RichText::new(p.name).strong());
                    if p.id == EMODAL {
                        title.on_hover_text(format!(
                            "One eModal window for {}. Pick the terminal in eModal's terminal list.",
                            browser_name(EMODAL).trim_start_matches("eModal (").trim_end_matches(')')
                        ));
                        ui.label(RichText::new("shared").small().color(MUTED));
                    }
                    ui.label(
                        RichText::new(if has_login { "login saved" } else { "no saved login" })
                            .small()
                            .color(if has_login { OPEN } else { MUTED }),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(state) = open_state {
                            if ui.button("Show").on_hover_text(format!("Bring the {} browser to the front", p.name)).clicked() {
                                desk.focus_browser(p.id);
                            }
                            ui.label(RichText::new(state.label()).small().color(state_color(state)));
                            dot(ui, state_color(state));
                        } else if ui.add(primary("Open browser")).clicked() {
                            desk.open_browser(p.id);
                        }
                    });
                });
                if let Some(slot) = desk.browser.get(p.id) {
                    if !slot.screen_note.is_empty() {
                        ui.label(RichText::new(slot.screen_note.as_str()).small().color(DUAL));
                    }
                    if !slot.last_capture.is_empty() {
                        let file = std::path::Path::new(&slot.last_capture)
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let path = slot.last_capture.clone();
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("Last saved: {file}")).small().color(MUTED));
                            if ui.small_button("Open").on_hover_text(&path).clicked() {
                                open_path(&path);
                            }
                        });
                    }
                }
            }
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                let mut on = desk.auto_capture;
                if ui
                    .checkbox(&mut on, "Check & screenshot after sign-in")
                    .on_hover_text(AUTO_HELP)
                    .changed()
                {
                    desk.set_auto_capture(on);
                }
            });
            // One folder per browser (the shared eModal window saves to screenshots/emodal).
            ui.horizontal_wrapped(|ui| {
                for p in &portals {
                    let label = if portals.len() > 1 {
                        format!("{} screenshots", p.name)
                    } else {
                        "Screenshots folder".to_string()
                    };
                    if ui
                        .small_button(label)
                        .on_hover_text(format!("Open screenshots/{}", p.id))
                        .clicked()
                    {
                        let folder = repo_root().join("screenshots").join(p.id);
                        let _ = std::fs::create_dir_all(&folder);
                        open_path(&folder.display().to_string());
                    }
                }
            });
        });

        ui.add_space(8.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            section_label(ui, "Links");
            ui.horizontal_wrapped(|ui| {
                if let Some(url) = empty_url {
                    if ui.button("Empty schedule ↗").on_hover_text(url).clicked() {
                        open_link(ui, url);
                    }
                    let busy = desk
                        .schedule_shot
                        .as_ref()
                        .is_some_and(|job| job.terminal_id == terminal_id)
                        || desk.schedule_queue.iter().any(|(id, _)| id == terminal_id);
                    let label = if busy { "Taking screenshot…" } else { "Screenshot schedule" };
                    if ui
                        .add_enabled(!busy, Button::new(label))
                        .on_hover_text("Save a screenshot of the public empty schedule")
                        .clicked()
                    {
                        let id = desk.selected.clone();
                        desk.shoot_schedule(&id, false);
                    }
                }
                for p in &portals {
                    if ui
                        .button(format!("{} ↗", p.name))
                        .on_hover_text(format!("Open in this computer's browser\n{}", p.url))
                        .clicked()
                    {
                        open_link(ui, p.url);
                    }
                }
            });
            if empty_url.is_none() {
                ui.label(RichText::new("No public empty schedule").small().color(MUTED));
            }
        });

        ui.add_space(8.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            section_label(ui, "Record finding");
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                for finding in Finding::ALL {
                    if finding == Finding::NotChecked {
                        continue;
                    }
                    let chosen = desk.draft_finding == finding;
                    let color = finding_color(finding);
                    let button = Button::new(
                        RichText::new(finding.label())
                            .size(12.0)
                            .color(if chosen { color } else { TEXT }),
                    )
                    .fill(if chosen { tint(color, 0.22) } else { SURFACE_HI })
                    .stroke(Stroke::new(1.0, if chosen { color } else { BORDER }));
                    if ui.add(button).clicked() {
                        desk.draft_finding = if chosen { Finding::NotChecked } else { finding };
                    }
                }
            });

            ui.add_space(6.0);
            ui.add(
                TextEdit::multiline(&mut desk.draft_note)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .hint_text("Note: line, size, and what the page showed"),
            );
            ui.add(
                TextEdit::singleline(&mut desk.draft_shot)
                    .desired_width(f32::INFINITY)
                    .hint_text("Screenshot file path (optional)"),
            );

            ui.add_space(6.0);
            let can_record = desk.draft_finding != Finding::NotChecked
                && !normalize_container(&desk.container).is_empty();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(can_record, primary("Record proof").min_size(Vec2::new(0.0, 30.0)))
                    .clicked()
                {
                    desk.record();
                }
                if !can_record {
                    ui.label(
                        RichText::new("Needs a container number and a finding.")
                            .small()
                            .color(MUTED),
                    );
                }
            });
        });
        ui.add_space(12.0);
    });
}

/// Terminals whose public schedule the desk can read into the receiving board.
/// Public schedules saved automatically after a sign-in (TTI is a picture:
/// saved, not read into rows).
const SCHEDULE_READERS: &[&str] = &["apm", "its", "trapac", "tti"];

enum LoginAction {
    Save(String),
    Delete(String),
    CopyId(String),
    CopyPassword(String),
    Reveal(String),
    Hide(String),
}

fn logins(desk: &mut Desk, ui: &mut egui::Ui) {
    let mut action: Option<LoginAction> = None;
    let folder = desk.login_dir.clone();
    ScrollArea::vertical().show(ui, |ui| {
        page_title(
            ui,
            "Logins",
            "When a browser shows a sign-in page, it fills in the saved ID and password. \
             You solve any captcha and press Sign in yourself. Nothing is submitted for you.",
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Saved in {}", folder.display())).small().color(MUTED));
            if ui.small_button("Open folder").clicked() {
                let _ = std::fs::create_dir_all(&folder);
                open_path(&folder.display().to_string());
            }
        });
        ui.label(
            RichText::new("One file per login system, readable only by your Mac user (owner-only).")
                .small()
                .color(MUTED),
        );
        ui.add_space(10.0);

        for (account, name) in ACCOUNTS {
            let used_by: Vec<String> = terminals()
                .iter()
                .flat_map(|terminal| {
                    let many = terminal.portals().len() > 1;
                    terminal
                        .portals()
                        .into_iter()
                        .filter(move |p| p.account == account)
                        .map(move |p| {
                            if many {
                                format!("{} ({})", terminal.name, p.name)
                            } else {
                                terminal.name.to_string()
                            }
                        })
                })
                .collect();
            let saved = desk.saved_logins.get(account);
            let saved_line = saved.map(|login| {
                format!("Saved · ID {} · updated {}", login.user_id, login.updated_at)
            });
            let draft = desk.login_drafts.entry(account.to_string()).or_default();

            card()
                .outer_margin(Margin::symmetric(0, 4))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(name).size(14.0).strong());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            match saved {
                                Some(login) => {
                                    ui.label(
                                        RichText::new(format!("updated {}", login.updated_at))
                                            .small()
                                            .color(MUTED),
                                    );
                                    pill(ui, "Saved", OPEN);
                                }
                                None => {
                                    pill(ui, "Not saved", MUTED);
                                }
                            }
                        });
                    });
                    ui.label(
                        RichText::new(format!("Used by {}", used_by.join(", ")))
                            .small()
                            .color(MUTED),
                    );
                    if let Some(login) = saved {
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("ID").small().color(MUTED));
                            ui.label(RichText::new(&login.user_id).monospace().strong());
                            if ui.small_button("Copy ID").clicked() {
                                action = Some(LoginAction::CopyId(account.to_string()));
                            }
                            ui.add_space(12.0);
                            ui.label(RichText::new("Password").small().color(MUTED));
                            match &draft.revealed {
                                Some((password, until)) => {
                                    ui.label(RichText::new(password).monospace().color(ACCENT));
                                    let left = (until - now_secs()).max(0.0).ceil() as u64;
                                    if ui
                                        .small_button(format!("Hide ({left}s)"))
                                        .clicked()
                                    {
                                        action = Some(LoginAction::Hide(account.to_string()));
                                    }
                                }
                                None => {
                                    ui.label(RichText::new("••••••••").monospace());
                                    if ui
                                        .small_button("Show")
                                        .on_hover_text("Show the saved password for 30 seconds")
                                        .clicked()
                                    {
                                        action = Some(LoginAction::Reveal(account.to_string()));
                                    }
                                }
                            }
                            if ui
                                .small_button("Copy password")
                                .on_hover_text("Copies it; the clipboard is cleared after 30 seconds")
                                .clicked()
                            {
                                action = Some(LoginAction::CopyPassword(account.to_string()));
                            }
                        });
                    }
                    ui.add_space(6.0);
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(if saved.is_some() { "CHANGE" } else { "ADD" }).size(10.5).strong().color(MUTED));
                        ui.add_space(4.0);
                        ui.label(RichText::new("ID").small().color(MUTED));
                        ui.add(
                            TextEdit::singleline(&mut draft.user_id)
                                .desired_width(200.0)
                                .hint_text("Login ID"),
                        );
                        ui.label(RichText::new("Password").small().color(MUTED));
                        let hint = if saved_line.is_some() {
                            "Saved — leave blank to keep"
                        } else {
                            "Password"
                        };
                        ui.add(
                            TextEdit::singleline(&mut draft.password)
                                .password(!draft.show)
                                .desired_width(200.0)
                                .hint_text(hint),
                        );
                        if ui
                            .small_button(if draft.show { "Hide" } else { "Show" })
                            .on_hover_text("Show what you are typing")
                            .clicked()
                        {
                            draft.show = !draft.show;
                        }
                        if ui.button("Save").clicked() {
                            action = Some(LoginAction::Save(account.to_string()));
                        }
                        if saved_line.is_some() {
                            if draft.confirm_delete {
                                if ui
                                    .button(RichText::new("Confirm delete").color(REFUSAL))
                                    .clicked()
                                {
                                    action = Some(LoginAction::Delete(account.to_string()));
                                }
                                if ui.small_button("Cancel").clicked() {
                                    draft.confirm_delete = false;
                                }
                            } else if ui.button("Delete").clicked() {
                                draft.confirm_delete = true;
                            }
                        }
                    });
                });
        }
    });
    match action {
        Some(LoginAction::Save(account)) => desk.save_login(&account),
        Some(LoginAction::Delete(account)) => desk.delete_login(&account),
        Some(LoginAction::CopyId(account)) => desk.copy_login_id(ui, &account),
        Some(LoginAction::CopyPassword(account)) => desk.copy_login_password(ui, &account),
        Some(LoginAction::Reveal(account)) => desk.reveal_password(&account),
        Some(LoginAction::Hide(account)) => {
            if let Some(draft) = desk.login_drafts.get_mut(&account) {
                draft.revealed = None;
            }
        }
        None => {}
    }
}

fn evidence(desk: &mut Desk, ui: &mut egui::Ui) {
    page_title(
        ui,
        "Proof log",
        "Each record is kept. Recording again adds a new line; the board shows the latest finding.",
    );
    ui.checkbox(&mut desk.this_container_only, "This container only");
    ui.add_space(6.0);

    let container = normalize_container(&desk.container);
    let mut rows: Vec<&Evidence> = desk
        .log
        .iter()
        .rev()
        .filter(|entry| !desk.this_container_only || entry.container == container)
        .collect();
    if rows.is_empty() {
        empty_state(
            ui,
            "No proof recorded yet",
            "On the Board, pick a terminal and record what you found.",
        );
        return;
    }

    ScrollArea::vertical().show(ui, |ui| {
        for entry in rows.drain(..) {
            card()
                .outer_margin(Margin::symmetric(0, 3))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&entry.terminal_name).size(14.0).strong());
                        ui.label(RichText::new(&entry.container).monospace());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            pill(ui, entry.finding.label(), finding_color(entry.finding));
                        });
                    });
                    ui.label(
                        RichText::new(format!(
                            "Recorded {}  ·  for {}  ·  {}",
                            entry.at,
                            entry.date,
                            entry.shift.label()
                        ))
                        .small()
                        .color(MUTED),
                    );
                    if !entry.note.is_empty() {
                        ui.add_space(2.0);
                        ui.label(&entry.note);
                    }
                    if !entry.screenshot.is_empty() {
                        ui.horizontal(|ui| {
                            if ui.small_button("Open screenshot").clicked() {
                                open_path(&entry.screenshot);
                            }
                            ui.label(
                                RichText::new(&entry.screenshot)
                                    .small()
                                    .monospace()
                                    .color(MUTED),
                            );
                        });
                    }
                });
        }
        ui.add_space(12.0);
    });
}

/// One receiving row as a compact chip: line, size, status and a remove "×".
/// Painted by hand so a row of chips wraps cleanly. Returns true when removed.
fn receive_chip(ui: &mut egui::Ui, row: &Receiving) -> bool {
    let color = receive_color(row.status);
    let painter = ui.painter();
    let line = painter.layout_no_wrap(row.line.clone(), egui::FontId::proportional(13.0), TEXT);
    let size = painter.layout_no_wrap(row.size.label().to_string(), egui::FontId::proportional(13.0), MUTED);
    let status = painter.layout_no_wrap(row.status.label().to_string(), egui::FontId::proportional(11.5), color);
    let close = painter.layout_no_wrap("×".to_string(), egui::FontId::proportional(15.0), MUTED);
    let gap = 7.0;
    let height = 26.0;
    let width = 10.0 + line.size().x + gap + size.size().x + gap + status.size().x + gap + close.size().x + 8.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let close_rect = egui::Rect::from_min_size(
        egui::pos2(rect.right() - close.size().x - 12.0, rect.top()),
        Vec2::new(close.size().x + 12.0, height),
    );
    let close_response = ui
        .interact(close_rect, ui.make_persistent_id(("chip-remove", row.id)), Sense::click())
        .on_hover_text("Remove");

    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(7),
        tint(color, 0.10),
        Stroke::new(1.0, tint(color, 0.38)),
        egui::StrokeKind::Inside,
    );
    let mut x = rect.left() + 10.0;
    for (galley, fallback) in [(line, TEXT), (size, MUTED), (status, color)] {
        let w = galley.size().x;
        let y = rect.center().y - galley.size().y / 2.0;
        painter.galley(egui::pos2(x, y), galley, fallback);
        x += w + gap;
    }
    let close_color = if close_response.hovered() { REFUSAL } else { MUTED };
    painter.galley(
        close_rect.center() - close.size() / 2.0,
        close,
        close_color,
    );
    if close_response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    close_response.clicked()
}

/// Centered message for a screen with nothing to show yet.
fn empty_state(ui: &mut egui::Ui, title: &str, hint: &str) {
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical_centered(|ui| {
            ui.add_space(18.0);
            ui.label(RichText::new(title).size(15.0).strong());
            ui.label(RichText::new(hint).color(MUTED));
            ui.add_space(18.0);
        });
    });
}

fn receiving(desk: &mut Desk, ui: &mut egui::Ui) {
    page_title(
        ui,
        "Receiving",
        "Each terminal, and which empties it is taking on this date and shift.",
    );

    let mut add = false;
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section_label(ui, "Add a receiving row");
        ui.horizontal_wrapped(|ui| {
        let selected_label = terminal(&desk.selected)
            .map(|item| item.name)
            .unwrap_or("Terminal");
        ComboBox::from_id_salt("recv-terminal")
            .selected_text(selected_label)
            .show_ui(ui, |ui| {
                for item in terminals() {
                    ui.selectable_value(&mut desk.selected, item.id.to_string(), item.name);
                }
            });
        ui.add(
            TextEdit::singleline(&mut desk.draft_line)
                .desired_width(140.0)
                .hint_text("Shipping line"),
        );
        ComboBox::from_id_salt("recv-line")
            .selected_text("Lines")
            .show_ui(ui, |ui| {
                for line in LINES {
                    if ui
                        .selectable_label(desk.draft_line == *line, *line)
                        .clicked()
                    {
                        desk.draft_line = (*line).to_string();
                    }
                }
            });
        ComboBox::from_id_salt("recv-size")
            .selected_text(desk.draft_size.label())
            .show_ui(ui, |ui| {
                for size in BoxSize::ALL {
                    ui.selectable_value(&mut desk.draft_size, size, size.label());
                }
            });
        ComboBox::from_id_salt("recv-status")
            .selected_text(desk.draft_receive.label())
            .show_ui(ui, |ui| {
                for status in ReceiveStatus::ALL {
                    ui.selectable_value(&mut desk.draft_receive, status, status.label());
                }
            });
        if ui.add(primary("Add")).clicked() {
            add = true;
        }
        });
    });
    if add {
        let id = desk.selected.clone();
        desk.add_receiving(&id);
    }

    ui.add_space(4.0);
    let mut remove_id = None;
    let mut pending_open = None;
    ScrollArea::vertical().show(ui, |ui| {
        for port in [Port::LosAngeles, Port::LongBeach] {
            ui.add_space(10.0);
            section_label(ui, port.label());
            ui.add_space(2.0);
            for item in terminals().iter().filter(|item| item.port == port) {
                let rows: Vec<Receiving> = desk
                    .receivings
                    .iter()
                    .filter(|row| {
                        row.date == desk.date
                            && row.shift == desk.shift
                            && row.terminal_id == item.id
                    })
                    .cloned()
                    .collect();
                let terminal_id = item.id.to_string();
                let name = item.name;
                let pier = item.pier;
                card()
                    .inner_margin(Margin::symmetric(12, 10))
                    .outer_margin(Margin::symmetric(0, 3))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(name).size(14.0).strong());
                            ui.label(RichText::new(pier).small().color(MUTED));
                            if rows.is_empty() {
                                ui.label(
                                    RichText::new("· nothing recorded for this shift")
                                        .small()
                                        .color(MUTED),
                                );
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button("Open browser").clicked() {
                                    pending_open = Some(terminal_id.clone());
                                }
                            });
                        });
                        if !rows.is_empty() {
                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                                for row in &rows {
                                    if receive_chip(ui, row) {
                                        remove_id = Some(row.id);
                                    }
                                }
                            });
                        }
                    });
            }
        }
        ui.add_space(12.0);
    });
    if let Some(id) = remove_id {
        desk.remove_receiving(id);
    }
    if let Some(id) = pending_open {
        let browser_id = terminal(&id)
            .and_then(|t| t.portals().first().map(|p| p.id))
            .unwrap_or("")
            .to_string();
        if desk.browser.get(&browser_id).is_some() {
            desk.focus_browser(&browser_id);
        } else if !browser_id.is_empty() {
            desk.open_browser(&browser_id);
        }
        desk.selected = id;
    }
}

fn footer(desk: &mut Desk, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui
            .add(primary("Copy proof"))
            .on_hover_text("Copy the proof packet for this container, date and shift")
            .clicked()
        {
            desk.copy_proof(ui);
            desk.status = "Proof packet copied.".to_string();
        }
        if ui.button("Copy full board").clicked() {
            desk.copy_board(ui);
            desk.status = "Full board copied.".to_string();
        }
        if ui.button("Copy receiving").clicked() {
            desk.copy_receiving(ui);
            desk.status = "Receiving list copied.".to_string();
        }
        if !desk.status.is_empty() {
            ui.add_space(8.0);
            dot(ui, ACCENT);
            ui.label(RichText::new(&desk.status).small().color(TEXT));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let full = desk.path.display().to_string();
            let short = desk
                .path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| full.clone());
            if ui
                .add(
                    egui::Label::new(RichText::new(format!("Saved to {short}")).small().color(MUTED))
                        .sense(Sense::click()),
                )
                .on_hover_text(format!("{full}\n\nClick to show it in Finder."))
                .clicked()
            {
                if let Some(folder) = desk.path.parent() {
                    open_path(&folder.display().to_string());
                }
            }
        });
    });
}

/// Empty the clipboard, but only if it still holds `secret` (the user may have
/// copied something else since).
fn clear_clipboard_if(secret: &str) {
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let current = Command::new("pbpaste")
            .output()
            .map(|out| out.stdout)
            .unwrap_or_default();
        if current == secret.as_bytes() {
            if let Ok(mut child) = Command::new("pbcopy").stdin(Stdio::piped()).spawn() {
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(b"");
                }
                let _ = child.wait();
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = secret;
}

fn open_link(ui: &egui::Ui, url: &str) {
    ui.ctx().open_url(egui::OpenUrl::new_tab(url));
}

fn open_path(path: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", path])
        .spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

fn receive_color(status: ReceiveStatus) -> Color32 {
    match status {
        ReceiveStatus::Receiving => OPEN,
        ReceiveStatus::DualOnly => DUAL,
        ReceiveStatus::NotReceiving => REFUSAL,
    }
}

fn finding_color(finding: Finding) -> Color32 {
    match finding {
        Finding::NotChecked => MUTED,
        Finding::Open => OPEN,
        Finding::DualOnly => DUAL,
        Finding::Closed | Finding::NoSlots => REFUSAL,
        Finding::CouldNotCheck => Color32::from_rgb(196, 132, 74),
    }
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

const AUTO_HELP: &str = "A few seconds after you sign in, the browser saves the page it lands on (and any extra pages set in .env). On APM and LBCT it then looks up empty-return appointments for the check list and saves a screenshot for each container. Files go to screenshots/<terminal>/.";
const ACCENT: Color32 = Color32::from_rgb(232, 176, 74);
const REFUSAL: Color32 = Color32::from_rgb(226, 98, 88);
const DUAL: Color32 = Color32::from_rgb(222, 172, 70);
const OPEN: Color32 = Color32::from_rgb(78, 184, 128);
const MUTED: Color32 = Color32::from_rgb(146, 153, 165);
const TEXT: Color32 = Color32::from_rgb(232, 234, 237);
const BG: Color32 = Color32::from_rgb(17, 19, 23);
const PANEL: Color32 = Color32::from_rgb(24, 27, 32);
const SURFACE: Color32 = Color32::from_rgb(31, 35, 42);
const SURFACE_HI: Color32 = Color32::from_rgb(42, 47, 56);
const BORDER: Color32 = Color32::from_rgb(46, 52, 62);
const ON_ACCENT: Color32 = Color32::from_rgb(24, 22, 18);

/// `color` mixed over the card surface, for tinted backgrounds.
fn tint(color: Color32, amount: f32) -> Color32 {
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
    Color32::from_rgb(
        mix(SURFACE.r(), color.r()),
        mix(SURFACE.g(), color.g()),
        mix(SURFACE.b(), color.b()),
    )
}

/// A rounded, bordered container used for every grouped block.
fn card() -> Frame {
    Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(10)
        .inner_margin(Margin::same(12))
}

/// Small upper-case heading above a group of controls.
fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .size(10.5)
            .strong()
            .color(MUTED),
    );
}

/// Page title with a muted one-line explanation under it.
fn page_title(ui: &mut egui::Ui, title: &str, blurb: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).size(20.0).strong());
    if !blurb.is_empty() {
        ui.label(RichText::new(blurb).color(MUTED));
    }
    ui.add_space(10.0);
}

/// A colored status badge, sized to its text.
fn pill(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        egui::FontId::proportional(11.5),
        color,
    );
    let size = galley.size() + Vec2::new(16.0, 6.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(9),
        tint(color, 0.16),
        Stroke::new(1.0, tint(color, 0.45)),
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}

/// The one main action in a group.
fn primary(text: &str) -> Button<'static> {
    Button::new(RichText::new(text.to_string()).strong().color(ON_ACCENT)).fill(ACCENT)
}

/// A filled colored dot, e.g. for browser state.
fn dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
}

#[cfg(test)]
mod tests {
    /// Touches the real clipboard (saved and restored). Run with --ignored.
    #[test]
    #[ignore]
    #[cfg(target_os = "macos")]
    fn clears_clipboard_only_if_unchanged() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let paste = || String::from_utf8(Command::new("pbpaste").output().unwrap().stdout).unwrap();
        let copy = |text: &str| {
            let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn().unwrap();
            child.stdin.as_mut().unwrap().write_all(text.as_bytes()).unwrap();
            child.wait().unwrap();
        };
        let saved = paste();

        copy("desk-test-secret");
        super::clear_clipboard_if("desk-test-secret");
        let cleared = paste();

        copy("something the user copied later");
        super::clear_clipboard_if("desk-test-secret");
        let kept = paste();

        copy(&saved);
        assert_eq!(cleared, "");
        assert_eq!(kept, "something the user copied later");
    }
}
