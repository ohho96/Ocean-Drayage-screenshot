mod browser;
mod credentials;
mod model;
mod store;

use std::collections::HashMap;

use browser::{now_secs, repo_root, BrowserManager, BrowserState, ImportBatch, ScheduleShot};
use eframe::egui::{
    self, Align, Button, Color32, ComboBox, CornerRadius, CursorIcon, Frame, Layout, Margin,
    RichText, ScrollArea, Sense, Stroke, TextEdit, Vec2,
};
use credentials::{credentials_dir, Login};
use model::{
    account_name, check_key, full_board, normalize_container, proof_packet, receiving_board, terminal, terminals,
    BoxSize, Check, Evidence, Finding, Port, ReceiveStatus, Receiving, Shift, Terminal, ACCOUNTS, LINES,
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
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(12.0, 6.0);
        style.spacing.window_margin = Margin::same(12);

        let visuals = &mut style.visuals;
        visuals.window_fill = Color32::from_rgb(22, 24, 28);
        visuals.panel_fill = Color32::from_rgb(28, 31, 37);
        visuals.extreme_bg_color = Color32::from_rgb(16, 18, 22);
        visuals.faint_bg_color = Color32::from_rgb(36, 40, 48);
        visuals.code_bg_color = Color32::from_rgb(16, 18, 22);
        visuals.override_text_color = Some(Color32::from_rgb(232, 234, 237));
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(52, 58, 68));
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 45, 54);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(50, 56, 68);
        visuals.widgets.active.bg_fill = Color32::from_rgb(58, 66, 80);
        visuals.selection.bg_fill = Color32::from_rgb(72, 90, 118);
        visuals.hyperlink_color = Color32::from_rgb(232, 184, 92);
        visuals.window_corner_radius = CornerRadius::same(8);
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
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
}

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
    login_dir: std::path::PathBuf,
    saved_logins: HashMap<String, SavedLogin>,
    login_drafts: HashMap<String, LoginDraft>,
}

