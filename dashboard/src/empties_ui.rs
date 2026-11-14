//! The Empties screen: the day's list of empty containers, each with its own
//! SSL, size and type, and where each one can be returned.

use super::*;
use model::{remove_box, upsert_box, EmptyBox, BOX_KINDS, BOX_SIZES, MANUAL_SOURCE};

/// Which rows the container table shows.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum BoxView {
    #[default]
    All,
    Single,
    Dual,
    NoTerminal,
    Special,
}

/// Where one container stands on the receiving board.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ReturnState {
    Single,
    Dual,
    NoTerminal,
    Special,
}

impl ReturnState {
    fn view(self) -> BoxView {
        match self {
            Self::Single => BoxView::Single,
            Self::Dual => BoxView::Dual,
            Self::NoTerminal => BoxView::NoTerminal,
            Self::Special => BoxView::Special,
        }
    }
}

/// Edit state for the container list, kept on the desk between frames.
#[derive(Default)]
pub struct BoxForm {
    /// The "Add container" row.
    pub draft: EmptyBox,
    /// The row being edited, by container number, and its edited copy.
    pub editing: Option<EmptyBox>,
    pub search: String,
    pub line_filter: String,
    pub view: BoxView,
    pub confirm_clear: bool,
    pub focus_input: bool,
    /// Bulk mode: many containers at once.
    pub bulk: bool,
    pub bulk_text: String,
    /// SSL (typed as SCAC or name), size and type for bulk lines that leave them out.
    pub bulk_defaults: EmptyBox,
}

enum Action {
    ChooseFile,
    Paste,
    RemoveUpload(u64),
    UseForChecks,
    CopyPlan,
    Capture,
    Save(EmptyBox),
    SaveMany(Vec<EmptyBox>),
    Remove(String),
    ClearDay,
    SetCurrent(String),
    SetChecked(Vec<String>, bool),
    /// Return locations for a line (terminal ids); None goes back to the receiving board.
    SetLocations(String, Option<Vec<String>>),
}

struct Row {
    item: EmptyBox,
    state: ReturnState,
    summary: String,
    hover: String,
    /// Carrier screenshot windows covered for this box (waiver date = this day).
    cover: proof::Coverage,
}

pub fn empties(desk: &mut Desk, ui: &mut egui::Ui) {
    let mut actions: Vec<Action> = Vec::new();
    let date = desk.date.clone();
    let shift = desk.shift;
    let boxes = boxes_for_day(&desk.uploads, &date);
    let rows: Vec<Row> = boxes
        .iter()
        .map(|item| {
            let (locations, guessed) =
                proof::locations_for(&item.line, &desk.return_locations, &desk.receivings);
            let cover = proof::coverage(item, &date, locations, guessed, &desk.log);
            describe(item, &date, shift, &desk.receivings, cover)
        })
        .collect();
    let count = |state: ReturnState| rows.iter().filter(|row| row.state == state).count();
    let (single, dual, none, special) = (
        count(ReturnState::Single),
        count(ReturnState::Dual),
        count(ReturnState::NoTerminal),
        count(ReturnState::Special),
    );

    ScrollArea::vertical().show(ui, |ui| {
        page_title(
            ui,
            &format!("Empties for {date}"),
            "Each empty that needs a return appointment, with its SSL, size and type, and where it can go back.",
        );

        add_row(desk, ui, &boxes, &mut actions);
        ui.add_space(8.0);
        import_section(desk, ui, &date, boxes.is_empty(), &mut actions);
        ui.add_space(14.0);
        capture_line(desk, ui, shift, &mut actions);
        ui.add_space(6.0);

        if boxes.is_empty() {
            empty_state(
                ui,
                "No empties for this date yet",
                "Add containers one at a time above, or import a list from a file or Excel.",
            );
            return;
        }

        // Filter chips double as the day's summary.
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let view = &mut desk.box_form.view;
            view_chip(ui, view, BoxView::All, &format!("All {}", rows.len()), TEXT);
            view_chip(ui, view, BoxView::Single, &format!("Single {single}"), OPEN);
            view_chip(ui, view, BoxView::Dual, &format!("Dual only {dual}"), DUAL);
            view_chip(ui, view, BoxView::NoTerminal, &format!("No terminal {none}"), REFUSAL);
            if special > 0 {
                view_chip(ui, view, BoxView::Special, &format!("Special {special}"), MUTED);
            }
        });
        ui.add_space(6.0);

        proof_section(desk, ui, &date, &rows, &mut actions);
        ui.add_space(8.0);

        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            toolbar(desk, ui, &boxes, &mut actions);
            ui.add_space(6.0);
            table(desk, ui, &rows, &mut actions);
        });
    });

    for action in actions {
        apply(desk, ui, action, &date, shift, &boxes);
    }
}

