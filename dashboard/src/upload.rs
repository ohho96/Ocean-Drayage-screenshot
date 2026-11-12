//! Read a daily list of empties from CSV, tab-separated text (Excel paste) or
//! an Excel workbook.
//!
//! Columns are found by header name (container / SSL / size / type, many
//! spellings). Without a header row, the container column is found by its
//! shape (4 letters + 7 digits), and the other cells are recognised by
//! content: shipping line names or SCAC codes, sizes like 40, 40HC, 40' HC,
//! and ISO size-type codes like 45G1 or 22R1.

use std::path::Path;

use crate::model::EmptyBox;

pub struct Parsed {
    pub boxes: Vec<EmptyBox>,
    pub warnings: Vec<String>,
}

/// Shipping line aliases -> dashboard names (LINES).
const LINE_ALIASES: &[(&str, &str)] = &[
    ("MAERSK", "Maersk"), ("MAEU", "Maersk"), ("MSK", "Maersk"), ("MAE", "Maersk"),
    ("MSC", "MSC"), ("MSCU", "MSC"), ("MEDITERRANEAN SHIPPING", "MSC"),
    ("CMA CGM", "CMA CGM"), ("CMA", "CMA CGM"), ("CMDU", "CMA CGM"), ("CMACGM", "CMA CGM"),
    ("COSCO", "COSCO"), ("COSU", "COSCO"), ("COS", "COSCO"), ("OOCL", "COSCO"), ("OOLU", "COSCO"),
    ("EVERGREEN", "Evergreen"), ("EGLV", "Evergreen"), ("EMC", "Evergreen"), ("EGL", "Evergreen"),
    ("HAPAG-LLOYD", "Hapag-Lloyd"), ("HAPAG LLOYD", "Hapag-Lloyd"), ("HAPAG", "Hapag-Lloyd"),
    ("HLCU", "Hapag-Lloyd"), ("HLC", "Hapag-Lloyd"), ("HAP", "Hapag-Lloyd"),
    ("ONE", "ONE"), ("ONEY", "ONE"), ("OCEAN NETWORK EXPRESS", "ONE"),
    ("YANG MING", "Yang Ming"), ("YANGMING", "Yang Ming"), ("YMLU", "Yang Ming"), ("YML", "Yang Ming"),
    ("HMM", "HMM"), ("HDMU", "HMM"), ("HYUNDAI", "HMM"), ("HYUNDAI MERCHANT MARINE", "HMM"),
    ("ZIM", "ZIM"), ("ZIMU", "ZIM"),
    ("WAN HAI", "Wan Hai"), ("WANHAI", "Wan Hai"), ("WHLC", "Wan Hai"), ("WHL", "Wan Hai"),
    ("PIL", "PIL"), ("PCIU", "PIL"), ("PACIFIC INTERNATIONAL LINES", "PIL"),
    ("SM LINE", "SM Line"), ("SMLM", "SM Line"), ("SML", "SM Line"),
    ("MATSON", "Matson"), ("MATS", "Matson"), ("MAT", "Matson"),
];

pub fn normalize_line(raw: &str) -> Option<&'static str> {
    let key = raw.trim().to_uppercase().replace(['.', ','], "");
    let key = key.split_whitespace().collect::<Vec<_>>().join(" ");
    if key.is_empty() {
        return None;
    }
    if let Some((_, name)) = LINE_ALIASES.iter().find(|(alias, _)| *alias == key) {
        return Some(name);
    }
    // "OCEAN NETWORK EXPRESS (ONE)" or "Yang Ming Line"
    if let (Some(open), Some(close)) = (key.find('('), key.find(')')) {
        if open < close {
            if let Some(name) = normalize_line(&key[open + 1..close]) {
                return Some(name);
            }
        }
    }
    LINE_ALIASES
        .iter()
        .filter(|(alias, _)| alias.len() >= 4)
        .find(|(alias, _)| key.starts_with(alias))
        .map(|(_, name)| *name)
}