impl Desk {
    fn new() -> Self {
        let mut desk = Self::build();
        desk.refresh_logins();
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
            .max(saved.receivings.iter().map(|row| row.id).max().unwrap_or(0) + 1);

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
            login_dir: credentials_dir(),
            saved_logins: HashMap::new(),
            login_drafts: HashMap::new(),
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
        }
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
        let Some(known) = terminal(&batch.terminal_id) else {
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

    fn shoot_schedule(&mut self) {
        if self.schedule_shot.is_some() {
            self.status = "A schedule screenshot is already running.".to_string();
            return;
        }
        match ScheduleShot::start(self.browser.root(), &self.selected) {
            Ok(job) => {
                self.status = "Taking a screenshot of the public empty schedule.".to_string();
                self.schedule_shot = Some(job);
            }
            Err(err) => self.status = err,
        }
    }

    fn poll_schedule_shot(&mut self) {
        let Some(job) = self.schedule_shot.as_ref() else {
            return;
        };
        let Some(outcome) = job.poll() else {
            return;
        };
        let terminal_id = job.terminal_id.clone();
        self.schedule_shot = None;
        match outcome {
            Ok(shot) => {
                let path = shot.screenshot;
                let count = shot.receiving.len();
                for row in shot.receiving {
                    self.upsert_receiving(
                        &row.date, row.shift, &terminal_id, row.line, row.size, row.status,
                    );
                }
                if count > 0 {
                    self.persist();
                }
                let rows = if count > 0 {
                    format!(" {count} receiving rows updated from the schedule.")
                } else {
                    String::new()
                };
                if self.selected == terminal_id {
                    self.draft_shot = path.clone();
                    self.status =
                        format!("Schedule saved to {path}.{rows} Set the finding and record it.");
                } else {
                    self.status = format!("Schedule saved to {path}.{rows}");
                }
            }
            Err(err) => self.status = err,
        }
    }

    fn copy_proof(&self, ui: &egui::Ui) {
        let text = proof_packet(
            &self.container,
            &self.date,
            self.shift,
            &timestamp(),
            &self.checks,
        );
        ui.ctx().copy_text(text);
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
        self.poll_schedule_shot();
        if self.browser.any_open() || self.schedule_shot.is_some() {
            // Keep polling the Python side even when the mouse is still.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(500));
        }
        for batch in self.browser.poll() {
            self.import_results(batch);
        }
        if let Some(message) = self.browser.take_notices().pop() {
            self.status = message;
        }
        egui::Panel::top("header").show(ui, |ui| header(self, ui));
        self.sync_draft();
        egui::Panel::bottom("footer").show(ui, |ui| footer(self, ui));
        egui::Panel::left("summary")
            .default_size(236.0)
            .resizable(false)
            .show(ui, |ui| summary(self, ui));

        if self.screen == Screen::Board {
            egui::Panel::right("detail")
                .default_size(360.0)
                .min_size(300.0)
                .show(ui, |ui| detail(self, ui));
            egui::CentralPanel::default().show(ui, |ui| board(self, ui));
        } else if self.screen == Screen::Receiving {
            egui::CentralPanel::default().show(ui, |ui| receiving(self, ui));
        } else if self.screen == Screen::Logins {
            egui::CentralPanel::default().show(ui, |ui| logins(self, ui));
        } else {
            egui::CentralPanel::default().show(ui, |ui| evidence(self, ui));
        }
    }
}

fn header(desk: &mut Desk, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.heading("Empty Return Desk");
        ui.label(
            RichText::new(
                "Which empties each terminal is taking, and proof when a return is refused",
            )
            .small()
            .color(MUTED),
        );
    });
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        screen_button(ui, &mut desk.screen, Screen::Board, "Board");
        screen_button(ui, &mut desk.screen, Screen::Receiving, "Receiving");
        let evidence_label = format!("Evidence ({})", desk.log.len());
        screen_button(ui, &mut desk.screen, Screen::Evidence, &evidence_label);
        let logins_label = format!("Logins ({}/{})", desk.saved_logins.len(), ACCOUNTS.len());
        screen_button(ui, &mut desk.screen, Screen::Logins, &logins_label);

        ui.separator();
        ui.label("Container");
        let before = desk.container.clone();
        ui.add(
            TextEdit::singleline(&mut desk.container)
                .desired_width(150.0)
                .hint_text("MSMU1234567"),
        );
        if desk.container != before {
            desk.persist();
        }

        ui.label("Date");
        if ui.button("◀").clicked() {
            desk.shift_date(-1);
        }
        ui.monospace(&desk.date);
        if ui.button("▶").clicked() {
            desk.shift_date(1);
        }
        if ui.button("Today").clicked() {
            desk.date = today();
        }

        let previous_shift = desk.shift;
        ComboBox::from_id_salt("shift")
            .selected_text(desk.shift.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut desk.shift, Shift::First, Shift::First.label());
                ui.selectable_value(&mut desk.shift, Shift::Second, Shift::Second.label());
            });
        if desk.shift != previous_shift {
            desk.persist();
        }
    });
    browser_list(desk, ui);
}