fn describe(
    item: &EmptyBox,
    date: &str,
    shift: Shift,
    receivings: &[Receiving],
    cover: proof::Coverage,
) -> Row {
    let options = return_options(item, date, shift, receivings);
    let total = terminals().len();
    let (state, summary) = if !options.single.is_empty() {
        (ReturnState::Single, options.single.join(", "))
    } else if !options.dual.is_empty() {
        (ReturnState::Dual, format!("Dual only: {}", options.dual.join(", ")))
    } else if item.board_size().is_none() {
        (ReturnState::Special, "Special equipment — check by hand".to_string())
    } else {
        let known = total - options.unknown.len();
        (
            ReturnState::NoTerminal,
            format!("No terminal receiving ({known} of {total} known)"),
        )
    };
    let list = |names: &[&str]| if names.is_empty() { "—".to_string() } else { names.join(", ") };
    let hover = format!(
        "Receiving: {}\nDual only: {}\nNot receiving: {}\nNo information: {}",
        list(&options.single),
        list(&options.dual),
        list(&options.closed),
        list(&options.unknown),
    );
    Row {
        item: item.clone(),
        state,
        summary,
        hover,
        cover,
    }
}

fn state_color(state: ReturnState) -> Color32 {
    match state {
        ReturnState::Single => OPEN,
        ReturnState::Dual => DUAL,
        ReturnState::NoTerminal => REFUSAL,
        ReturnState::Special => MUTED,
    }
}

/// "Add containers": one at a time, or many at once in bulk.
fn add_row(desk: &mut Desk, ui: &mut egui::Ui, boxes: &[EmptyBox], actions: &mut Vec<Action>) {
    let form = &mut desk.box_form;
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            section_label(ui, "Add containers");
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            for (bulk, label) in [(false, "One"), (true, "Bulk")] {
                let chosen = form.bulk == bulk;
                let button = Button::new(
                    RichText::new(label)
                        .size(12.0)
                        .strong()
                        .color(if chosen { ACCENT } else { MUTED }),
                )
                .fill(if chosen { tint(ACCENT, 0.18) } else { SURFACE })
                .stroke(Stroke::new(1.0, if chosen { tint(ACCENT, 0.6) } else { BORDER }));
                if ui.add(button).clicked() {
                    form.bulk = bulk;
                }
            }
        });
        ui.add_space(4.0);
        if form.bulk {
            bulk_add(form, ui, boxes, actions);
        } else {
            single_add(form, ui, boxes, actions);
        }
    });
}

fn single_add(form: &mut BoxForm, ui: &mut egui::Ui, boxes: &[EmptyBox], actions: &mut Vec<Action>) {
    let parsed = upload::as_container(&form.draft.container);
    let existing = parsed
        .as_ref()
        .and_then(|number| boxes.iter().find(|b| &b.container == number));
    let mut submit = false;
    ui.horizontal_wrapped(|ui| {
        let before = form.draft.container.clone();
        let field = ui.add(
            TextEdit::singleline(&mut form.draft.container)
                .desired_width(150.0)
                .font(egui::TextStyle::Monospace)
                .char_limit(16)
                .hint_text("MSCU1234567"),
        );
        if form.focus_input {
            field.request_focus();
            form.focus_input = false;
        }
        if form.draft.container != before {
            // Fill in the SCAC from the owner code (MSCU → MSC) when it is blank.
            if let Some(number) = upload::as_container(&form.draft.container) {
                if form.draft.line.trim().is_empty() {
                    if let Some(name) = upload::normalize_line(&number[..4]) {
                        form.draft.line = upload::scac_for(name).unwrap_or(name).to_string();
                    }
                }
            }
        }
        if field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            submit = true;
        }

        ssl_field(ui, "add-ssl", &mut form.draft.line);
        size_picker(ui, &mut form.draft.size);
        kind_picker(ui, "add-kind", &mut form.draft.kind);

        let label = if existing.is_some() { "Update" } else { "Add" };
        if ui.add_enabled(parsed.is_some(), primary(label)).clicked() {
            submit = true;
        }
    });

    let hint = match (&parsed, existing) {
        (None, _) if form.draft.container.trim().is_empty() => {
            "Type a container number, its SCAC (3 or 4 letters, e.g. MSC or MSCU), size and type, then press Enter. Use Bulk for many at once.".to_string()
        }
        (None, _) => "A container number is 4 letters then 7 digits, e.g. MSCU1234567.".to_string(),
        (Some(_), Some(old)) => format!(
            "Already in today's list as {} {} — saving updates it.",
            if old.line.is_empty() { "?" } else { &old.line },
            old.size_label()
        ),
        (Some(_), None) => "Press Enter or Add.".to_string(),
    };
    ui.label(RichText::new(hint).small().color(MUTED));

    if submit {
        if let Some(number) = parsed {
            let mut item = form.draft.clone();
            item.container = number;
            actions.push(Action::Save(item));
            form.draft.container.clear();
            form.focus_input = true;
        }
    }
}