/// Read (size, kind) from text like "40", "40HC", "40' HC", "40HQ", "45G1", "22R1", "HC", "Reefer".
pub fn parse_size_type(raw: &str) -> (String, String) {
    let text = raw.trim().to_uppercase();
    let compact: String = text.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if compact.is_empty() {
        return (String::new(), String::new());
    }
    // ISO 6346 size-type codes: 22G1, 42G1, 45G1, L5G1, 22R1, 45R1, 22U1, 42P1, 22T1 ...
    if compact.len() == 4 {
        let bytes = compact.as_bytes();
        let size = match bytes[0] {
            b'2' => Some("20"),
            b'4' => Some("40"),
            b'L' => Some("45"),
            _ => None,
        };
        let high = bytes[1] == b'5' || bytes[0] == b'L';
        let kind = match bytes[2] {
            b'G' => Some(if high { "HC" } else { "DV" }),
            b'R' => Some("RF"),
            b'U' => Some("OT"),
            b'P' => Some("FR"),
            b'T' => Some("TK"),
            _ => None,
        };
        if let (Some(size), Some(kind)) = (size, kind) {
            if bytes[1].is_ascii_digit() || bytes[0] == b'L' {
                return (size.into(), kind.into());
            }
        }
    }
    let size = ["45", "40", "20"]
        .into_iter()
        .find(|size| compact.starts_with(size))
        .unwrap_or("")
        .to_string();
    let rest = compact.trim_start_matches(size.as_str());
    let kind = parse_kind(rest).unwrap_or(if size.is_empty() { "" } else { "" });
    (size, kind.to_string())
}

fn parse_kind(raw: &str) -> Option<&'static str> {
    let key: String = raw
        .to_uppercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let kind = match key.as_str() {
        "" => return None,
        "DV" | "DC" | "GP" | "SD" | "ST" | "STD" | "DRY" | "G" | "SD86" => "DV",
        "HC" | "HQ" | "HCDV" | "HIGHCUBE" | "SD96" | "HCD" => "HC",
        "RF" | "RE" | "RH" | "REEFER" | "RFR" | "HCRF" | "RQ" => "RF",
        "OT" | "OPENTOP" | "UT" => "OT",
        "FR" | "FLATRACK" | "FLAT" | "PF" => "FR",
        "TK" | "TANK" => "TK",
        _ => return None,
    };
    Some(kind)
}

/// A container number: 4 letters then 7 digits (spaces and dashes ignored).
pub fn as_container(raw: &str) -> Option<String> {
    let compact: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_uppercase();
    let ok = compact.len() == 11
        && compact[..4].chars().all(|c| c.is_ascii_alphabetic())
        && compact[4..].chars().all(|c| c.is_ascii_digit());
    ok.then_some(compact)
}

#[derive(Default)]
struct Columns {
    container: Option<usize>,
    line: Option<usize>,
    size: Option<usize>,
    kind: Option<usize>,
}

fn header_columns(cells: &[String]) -> Option<Columns> {
    let mut columns = Columns::default();
    for (index, cell) in cells.iter().enumerate() {
        let key: String = cell
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        let is = |names: &[&str]| names.iter().any(|name| key == *name);
        if columns.container.is_none()
            && (is(&["container", "containerno", "containernumber", "cntr", "cntrno", "unit", "unitnumber", "equipment", "equipmentnumber", "box"])
                || key.starts_with("container"))
        {
            columns.container = Some(index);
        } else if columns.line.is_none()
            && is(&["ssl", "line", "sline", "ssline", "shippingline", "steamshipline", "carrier", "owner", "scac", "lineop", "lineoperator", "operator"])
        {
            columns.line = Some(index);
        } else if columns.size.is_none()
            && is(&["size", "sizetype", "sztp", "sizetp", "length", "isocode", "iso", "eqtype", "equipmenttype", "cntrsize", "containersize"])
        {
            columns.size = Some(index);
        } else if columns.kind.is_none() && is(&["type", "tp", "cntrtype", "containertype", "kind", "height"]) {
            columns.kind = Some(index);
        }
    }
    columns.container.map(|_| columns)
}