fn screen_button(ui: &mut egui::Ui, current: &mut Screen, screen: Screen, label: &str) {
    let selected = *current == screen;
    let button = if selected {
        Button::new(RichText::new(label).color(Color32::from_rgb(22, 24, 28))).fill(ACCENT)
    } else {
        Button::new(label)
    };
    if ui.add(button).clicked() {
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

    ui.add_space(6.0);
    Frame::new()
        .fill(Color32::from_rgb(34, 37, 44))
        .corner_radius(8)
        .inner_margin(8)
        .show(ui, |ui| {
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
            });
            egui::Grid::new("browser-list")
                .num_columns(5)
                .spacing(Vec2::new(12.0, 6.0))
                .show(ui, |ui| {
                    for slot in &desk.browser.slots {
                        let state = slot.state(now);
                        let known = terminal(&slot.terminal_id);
                        let name = known.map(|t| t.name).unwrap_or("Terminal");
                        let auto = known.map(|t| t.auto_check).unwrap_or(false);

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

    for (action, terminal_id) in actions {
        match action {
            BrowserAction::Show => desk.focus_browser(&terminal_id),
            BrowserAction::Check => desk.check_containers(&terminal_id),
            BrowserAction::Close => desk.close_browser(&terminal_id),
        }
    }
}

fn summary(desk: &mut Desk, ui: &mut egui::Ui) {
    ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(8.0);
        ui.label(RichText::new("THIS CHECK").small().color(MUTED).strong());
        let container = normalize_container(&desk.container);
        ui.label(
            RichText::new(if container.is_empty() {
                "No container".to_string()
            } else {
                container
            })
            .strong(),
        );
        ui.label(
            RichText::new(format!("{}  ·  {}", desk.date, desk.shift.label()))
                .small()
                .color(MUTED),
        );
        ui.add_space(12.0);

        let [no_return, dual, open, not_checked] = desk.tally();
        stat(ui, no_return, "No return", REFUSAL);
        ui.add_space(6.0);
        stat(ui, dual, "Dual only", DUAL);
        ui.add_space(6.0);
        stat(ui, open, "Open", OPEN);
        ui.add_space(6.0);
        stat(ui, not_checked, "Unchecked", MUTED);

        ui.add_space(16.0);
        ui.checkbox(&mut desk.problems_only, "Problems only");
        ui.label(
            RichText::new("Closed, no slots, or dual only.")
                .small()
                .color(MUTED),
        );

        ui.add_space(16.0);
        ui.label(
            RichText::new("CONTAINERS TO CHECK")
                .small()
                .color(MUTED)
                .strong(),
        );
        ui.label(
            RichText::new("One per line. Used when the browser checks appointments.")
                .small()
                .color(MUTED),
        );
        let before = desk.containers_text.clone();
        ui.add(
            TextEdit::multiline(&mut desk.containers_text)
                .desired_rows(6)
                .hint_text("MSMU1234567"),
        );
        if desk.containers_text != before {
            desk.persist();
        }
    });
}

fn stat(ui: &mut egui::Ui, value: usize, label: &str, color: Color32) {
    Frame::new()
        .fill(Color32::from_rgb(22, 24, 28))
        .corner_radius(8)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(value.to_string())
                    .size(28.0)
                    .color(color)
                    .strong(),
            );
            ui.label(RichText::new(label).small().color(MUTED));
        });
}

fn board(desk: &mut Desk, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Open the browser, sign in, then check the containers that have no return appointment.",
        )
        .small()
        .color(MUTED),
    );
    if normalize_container(&desk.container).is_empty() {
        ui.label(
            RichText::new("Enter a container number before you record a proof.").color(ACCENT),
        );
    }
    ui.add_space(8.0);

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
            ui.add_space(6.0);
            ui.label(RichText::new(port.label()).small().color(MUTED).strong());
            ui.add_space(4.0);
            for terminal in rows {
                terminal_row(desk, ui, terminal);
            }
        }
        if desk.problems_only && desk.tally()[0] + desk.tally()[1] == 0 {
            ui.add_space(8.0);
            ui.label("Nothing is recorded as unavailable for this container, date, and shift.");
        }
    });
}