fn bulk_add(form: &mut BoxForm, ui: &mut egui::Ui, boxes: &[EmptyBox], actions: &mut Vec<Action>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Defaults").small().color(MUTED))
            .on_hover_text("Used for any line that doesn't give its own SCAC, size or type");
        ssl_field(ui, "bulk-ssl", &mut form.bulk_defaults.line);
        size_picker(ui, &mut form.bulk_defaults.size);
        kind_picker(ui, "bulk-kind", &mut form.bulk_defaults.kind);
    });
    ui.add_space(4.0);
    ui.add(
        TextEdit::multiline(&mut form.bulk_text)
            .desired_rows(6)
            .desired_width(f32::INFINITY)
            .font(egui::TextStyle::Monospace)
            .hint_text(
                "Paste container numbers — one per line, or separated by spaces or commas.\n\
                 A line can add its own SCAC and size/type:\n\
                 MSCU1234567\n\
                 MSCU7654321, MSCU1112223\n\
                 ONEU1029384 ONEY 40HC\n\
                 HDMU2233445 HMM 45",
            ),
    );

    let bulk = upload::parse_bulk(&form.bulk_text, &form.bulk_defaults);
    if form.bulk_text.trim().is_empty() {
        ui.label(
            RichText::new("SCAC can be 3 or 4 letters (MSC / MSCU, ONE / ONEY, HMM / HDMU). Without one, the container's owner code is used.")
                .small()
                .color(MUTED),
        );
        return;
    }

    let updates = bulk
        .rows
        .iter()
        .filter(|row| boxes.iter().any(|b| b.container == row.item.container))
        .count();
    let new = bulk.rows.len() - updates;
    let unknown = bulk.rows.iter().filter(|row| row.unknown_line).count();
    let no_ssl = bulk.rows.iter().filter(|row| row.item.line.is_empty()).count();
    let no_size = bulk.rows.iter().filter(|row| row.item.size.is_empty()).count();

    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(RichText::new(format!("{} containers", bulk.rows.len())).strong());
        if new > 0 {
            pill(ui, &format!("{new} new"), OPEN);
        }
        if updates > 0 {
            pill(ui, &format!("{updates} update"), ACCENT)
                .on_hover_text("Already in today's list; adding updates their SSL, size and type");
        }
        if unknown > 0 {
            pill(ui, &format!("{unknown} unknown SCAC"), DUAL);
        }
        if no_ssl > 0 {
            pill(ui, &format!("{no_ssl} no SSL"), DUAL);
        }
        if no_size > 0 {
            pill(ui, &format!("{no_size} no size"), DUAL);
        }
        if !bulk.skipped.is_empty() {
            pill(ui, &format!("{} skipped", bulk.skipped.len()), REFUSAL);
        }
    });
    for note in bulk.skipped.iter().take(6) {
        ui.label(RichText::new(format!("⚠ {note}")).small().color(DUAL));
    }
    if bulk.skipped.len() > 6 {
        ui.label(RichText::new(format!("…and {} more", bulk.skipped.len() - 6)).small().color(DUAL));
    }

    if !bulk.rows.is_empty() {
        ui.add_space(4.0);
        const PREVIEW: usize = 10;
        egui::Grid::new("bulk-preview")
            .num_columns(5)
            .striped(true)
            .spacing(Vec2::new(16.0, 4.0))
            .show(ui, |ui| {
                for head in ["Container", "SSL", "Size", "Type", ""] {
                    section_label(ui, head);
                }
                ui.end_row();
                for row in bulk.rows.iter().take(PREVIEW) {
                    let item = &row.item;
                    ui.label(RichText::new(&item.container).monospace());
                    ssl_label(ui, &item.line, row.unknown_line);
                    if item.size.is_empty() {
                        ui.label(RichText::new("?").color(DUAL));
                    } else {
                        ui.label(format!("{}'", item.size));
                    }
                    ui.label(kind_name(&item.kind));
                    let update = boxes.iter().any(|b| b.container == item.container);
                    ui.horizontal(|ui| {
                        if update {
                            ui.label(RichText::new("updates").small().color(ACCENT));
                        }
                        if row.repeated {
                            ui.label(RichText::new("listed twice, last kept").small().color(MUTED));
                        }
                    });
                    ui.end_row();
                }
            });
        if bulk.rows.len() > PREVIEW {
            ui.label(
                RichText::new(format!("…and {} more", bulk.rows.len() - PREVIEW))
                    .small()
                    .color(MUTED),
            );
        }
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let label = match bulk.rows.len() {
            1 => "Add 1 container".to_string(),
            n => format!("Add {n} containers"),
        };
        if ui.add_enabled(!bulk.rows.is_empty(), primary(&label)).clicked() {
            actions.push(Action::SaveMany(bulk.rows.iter().map(|row| row.item.clone()).collect()));
            form.bulk_text.clear();
        }
        if ui.button("Clear").clicked() {
            form.bulk_text.clear();
        }
    });
}