fn split_rows(text: &str) -> Vec<Vec<String>> {
    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    let sample = lines.iter().take(5).copied().collect::<Vec<_>>().join("\n");
    let delimiter = if sample.contains('\t') {
        '\t'
    } else if sample.contains(',') {
        ','
    } else if sample.contains(';') {
        ';'
    } else {
        ' '
    };
    lines
        .into_iter()
        .map(|line| {
            if delimiter == ' ' {
                line.split_whitespace().map(str::to_string).collect()
            } else {
                split_delimited(line, delimiter)
            }
        })
        .collect()
}

/// Split one line, honouring double quotes (CSV).
fn split_delimited(line: &str, delimiter: char) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == delimiter && !quoted => cells.push(std::mem::take(&mut cell).trim().to_string()),
            c => cell.push(c),
        }
    }
    cells.push(cell.trim().to_string());
    cells
}

pub fn parse_rows(rows: Vec<Vec<String>>) -> Parsed {
    let mut boxes: Vec<EmptyBox> = Vec::new();
    let mut warnings = Vec::new();
    let mut rows = rows.into_iter().peekable();

    // Skip title rows until a header or a container shows up.
    let mut columns = None;
    while let Some(row) = rows.peek() {
        if let Some(found) = header_columns(row) {
            columns = Some(found);
            rows.next();
            break;
        }
        if row.iter().any(|cell| as_container(cell).is_some()) {
            break;
        }
        rows.next();
    }

    for (number, row) in rows.enumerate() {
        let cell = |index: Option<usize>| index.and_then(|i| row.get(i)).map(String::as_str).unwrap_or("");
        let container = match &columns {
            Some(columns) => as_container(cell(columns.container)),
            None => row.iter().find_map(|cell| as_container(cell)),
        };
        let Some(container) = container else {
            if row.iter().any(|cell| !cell.trim().is_empty()) {
                warnings.push(format!("Row {}: no container number, skipped.", number + 1));
            }
            continue;
        };

        let (mut line, mut size, mut kind) = (String::new(), String::new(), String::new());
        if let Some(columns) = &columns {
            let raw_line = cell(columns.line);
            line = normalize_line(raw_line).map(str::to_string).unwrap_or_else(|| raw_line.trim().to_string());
            let (s, k) = parse_size_type(cell(columns.size));
            size = s;
            kind = k;
            if let Some(k) = parse_kind(cell(columns.kind)) {
                kind = k.to_string();
            } else if kind.is_empty() {
                let (s2, k2) = parse_size_type(cell(columns.kind));
                if size.is_empty() {
                    size = s2;
                }
                kind = k2;
            }
        } else {
            for value in &row {
                if as_container(value).is_some() {
                    continue;
                }
                if line.is_empty() {
                    if let Some(name) = normalize_line(value) {
                        line = name.to_string();
                        continue;
                    }
                }
                let (s, k) = parse_size_type(value);
                if size.is_empty() && !s.is_empty() {
                    size = s;
                }
                if kind.is_empty() && !k.is_empty() {
                    kind = k;
                } else if kind.is_empty() {
                    if let Some(k) = parse_kind(value) {
                        kind = k.to_string();
                    }
                }
            }
        }
        if size == "20" || size == "40" {
            if kind.is_empty() {
                kind = "DV".into();
            }
        } else if size == "45" && kind.is_empty() {
            kind = "HC".into();
        }

        if line.is_empty() {
            warnings.push(format!("{container}: no shipping line."));
        } else if normalize_line(&line).is_none() {
            warnings.push(format!("{container}: shipping line \"{line}\" not recognised."));
        }
        if size.is_empty() {
            warnings.push(format!("{container}: no size."));
        }

        let item = EmptyBox { container, line, size, kind };
        if let Some(existing) = boxes.iter_mut().find(|b| b.container == item.container) {
            warnings.push(format!("{}: listed twice, kept the last row.", item.container));
            *existing = item;
        } else {
            boxes.push(item);
        }
    }
    Parsed { boxes, warnings }
}

pub fn parse_text(text: &str) -> Parsed {
    parse_rows(split_rows(text))
}