fn terminal_row(desk: &mut Desk, ui: &mut egui::Ui, terminal: &Terminal) {
    let finding = desk.finding_for(terminal.id);
    let selected = desk.selected == terminal.id;
    let browser_state = desk
        .browser
        .get(terminal.id)
        .map(|slot| slot.state(now_secs()));
    let fill = if selected {
        Color32::from_rgb(46, 56, 72)
    } else {
        Color32::from_rgb(34, 38, 46)
    };
    let response = Frame::new()
        .fill(fill)
        .stroke(Stroke::new(
            1.0,
            if selected {
                Color32::from_rgb(90, 112, 140)
            } else {
                Color32::from_rgb(48, 54, 64)
            },
        ))
        .corner_radius(8)
        .inner_margin(10)
        .outer_margin(Margin::symmetric(0, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(6.0, 32.0), Sense::hover());
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(3), finding_color(finding));
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(terminal.name).strong());
                        ui.label(RichText::new(terminal.pier).small().color(MUTED));
                        if let Some(state) = browser_state {
                            ui.label(
                                RichText::new(format!("● {}", state.label()))
                                    .small()
                                    .color(state_color(state)),
                            );
                        }
                    });
                    ui.label(RichText::new(terminal.where_to_look()).small().color(MUTED));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(finding.label()).color(finding_color(finding)));
                });
            });
        })
        .response;

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if response.hovered() && ui.input(|input| input.pointer.primary_clicked()) {
        desk.selected = terminal.id.to_string();
    }
}

fn detail(desk: &mut Desk, ui: &mut egui::Ui) {
    let Some(terminal) = terminal(&desk.selected) else {
        ui.label("Select a terminal.");
        return;
    };
    let terminal_id = terminal.id;
    let name = terminal.name;
    let pier = terminal.pier;
    let port = terminal.port.label();
    let look = terminal.where_to_look();
    let empty_url = terminal.empty_url;
    let appt_url = terminal.appt_url;
    let appt_system = terminal.appt_system;
    let guidance = if terminal.auto_check {
        "Open the browser and sign in. Then check containers. A screenshot is saved when no return appointment is offered, and the finding is recorded here."
    } else if empty_url.is_some() {
        "Open the browser and sign in, then record the finding. The public empty schedule is the other place to see whether this line is being received."
    } else {
        "This terminal does not post a public empty list. Open the browser, sign in, try an empty return, and record that no slot is offered."
    };
    let account = terminal.account();
    let login_line = if desk.saved_logins.contains_key(account) {
        format!(
            "Saved {} login will be filled in on the sign-in page. Solve any captcha and press Sign in yourself.",
            account_name(account)
        )
    } else {
        format!(
            "No saved {} login. Sign in by hand, or add it under Logins.",
            account_name(account)
        )
    };

    ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(name).size(22.0).strong());
        ui.label(RichText::new(format!("{pier}  ·  {port}")).color(MUTED));
        ui.add_space(6.0);
        ui.label(RichText::new(look).small().color(MUTED));
        ui.add_space(8.0);
        ui.label(guidance);
        ui.add_space(6.0);
        ui.label(RichText::new(login_line).small().color(MUTED));
        ui.add_space(12.0);

        let open_state = desk
            .browser
            .get(terminal_id)
            .map(|slot| slot.state(now_secs()));
        if let Some(state) = open_state {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Browser:").small().color(MUTED));
                ui.label(RichText::new(state.label()).color(state_color(state)));
            });
            if ui.button(format!("Show {appt_system} browser")).clicked() {
                desk.focus_browser(terminal_id);
            }
        } else {
            let open_browser = Button::new(
                RichText::new(format!("Open browser — {appt_system}"))
                    .color(Color32::from_rgb(22, 24, 28)),
            )
            .fill(ACCENT);
            if ui.add(open_browser).clicked() {
                desk.open_browser(terminal_id);
            }
        }

        if let Some(url) = empty_url {
            if ui.button("Open empty schedule").clicked() {
                open_link(ui, url);
            }
            let busy = desk.schedule_shot.is_some();
            let label = if busy {
                "Taking schedule screenshot…"
            } else {
                "Screenshot empty schedule"
            };
            if ui.add_enabled(!busy, Button::new(label)).clicked() {
                desk.shoot_schedule();
            }
        } else {
            ui.label(
                RichText::new("No public empty schedule")
                    .small()
                    .color(MUTED),
            );
        }
        if ui
            .button(format!("Open {appt_system} in this computer's browser"))
            .clicked()
        {
            open_link(ui, appt_url);
        }
        ui.add_space(4.0);
        ui.label(RichText::new(appt_url).small().color(MUTED));

        ui.add_space(16.0);
        ui.label(RichText::new("FINDING").small().color(MUTED).strong());
        ComboBox::from_id_salt(format!("finding-{terminal_id}"))
            .selected_text(desk.draft_finding.label())
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for finding in Finding::ALL {
                    ui.selectable_value(&mut desk.draft_finding, finding, finding.label());
                }
            });

        ui.add_space(8.0);
        ui.label(RichText::new("NOTE").small().color(MUTED).strong());
        ui.add(
            TextEdit::multiline(&mut desk.draft_note)
                .desired_rows(4)
                .hint_text("Line, size, and what the page showed"),
        );

        ui.add_space(8.0);
        ui.label(
            RichText::new("SCREENSHOT FILE")
                .small()
                .color(MUTED)
                .strong(),
        );
        ui.add(TextEdit::singleline(&mut desk.draft_shot).hint_text("Path to the image, optional"));

        ui.add_space(12.0);
        let can_record = desk.draft_finding != Finding::NotChecked
            && !normalize_container(&desk.container).is_empty();
        let record =
            Button::new(RichText::new("Record proof").color(Color32::from_rgb(22, 24, 28)))
                .fill(ACCENT);
        if ui.add_enabled(can_record, record).clicked() {
            desk.record();
        }
        if !can_record {
            ui.label(
                RichText::new("A container number and a finding are required.")
                    .small()
                    .color(MUTED),
            );
        }
    });
}