/// SSL typed as a SCAC (3 or 4 letters) or a line name, with a list to pick
/// from, and what it resolves to.
fn ssl_field(ui: &mut egui::Ui, id: &str, text: &mut String) {
    ui.add(
        TextEdit::singleline(text)
            .desired_width(64.0)
            .char_limit(24)
            .font(egui::TextStyle::Monospace)
            .hint_text("SCAC"),
    );
    ComboBox::from_id_salt(id)
        .selected_text("")
        .width(14.0)
        .height(340.0)
        .show_ui(ui, |ui| {
            for name in LINES {
                let scac = upload::scac_for(name).unwrap_or("");
                if ui
                    .selectable_label(false, RichText::new(format!("{scac:<5} {name}")).monospace())
                    .clicked()
                {
                    *text = if scac.is_empty() { name.to_string() } else { scac.to_string() };
                }
            }
        });
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        let (name, known) = upload::resolve_line(trimmed);
        if !known {
            ui.label(RichText::new("unknown SCAC").small().color(DUAL))
                .on_hover_text("Kept as typed. Pick from the list if it is one of the known lines.");
        } else if !name.eq_ignore_ascii_case(trimmed) {
            ui.label(RichText::new(name).small().color(OPEN));
        }
    }
}

/// Line name with its SCAC in muted text, e.g. "MSC  MSCU".
fn ssl_label(ui: &mut egui::Ui, line: &str, unknown: bool) {
    if line.is_empty() {
        ui.label(RichText::new("SSL?").color(DUAL))
            .on_hover_text("No SSL — edit to set it");
        return;
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.label(RichText::new(line).color(if unknown { DUAL } else { TEXT }));
        match upload::scac_for(line) {
            Some(scac) if !scac.eq_ignore_ascii_case(line) => {
                ui.label(RichText::new(scac).small().monospace().color(MUTED));
            }
            None if unknown => {
                ui.label(RichText::new("unknown").small().color(DUAL));
            }
            _ => {}
        }
    });
}

/// File, drop and paste import, folded away once the day has containers.
fn import_section(
    desk: &mut Desk,
    ui: &mut egui::Ui,
    date: &str,
    open: bool,
    actions: &mut Vec<Action>,
) {
    let day_uploads: Vec<(u64, String, String, usize)> = desk
        .uploads
        .iter()
        .filter(|u| u.date == date)
        .map(|u| (u.id, u.uploaded_at.clone(), u.source.clone(), u.boxes.len()))
        .collect();
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::CollapsingHeader::new(
            RichText::new("IMPORT A LIST").size(10.5).strong().color(MUTED),
        )
        .id_salt(("import", date))
        .default_open(open)
        .show(ui, |ui| {
            ui.label(
                RichText::new(
                    "Drop a CSV or Excel file on this window, choose a file, or paste rows from Excel. \
                     Columns: container, SSL, size, type (headers optional).",
                )
                .small()
                .color(MUTED),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.add(primary("Choose file…")).clicked() {
                    actions.push(Action::ChooseFile);
                }
                ui.label(RichText::new("or drop a .csv / .xlsx anywhere").small().color(MUTED));
            });
            ui.add_space(4.0);
            ui.add(
                TextEdit::multiline(&mut desk.paste_text)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace)
                    .hint_text("Paste rows here, e.g.  YMLU1234567  YML  40  HC"),
            );
            if ui
                .add_enabled(!desk.paste_text.trim().is_empty(), Button::new("Add pasted list"))
                .clicked()
            {
                actions.push(Action::Paste);
            }

            if !day_uploads.is_empty() {
                ui.add_space(8.0);
                section_label(ui, "Lists for this date");
                for (id, at, source, n) in &day_uploads {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(at).monospace().color(MUTED));
                        ui.label(if source == MANUAL_SOURCE { "Added by hand" } else { source });
                        ui.label(RichText::new(format!("{n} empties")).color(MUTED));
                        if ui
                            .small_button("Remove list")
                            .on_hover_text("Remove this whole list from the date")
                            .clicked()
                        {
                            actions.push(Action::RemoveUpload(*id));
                        }
                    });
                }
            }
            let other_days: std::collections::BTreeSet<&str> = desk
                .uploads
                .iter()
                .filter(|u| u.date != date)
                .map(|u| u.date.as_str())
                .collect();
            if !other_days.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "Other days with empties: {} (use the date arrows)",
                        other_days.into_iter().rev().take(10).collect::<Vec<_>>().join(", ")
                    ))
                    .small()
                    .color(MUTED),
                );
            }
        });

        if !desk.upload_warnings.is_empty() {
            ui.add_space(4.0);
            for warning in desk.upload_warnings.iter().take(12) {
                ui.label(RichText::new(format!("⚠ {warning}")).small().color(DUAL));
            }
            if desk.upload_warnings.len() > 12 {
                ui.label(
                    RichText::new(format!("…and {} more", desk.upload_warnings.len() - 12))
                        .small()
                        .color(DUAL),
                );
            }
        }
    });
}