pub fn parse_file(path: &Path) -> Result<Parsed, String> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if matches!(extension.as_str(), "xlsx" | "xlsm" | "xls" | "xlsb" | "ods") {
        use calamine::Reader;
        let mut workbook = calamine::open_workbook_auto(path).map_err(|err| format!("Could not open workbook: {err}"))?;
        let range = workbook
            .worksheet_range_at(0)
            .ok_or("The workbook has no sheets.")?
            .map_err(|err| format!("Could not read the first sheet: {err}"))?;
        let rows = range
            .rows()
            .map(|row| row.iter().map(|cell| cell.to_string().trim().to_string()).collect())
            .collect();
        return Ok(parse_rows(rows));
    }
    let bytes = std::fs::read(path).map_err(|err| format!("Could not read {}: {err}", path.display()))?;
    Ok(parse_text(&String::from_utf8_lossy(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_csv_with_header() {
        let parsed = parse_text(
            "Daily empties\nContainer #,SSL,Size,Type\nYMLU 123456-7,YML,40,HC\nTGHU7654321,Maersk,20,DV\nCMAU1111111,CMDU,45,\n,,,\n",
        );
        assert_eq!(parsed.boxes.len(), 3, "{:?}", parsed.warnings);
        assert_eq!(parsed.boxes[0], EmptyBox { container: "YMLU1234567".into(), line: "Yang Ming".into(), size: "40".into(), kind: "HC".into() });
        assert_eq!(parsed.boxes[1].line, "Maersk");
        assert_eq!(parsed.boxes[2].line, "CMA CGM");
        assert_eq!(parsed.boxes[2].kind, "HC");
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
    }

    #[test]
    fn reads_excel_paste_without_header() {
        let parsed = parse_text("ONEU1234567\tONE\t40HC\nHDMU7654321\tHMM\t22G1\nZIMU0000001\tZIM\t45R1\nWHLU1231231\tWan Hai\t40'\n");
        let sizes: Vec<String> = parsed.boxes.iter().map(|b| b.size_label()).collect();
        assert_eq!(sizes, vec!["40' HC", "20' DV", "40' RF", "40' DV"]);
        assert_eq!(parsed.boxes[1].line, "HMM");
        assert_eq!(parsed.boxes[3].line, "Wan Hai");
    }

    #[test]
    fn size_and_line_spellings() {
        assert_eq!(parse_size_type("45G1"), ("40".into(), "HC".into()));
        assert_eq!(parse_size_type("L5G1"), ("45".into(), "HC".into()));
        assert_eq!(parse_size_type("40' HQ"), ("40".into(), "HC".into()));
        assert_eq!(parse_size_type("20GP"), ("20".into(), "DV".into()));
        assert_eq!(parse_size_type("40RF"), ("40".into(), "RF".into()));
        assert_eq!(normalize_line("Ocean Network Express (ONE)"), Some("ONE"));
        assert_eq!(normalize_line("hapag-lloyd"), Some("Hapag-Lloyd"));
        assert_eq!(normalize_line("Evergreen Line"), Some("Evergreen"));
        assert_eq!(normalize_line("Nobody"), None);
    }

    /// Run with DESK_XLSX_FIXTURE=/path/to/file.xlsx cargo test -- --ignored
    #[test]
    #[ignore]
    fn reads_xlsx_fixture() {
        let path = std::env::var("DESK_XLSX_FIXTURE").expect("set DESK_XLSX_FIXTURE");
        let parsed = parse_file(Path::new(&path)).unwrap();
        for b in &parsed.boxes {
            println!("{} | {} | {}", b.container, b.line, b.size_label());
        }
        println!("warnings: {:?}", parsed.warnings);
        assert_eq!(parsed.boxes.len(), 4);
    }

    #[test]
    fn warns_about_problems() {
        let parsed = parse_text("container,line,size\nMSCU1234567,Nobody Shipping,40\nMSCU1234567,MSC,\nnot a box,MSC,40\n");
        assert_eq!(parsed.boxes.len(), 1);
        assert_eq!(parsed.boxes[0].line, "MSC");
        let all = parsed.warnings.join(" | ");
        assert!(all.contains("not recognised"), "{all}");
        assert!(all.contains("listed twice"), "{all}");
        assert!(all.contains("no container"), "{all}");
        assert!(all.contains("no size"), "{all}");
    }
}