enum LoginAction {
    Save(String),
    Delete(String),
}

fn logins(desk: &mut Desk, ui: &mut egui::Ui) {
    let mut action: Option<LoginAction> = None;
    let folder = desk.login_dir.clone();
    ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(8.0);
        ui.label(RichText::new("Logins").size(22.0).strong());
        ui.label(
            RichText::new(
                "When a browser shows a sign-in page, it fills in the saved ID and password. \
                 You solve any captcha and press Sign in yourself. Nothing is submitted for you.",
            )
            .color(MUTED),
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
            let used_by: Vec<&str> = terminals()
                .iter()
                .filter(|terminal| terminal.account() == account)
                .map(|terminal| terminal.name)
                .collect();
            let saved = desk.saved_logins.get(account);
            let saved_line = saved.map(|login| {
                format!("Saved · ID {} · updated {}", login.user_id, login.updated_at)
            });
            let draft = desk.login_drafts.entry(account.to_string()).or_default();

            Frame::new()
                .fill(Color32::from_rgb(34, 38, 46))
                .stroke(Stroke::new(1.0, Color32::from_rgb(48, 54, 64)))
                .corner_radius(8)
                .inner_margin(10)
                .outer_margin(Margin::symmetric(0, 4))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(name).strong());
                        match &saved_line {
                            Some(line) => ui.label(RichText::new(line).small().color(OPEN)),
                            None => ui.label(RichText::new("Not saved").small().color(MUTED)),
                        };
                    });
                    ui.label(
                        RichText::new(format!("Used by {}", used_by.join(", ")))
                            .small()
                            .color(MUTED),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("ID");
                        ui.add(
                            TextEdit::singleline(&mut draft.user_id)
                                .desired_width(200.0)
                                .hint_text("Login ID"),
                        );
                        ui.label("Password");
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
        None => {}
    }
}