fn capture_line(desk: &mut Desk, ui: &mut egui::Ui, shift: Shift, actions: &mut Vec<Action>) {
    ui.horizontal_wrapped(|ui| {
        section_label(ui, &format!("Where to return · {}", shift.label()));
        let captured: Vec<String> = SCHEDULE_READERS
            .iter()
            .map(|id| {
                let name = terminal(id).map(|t| t.name).unwrap_or(id);
                match desk.last_capture.get(*id) {
                    Some((_, at)) => format!("{name} {at}"),
                    None => format!("{name} not yet"),
                }
            })
            .collect();
        ui.label(
            RichText::new(format!("Receiving captured: {}", captured.join(" · ")))
                .small()
                .color(MUTED),
        );
        let busy = desk.schedule_shot.is_some() || !desk.schedule_queue.is_empty();
        if ui
            .add_enabled(!busy, Button::new(if busy { "Capturing…" } else { "Capture receiving now" }))
            .on_hover_text("Read ITS and TraPac public schedules into the receiving board. Also runs when a browser signs in.")
            .clicked()
        {
            actions.push(Action::Capture);
        }
    });
}

fn view_chip(ui: &mut egui::Ui, view: &mut BoxView, this: BoxView, label: &str, color: Color32) {
    let chosen = *view == this;
    let button = Button::new(
        RichText::new(label)
            .size(12.0)
            .strong()
            .color(if chosen { color } else { MUTED }),
    )
    .fill(if chosen { tint(color, 0.18) } else { SURFACE })
    .stroke(Stroke::new(1.0, if chosen { tint(color, 0.6) } else { BORDER }))
    .corner_radius(12);
    if ui.add(button).clicked() {
        *view = this;
    }
}

/// Search, SSL filter, and the list-wide actions.
fn toolbar(desk: &mut Desk, ui: &mut egui::Ui, boxes: &[EmptyBox], actions: &mut Vec<Action>) {
    let mut lines: Vec<String> = boxes
        .iter()
        .map(|b| b.line.clone())
        .filter(|line| !line.is_empty())
        .collect();
    lines.sort();
    lines.dedup();
    let form = &mut desk.box_form;
    ui.horizontal_wrapped(|ui| {
        ui.add(
            TextEdit::singleline(&mut form.search)
                .desired_width(180.0)
                .hint_text("Search container or SSL"),
        );
        let label = if form.line_filter.is_empty() {
            "All SSLs".to_string()
        } else {
            form.line_filter.clone()
        };
        ComboBox::from_id_salt("box-line-filter")
            .selected_text(label)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut form.line_filter, String::new(), "All SSLs");
                for line in &lines {
                    ui.selectable_value(&mut form.line_filter, line.clone(), line);
                }
            });
        if !form.search.is_empty() || !form.line_filter.is_empty() || form.view != BoxView::All {
            if ui.small_button("Clear filters").clicked() {
                form.search.clear();
                form.line_filter.clear();
                form.view = BoxView::All;
            }
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if form.confirm_clear {
                if ui.small_button("Cancel").clicked() {
                    form.confirm_clear = false;
                }
                if ui
                    .button(RichText::new(format!("Remove all {}", boxes.len())).color(REFUSAL))
                    .clicked()
                {
                    actions.push(Action::ClearDay);
                }
            } else if ui
                .button("Clear day")
                .on_hover_text("Remove every container for this date")
                .clicked()
            {
                form.confirm_clear = true;
            }
            if ui
                .button("Use as check list")
                .on_hover_text("Replace the APM/LBCT container check list with today's containers")
                .clicked()
            {
                actions.push(Action::UseForChecks);
            }
            if ui.add(primary("Copy return plan")).clicked() {
                actions.push(Action::CopyPlan);
            }
        });
    });
}

fn table(desk: &mut Desk, ui: &mut egui::Ui, rows: &[Row], actions: &mut Vec<Action>) {
    let checked: std::collections::HashSet<String> = desk.container_list().into_iter().collect();
    let current = normalize_container(&desk.container);
    let desk_disputes = desk.disputes.clone();
    let form = &mut desk.box_form;
    let search = form.search.trim().to_uppercase();
    let visible: Vec<&Row> = rows
        .iter()
        .filter(|row| form.view == BoxView::All || row.state.view() == form.view)
        .filter(|row| form.line_filter.is_empty() || row.item.line == form.line_filter)
        .filter(|row| {
            search.is_empty()
                || row.item.container.contains(&search)
                || row.item.line.to_uppercase().contains(&search)
        })
        .collect();

    if visible.is_empty() {
        ui.label(RichText::new("No containers match these filters.").color(MUTED));
        return;
    }

    egui::Grid::new("empties-grid")
        .num_columns(8)
        .striped(true)
        .min_col_width(0.0)
        .spacing(Vec2::new(14.0, 6.0))
        .show(ui, |ui| {
            // Header, with a check-all box for the visible rows.
            let all_checked = visible.iter().all(|row| checked.contains(&row.item.container));
            let mut all = all_checked;
            if ui
                .checkbox(&mut all, "")
                .on_hover_text("Include all shown containers in the APM/LBCT appointment check")
                .changed()
            {
                actions.push(Action::SetChecked(
                    visible.iter().map(|row| row.item.container.clone()).collect(),
                    all,
                ));
            }
            for head in ["Container", "SSL", "Size", "Type", "Return", "Proof", ""] {
                section_label(ui, head);
            }
            ui.end_row();

            for row in &visible {
                let item = &row.item;
                let editing = form
                    .editing
                    .as_ref()
                    .is_some_and(|edit| edit.container == item.container);

                let mut on = checked.contains(&item.container);
                if ui
                    .checkbox(&mut on, "")
                    .on_hover_text("Include in the APM/LBCT appointment check")
                    .changed()
                {
                    actions.push(Action::SetChecked(vec![item.container.clone()], on));
                }

                let is_current = current == item.container;
                ui.horizontal(|ui| {
                    let text = RichText::new(&item.container).monospace().strong();
                    let label = ui
                        .add(
                            egui::Label::new(if is_current { text.color(ACCENT) } else { text })
                                .sense(Sense::click()),
                        )
                        .on_hover_text(if is_current {
                            "This is the container on the Board"
                        } else {
                            "Click to use this container on the Board"
                        });
                    if label.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if label.clicked() {
                        actions.push(Action::SetCurrent(item.container.clone()));
                    }
                    if is_current {
                        dot(ui, ACCENT);
                    }
                });

                if editing {
                    let edit = form.editing.as_mut().expect("editing row");
                    ssl_field(ui, &format!("edit-ssl-{}", item.container), &mut edit.line);
                    size_picker(ui, &mut edit.size);
                    kind_picker(ui, ("edit-kind", &item.container), &mut edit.kind);
                    ui.label(RichText::new("Save to see where it can go").small().color(MUTED));
                    ui.label("");
                    let mut cancel = false;
                    ui.horizontal(|ui| {
                        if ui.add(primary("Save")).clicked() {
                            actions.push(Action::Save(edit.clone()));
                        }
                        if ui.small_button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                    if cancel {
                        form.editing = None;
                    }
                } else {
                    let unknown = !item.line.is_empty() && upload::normalize_line(&item.line).is_none();
                    ssl_label(ui, &item.line, unknown);
                    if item.size.is_empty() {
                        ui.label(RichText::new("?").color(DUAL));
                    } else {
                        ui.label(format!("{}'", item.size));
                    }
                    ui.label(kind_name(&item.kind));
                    ui.label(RichText::new(&row.summary).color(state_color(row.state)))
                        .on_hover_text(&row.hover);
                    proof_cell(ui, &row.cover, desk_disputes.get(&item.container));
                    ui.horizontal(|ui| {
                        if ui.small_button("Edit").clicked() {
                            form.editing = Some(item.clone());
                        }
                        if ui
                            .add(Button::new(RichText::new("×").size(15.0).color(MUTED)).frame(false))
                            .on_hover_text("Remove from today's list")
                            .clicked()
                        {
                            actions.push(Action::Remove(item.container.clone()));
                        }
                    });
                }
                ui.end_row();
            }
        });
    ui.add_space(6.0);
    let shown = if visible.len() < rows.len() {
        format!("Showing {} of {} · ", visible.len(), rows.len())
    } else {
        String::new()
    };
    ui.label(
        RichText::new(format!(
            "{shown}Ticked containers are checked by the APM/LBCT browser · click a number to use it on the Board"
        ))
        .small()
        .color(MUTED),
    );
}

/// "2/4" covered windows × locations, coloured, with the details on hover.
fn proof_cell(ui: &mut egui::Ui, cover: &proof::Coverage, dispute: Option<&proof::Dispute>) {
    let (text, color) = if cover.locations.is_empty() {
        ("set locations".to_string(), MUTED)
    } else if cover.complete() {
        (format!("✓ {}/{}", cover.done(), cover.total()), OPEN)
    } else if cover.any_open() {
        (format!("{}/{} · slot offered", cover.done(), cover.total()), REFUSAL)
    } else if cover.done() == 0 {
        (format!("0/{}", cover.total()), REFUSAL)
    } else {
        (format!("{}/{}", cover.done(), cover.total()), DUAL)
    };
    let mut hover = cover.lines().join("\n");
    if cover.guessed && !cover.locations.is_empty() {
        hover.push_str("\n\nLocations come from the receiving board. Set the carrier's own list under Dispute screenshots.");
    }
    let deadline = dispute.and_then(|d| d.deadline_text(chrono::Local::now().date_naive()));
    ui.horizontal(|ui| {
        ui.label(RichText::new(text).color(color)).on_hover_text(hover);
        if let Some((due, level)) = deadline {
            if level > 0 {
                let short = if level == 2 { "due!" } else { "due soon" };
                ui.label(RichText::new(short).small().color(if level == 2 { REFUSAL } else { DUAL }))
                    .on_hover_text(due);
            }
        }
    });
}

/// When the carrier windows for this waiver date are, whether one is open,
/// and where each line takes its empties back.
fn proof_section(desk: &mut Desk, ui: &mut egui::Ui, date: &str, rows: &[Row], actions: &mut Vec<Action>) {
    let now = chrono::Local::now().naive_local();
    let lines: Vec<String> = {
        let mut seen: Vec<String> = Vec::new();
        for row in rows {
            let line = row.item.line.trim().to_string();
            if !line.is_empty() && !seen.iter().any(|l| l.eq_ignore_ascii_case(&line)) {
                seen.push(line);
            }
        }
        seen
    };
    let done = rows.iter().filter(|row| row.cover.complete()).count();
    let windows = proof::windows_for("MSC", date);
    let notice = proof::window_notice(&windows, now)
        .or_else(|| proof::window_notice(&proof::windows_for("ONE", date), now));

    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            section_label(ui, "Dispute screenshots");
            ui.label(
                RichText::new(format!("{done} of {} containers fully covered", rows.len()))
                    .small()
                    .color(if done == rows.len() && !rows.is_empty() { OPEN } else { MUTED }),
            );
        });
        ui.label(
            RichText::new(format!(
                "MSC and ONE want a screenshot at every return location, showing the container: {} and {} (Pacific). ONE counts the working day before.",
                windows.first().map(|w| w.describe()).unwrap_or_default(),
                windows.get(1).map(|w| w.describe()).unwrap_or_default(),
            ))
            .small()
            .color(MUTED),
        );
        if let Some((text, open)) = notice {
            ui.add_space(4.0);
            ui.label(RichText::new(text).strong().color(if open { ACCENT } else { TEXT }));
        }

        egui::CollapsingHeader::new(RichText::new("Return locations by line").small())
            .id_salt("return-locations")
            .default_open(false)
            .show(ui, |ui| {
                if lines.is_empty() {
                    ui.label(RichText::new("Add containers with their SSL first.").small().color(MUTED));
                }
                for line in &lines {
                    let key = line.to_uppercase();
                    let (current, guessed) =
                        proof::locations_for(line, &desk.return_locations, &desk.receivings);
                    let mut ids: Vec<String> = current.iter().map(|t| t.id.to_string()).collect();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(line).strong());
                        ui.label(
                            RichText::new(if guessed { "from receiving board" } else { "carrier list" })
                                .small()
                                .color(MUTED),
                        );
                        let mut changed = false;
                        for terminal in terminals() {
                            let mut on = ids.iter().any(|id| id == terminal.id);
                            if ui.toggle_value(&mut on, RichText::new(terminal.name).small()).changed() {
                                if on {
                                    ids.push(terminal.id.to_string());
                                } else {
                                    ids.retain(|id| id != terminal.id);
                                }
                                changed = true;
                            }
                        }
                        if changed {
                            actions.push(Action::SetLocations(key.clone(), Some(ids.clone())));
                        }
                        if !guessed && ui.small_button("Use receiving board").clicked() {
                            actions.push(Action::SetLocations(key.clone(), None));
                        }
                    });
                }
            });
    });
}