fn evidence(desk: &mut Desk, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Proof log").strong());
        ui.checkbox(&mut desk.this_container_only, "This container only");
    });
    ui.label(
        RichText::new("Each record is kept. Recording again adds a new line; the board shows the latest finding.")
            .small()
            .color(MUTED),
    );
    ui.add_space(8.0);

    let container = normalize_container(&desk.container);
    let mut rows: Vec<&Evidence> = desk
        .log
        .iter()
        .rev()
        .filter(|entry| !desk.this_container_only || entry.container == container)
        .collect();
    if rows.is_empty() {
        ui.label("No proof recorded yet. On the board, open a terminal and record what you found.");
        return;
    }

    ScrollArea::vertical().show(ui, |ui| {
        for entry in rows.drain(..) {
            Frame::new()
                .fill(Color32::from_rgb(34, 38, 46))
                .corner_radius(8)
                .inner_margin(10)
                .outer_margin(Margin::symmetric(0, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&entry.terminal_name).strong());
                        ui.label(RichText::new(&entry.container).monospace());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(entry.finding.label())
                                    .color(finding_color(entry.finding)),
                            );
                        });
                    });
                    ui.label(
                        RichText::new(format!(
                            "{}  ·  {}  ·  {}",
                            entry.at,
                            entry.date,
                            entry.shift.label()
                        ))
                        .small()
                        .color(MUTED),
                    );
                    if !entry.note.is_empty() {
                        ui.label(&entry.note);
                    }
                    if !entry.screenshot.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&entry.screenshot)
                                    .small()
                                    .monospace()
                                    .color(MUTED),
                            );
                            if ui.button("Open").clicked() {
                                open_path(&entry.screenshot);
                            }
                        });
                    }
                });
        }
    });
}

fn receiving(desk: &mut Desk, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.label(RichText::new("Receiving locations").strong());
    ui.label(
        RichText::new("Each terminal, and which empties it is taking on this date and shift.")
            .small()
            .color(MUTED),
    );
    ui.add_space(8.0);

    let mut add = false;
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
        if ui.button("Add").clicked() {
            add = true;
        }
    });
    if add {
        let id = desk.selected.clone();
        desk.add_receiving(&id);
    }

    ui.add_space(8.0);
    let mut remove_id = None;
    let mut pending_open = None;
    ScrollArea::vertical().show(ui, |ui| {
        for port in [Port::LosAngeles, Port::LongBeach] {
            ui.add_space(6.0);
            ui.label(RichText::new(port.label()).small().color(MUTED).strong());
            ui.add_space(4.0);
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
                Frame::new()
                    .fill(Color32::from_rgb(34, 38, 46))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(48, 54, 64)))
                    .corner_radius(8)
                    .inner_margin(10)
                    .outer_margin(Margin::symmetric(0, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(name).strong());
                            ui.label(RichText::new(pier).small().color(MUTED));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button("Open browser").clicked() {
                                    pending_open = Some(terminal_id.clone());
                                }
                            });
                        });
                        if rows.is_empty() {
                            ui.label(
                                RichText::new("Nothing recorded for this shift.")
                                    .small()
                                    .color(MUTED),
                            );
                        }
                        for row in &rows {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&row.line).strong());
                                ui.label(row.size.label());
                                ui.label(
                                    RichText::new(row.status.label())
                                        .color(receive_color(row.status)),
                                );
                                if ui.button("Remove").clicked() {
                                    remove_id = Some(row.id);
                                }
                            });
                        }
                    });
            }
        }
    });
    if let Some(id) = remove_id {
        desk.remove_receiving(id);
    }
    if let Some(id) = pending_open {
        if desk.browser.get(&id).is_some() {
            desk.focus_browser(&id);
        } else {
            desk.open_browser(&id);
        }
        desk.selected = id;
    }
}

fn footer(desk: &mut Desk, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let proof = Button::new(RichText::new("Copy proof").color(Color32::from_rgb(22, 24, 28)))
            .fill(ACCENT);
        if ui.add(proof).clicked() {
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
        ui.separator();
        ui.label(RichText::new(&desk.status).small().color(MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(desk.path.display().to_string())
                    .small()
                    .color(MUTED),
            );
        });
    });
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

const ACCENT: Color32 = Color32::from_rgb(232, 176, 74);
const REFUSAL: Color32 = Color32::from_rgb(214, 92, 82);
const DUAL: Color32 = Color32::from_rgb(212, 164, 64);
const OPEN: Color32 = Color32::from_rgb(72, 168, 118);
const MUTED: Color32 = Color32::from_rgb(154, 160, 170);