fn kind_name(kind: &str) -> String {
    match BOX_KINDS.iter().find(|(code, _)| *code == kind) {
        Some((code, name)) => format!("{code} · {name}"),
        None if kind.is_empty() => "—".to_string(),
        None => kind.to_string(),
    }
}

/// 20 / 40 / 45 as a segmented control. Clicking the chosen size clears it.
fn size_picker(ui: &mut egui::Ui, size: &mut String) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for option in BOX_SIZES {
            let chosen = size == option;
            let button = Button::new(
                RichText::new(format!("{option}'"))
                    .strong()
                    .color(if chosen { ACCENT } else { TEXT }),
            )
            .fill(if chosen { tint(ACCENT, 0.22) } else { SURFACE_HI })
            .stroke(Stroke::new(1.0, if chosen { ACCENT } else { BORDER }))
            .min_size(Vec2::new(40.0, 0.0));
            if ui.add(button).clicked() {
                *size = if chosen { String::new() } else { option.to_string() };
            }
        }
    });
}

fn kind_picker(ui: &mut egui::Ui, id: impl egui::AsIdSalt, kind: &mut String) {
    let label = if kind.is_empty() { "Type".to_string() } else { kind_name(kind) };
    ComboBox::from_id_salt(id)
        .selected_text(RichText::new(label).color(if kind.is_empty() { MUTED } else { TEXT }))
        .width(130.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(kind, String::new(), "Not set");
            for (code, name) in BOX_KINDS {
                ui.selectable_value(kind, code.to_string(), format!("{code} · {name}"));
            }
        });
}

fn apply(
    desk: &mut Desk,
    ui: &egui::Ui,
    action: Action,
    date: &str,
    shift: Shift,
    boxes: &[EmptyBox],
) {
    match action {
        Action::SetLocations(line, ids) => {
            match ids {
                Some(ids) => {
                    desk.return_locations.insert(line.clone(), ids);
                }
                None => {
                    desk.return_locations.remove(&line);
                }
            }
            desk.persist();
            desk.status = format!("Return locations for {line} saved.");
        }
        Action::ChooseFile => {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Empty lists", &["csv", "tsv", "txt", "xlsx", "xlsm", "xls", "ods"])
                .pick_file()
            {
                desk.upload_file(&path);
            }
        }
        Action::Paste => desk.upload_pasted(),
        Action::RemoveUpload(id) => desk.remove_upload(id),
        Action::UseForChecks => {
            desk.containers_text = boxes
                .iter()
                .map(|b| b.container.clone())
                .collect::<Vec<_>>()
                .join("\n");
            desk.persist();
            desk.status = format!("{} containers set as the check list.", boxes.len());
        }
        Action::CopyPlan => {
            ui.ctx()
                .copy_text(return_plan(date, shift, boxes, &desk.receivings));
            desk.status = "Return plan copied.".to_string();
        }
        Action::Capture => desk.capture_receiving(true),
        Action::Save(mut item) => {
            item.line = upload::resolve_line(&item.line).0;
            let number = item.container.clone();
            let now = timestamp();
            let added = upsert_box(&mut desk.uploads, date, item, &mut desk.next_id, &now);
            desk.box_form.editing = None;
            desk.persist();
            desk.status = if added {
                format!("Added {number} for {date}.")
            } else {
                format!("Updated {number}.")
            };
        }
        Action::SaveMany(items) => {
            let now = timestamp();
            let total = items.len();
            let mut added = 0;
            for mut item in items {
                item.line = upload::resolve_line(&item.line).0;
                if upsert_box(&mut desk.uploads, date, item, &mut desk.next_id, &now) {
                    added += 1;
                }
            }
            desk.persist();
            let updated = total - added;
            desk.status = match updated {
                0 => format!("Added {added} containers for {date}."),
                _ => format!("Added {added} and updated {updated} containers for {date}."),
            };
        }
        Action::Remove(number) => {
            remove_box(&mut desk.uploads, date, &number);
            if desk
                .box_form
                .editing
                .as_ref()
                .is_some_and(|edit| edit.container == number)
            {
                desk.box_form.editing = None;
            }
            desk.persist();
            desk.status = format!("Removed {number} from {date}.");
        }
        Action::ClearDay => {
            desk.uploads.retain(|upload| upload.date != date);
            desk.box_form = BoxForm {
                draft: std::mem::take(&mut desk.box_form.draft),
                ..BoxForm::default()
            };
            desk.persist();
            desk.status = format!("Removed all {} containers for {date}.", boxes.len());
        }
        Action::SetCurrent(number) => {
            desk.container = number.clone();
            desk.persist();
            desk.status = format!("{number} is now the container on the Board.");
        }
        Action::SetChecked(numbers, on) => {
            let mut list = desk.container_list();
            if on {
                for number in numbers {
                    if !list.contains(&number) {
                        list.push(number);
                    }
                }
            } else {
                list.retain(|existing| !numbers.contains(existing));
            }
            desk.containers_text = list.join("\n");
            desk.persist();
        }
    }
}
