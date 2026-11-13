//! Importing a Notion base exported as CSV ("Exporter" > Markdown & CSV): each row a song,
//! each column mapped to one of the song's fields. The mapping is guessed from the column
//! names, and the user corrects it before anything is written. Rows are matched to songs by
//! their name; nothing is created, and one Ctrl+Z takes the whole import back.
//!
//! Notion writes multi-select values joined with ", ", a status as its option's name, stars
//! as "⭐⭐⭐", numbers as plain text, relations and files as names or links.
use crate::actions;
use catalog::Rules;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};
use store::rusqlite::{self, Transaction};
use ts_rs::TS;

pub type R<T> = Result<T, String>;

/// Where a column goes. "" = ignored.
pub const TARGETS: [&str; 10] = ["title", "artist", "style", "key", "tempo", "stage", "notes", "rating", "kinds", "ft"];

/// Reads a CSV file (RFC 4180: quotes, doubled quotes, line breaks inside quotes; Notion
/// adds a UTF-8 BOM). Returns the header and the rows.
pub fn read_csv(text: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let text = text.trim_start_matches('\u{feff}');
    let mut rows: Vec<Vec<String>> = Vec::new();
    let (mut row, mut cell, mut quoted) = (Vec::new(), String::new(), false);
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        if quoted {
            match c {
                '"' if it.peek() == Some(&'"') => {
                    cell.push('"');
                    it.next();
                }
                '"' => quoted = false,
                _ => cell.push(c),
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            ',' => row.push(std::mem::take(&mut cell)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            _ => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows.retain(|r| r.iter().any(|c| !c.trim().is_empty()));
    if rows.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let head = rows.remove(0).into_iter().map(|h| h.trim().to_string()).collect();
    (head, rows)
}

fn fold(s: &str) -> String {
    catalog::ascii_low(s).split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

/// A column's field, guessed from its name (French and English).
pub fn guess(col: &str, first: bool) -> &'static str {
    let c = fold(col);
    let has = |ws: &[&str]| ws.iter().any(|w| c == *w || c.starts_with(&format!("{w} ")) || c.ends_with(&format!(" {w}")));
    if has(&["nom", "name", "titre", "title", "morceau", "track name", "song"]) || (first && c.is_empty()) {
        return "title";
    }
    if has(&["interet", "interest", "rating", "note etoiles", "etoiles", "stars", "priorite", "priority", "score"]) {
        return "rating";
    }
    if has(&["etat", "state", "status", "statut", "etape", "stage", "avancement"]) {
        return "stage";
    }
    if has(&["tonality", "tonalite", "key", "cle", "gamme", "scale"]) {
        return "key";
    }
    if has(&["bpm", "tempo"]) {
        return "tempo";
    }
    if has(&["style", "genre", "styles", "genres"]) {
        return "style";
    }
    if has(&["type", "types", "format", "kind", "version"]) {
        return "kinds";
    }
    if has(&["projet", "project", "artiste", "artist", "alias", "label"]) {
        return "artist";
    }
    if has(&["note", "notes", "commentaire", "comment", "comments", "description", "remarques"]) {
        return "notes";
    }
    if has(&["partner", "partenaire", "feat", "featuring", "collab", "collaborateur", "collaborateurs", "contacts", "contact"]) {
        return "ft";
    }
    if first {
        return "title";
    }
    ""
}

/// The stage an option of a Notion status means (his "ID", "Starting Point"... "Terminé").
pub fn guess_stage(v: &str) -> &'static str {
    let c = fold(v);
    let any = |ws: &[&str]| ws.iter().any(|w| c.contains(w));
    if any(&["termine", "fini", "done", "released", "sorti", "publie", "finished", "complete"]) {
        "done"
    } else if any(&["master"]) {
        "master"
    } else if any(&["label", "envoye", "sent", "pitch"]) {
        "master"
    } else if any(&["mix"]) {
        "mix"
    } else if any(&["arrang", "structure"]) {
        "arrangement"
    } else if any(&["pause", "abandon", "frigo", "hold"]) {
        "pause"
    } else if c.is_empty() {
        ""
    } else {
        "draft"
    }
}

/// "⭐⭐⭐⭐" / "4" / "4/5" -> 4.
pub fn stars(v: &str) -> Option<i64> {
    let n = v.chars().filter(|c| *c == '⭐' || *c == '★').count() as i64;
    if n > 0 {
        return Some(n.min(5));
    }
    let d: String = v.trim().chars().take_while(|c| c.is_ascii_digit()).collect();
    d.parse::<i64>().ok().filter(|n| (1..=5).contains(n))
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Column {
    pub name: String,
    /// One of TARGETS, or "" (ignored).
    pub target: String,
    /// A few values, to recognise it.
    pub sample: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct RowMatch {
    #[ts(type = "number")]
    pub row: i64,
    pub name: String,
    #[ts(type = "number | null")]
    pub song: Option<i64>,
    pub song_title: Option<String>,
    /// "exact" (same name), "proche" (same name once versions and numbers set aside).
    pub how: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ImportPreview {
    pub columns: Vec<Column>,
    /// Values of the status column, with their guessed stage and count: [value, stage, n].
    #[ts(type = "[string, string, number][]")]
    pub stages: Vec<(String, String, i64)>,
    pub rows: Vec<RowMatch>,
    #[ts(type = "number")]
    pub matched: i64,
}

/// What to do with a CSV: the columns (with guesses unless `columns` gives the user's
/// choice), its status values, and which song each row is.
pub fn preview(con: &rusqlite::Connection, text: &str, columns: Option<Vec<Column>>) -> R<ImportPreview> {
    let (head, rows) = read_csv(text);
    if head.is_empty() {
        return Err(i18n::tr!("notion.emptyFile"));
    }
    let mut cols: Vec<Column> = head
        .iter()
        .enumerate()
        .map(|(i, h)| Column {
            name: h.clone(),
            target: guess(h, i == 0).to_string(),
            sample: rows.iter().filter_map(|r| r.get(i)).filter(|v| !v.trim().is_empty()).take(3).cloned().collect(),
        })
        .collect();
    if let Some(user) = columns {
        for c in cols.iter_mut() {
            if let Some(u) = user.iter().find(|u| u.name == c.name) {
                c.target = u.target.clone();
            }
        }
    }
    // a field takes one column: the first one guessed keeps it
    let mut seen = std::collections::HashSet::new();
    for c in cols.iter_mut() {
        if !c.target.is_empty() && !seen.insert(c.target.clone()) && c.target != "kinds" {
            c.target.clear();
        }
    }
    let col_of = |t: &str| cols.iter().position(|c| c.target == t);
    let mut stages: BTreeMap<String, i64> = BTreeMap::new();
    if let Some(i) = col_of("stage") {
        for r in &rows {
            if let Some(v) = r.get(i).map(|v| v.trim()).filter(|v| !v.is_empty()) {
                *stages.entry(v.to_string()).or_default() += 1;
            }
        }
    }
    let ti = col_of("title").ok_or_else(|| i18n::tr!("notion.noTitleColumn"))?;
    let matches = match_rows(con, &rows, ti)?;
    let matched = matches.iter().filter(|m| m.song.is_some()).count() as i64;
    Ok(ImportPreview {
        stages: stages.into_iter().map(|(v, n)| (v.clone(), guess_stage(&v).to_string(), n)).collect(),
        rows: matches,
        matched,
        columns: cols,
    })
}

/// Which song each row names: the same title (accents, case and punctuation aside), else
/// the same name once versions, dates and numbers are set aside (the catalogue's key).
fn match_rows(con: &rusqlite::Connection, rows: &[Vec<String>], ti: usize) -> R<Vec<RowMatch>> {
    let artists: Vec<String> = setting(con, "artists").unwrap_or_default();
    let words: Vec<String> = setting(con, "work_words").unwrap_or_default();
    let rules = Rules::new(&artists, &words);
    let songs: Vec<(i64, String, String)> = {
        let mut s = con.prepare("select id, coalesce(title, ''), coalesce(vkey, '') from songs where gone=0").map_err(|e| e.to_string())?;
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(|e| e.to_string())?.flatten().collect()
    };
    let mut by_title: HashMap<String, Vec<usize>> = HashMap::new();
    let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, (_, t, vk)) in songs.iter().enumerate() {
        by_title.entry(fold(t)).or_default().push(i);
        for k in [rules.variant_key(t), vk.clone()] {
            if !k.is_empty() {
                by_key.entry(k).or_default().push(i);
            }
        }
    }
    Ok(rows
        .iter()
        .enumerate()
        .map(|(n, r)| {
            let name = r.get(ti).map(|s| s.trim().to_string()).unwrap_or_default();
            let one = |v: Option<&Vec<usize>>| v.filter(|v| { let mut u = (*v).clone(); u.dedup(); u.len() == 1 }).map(|v| v[0]);
            let (hit, how) = match one(by_title.get(&fold(&name))) {
                Some(i) => (Some(i), "exact"),
                None => (one(by_key.get(&rules.variant_key(&name))).filter(|_| !name.is_empty()), "proche"),
            };
            RowMatch {
                row: n as i64,
                name,
                song: hit.map(|i| songs[i].0),
                song_title: hit.map(|i| songs[i].1.clone()),
                how: if hit.is_some() { how.into() } else { String::new() },
            }
        })
        .collect())
}

fn setting<T: serde::de::DeserializeOwned>(con: &rusqlite::Connection, k: &str) -> Option<T> {
    use store::rusqlite::OptionalExtension;
    con.query_row("select v from settings where k=?", [k], |r| r.get::<_, String>(0))
        .optional()
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_str(&v).ok())
}

/// Writes the import: for every matched row, the mapped fields (`only_empty`: a field
/// already set in the app is left as it is). `stage_map`: Notion status -> stage ("" =
/// left out). Returns (songs changed, the ids, for the undo photo taken before).
pub fn apply(
    tx: &Transaction,
    text: &str,
    columns: &[Column],
    stage_map: &HashMap<String, String>,
    only_empty: bool,
) -> R<Vec<i64>> {
    let pv = preview(tx, text, Some(columns.to_vec()))?;
    let (_, rows) = read_csv(text);
    let mut changed = Vec::new();
    for m in pv.rows.iter().filter(|m| m.song.is_some()) {
        let sid = m.song.unwrap();
        let row = &rows[m.row as usize];
        let current: Map<String, Value> = tx
            .query_row("select artist, style, key, tempo, stage, notes, rating, kinds, ft from songs where id=?", [sid], |r| {
                let mut o = Map::new();
                for (i, k) in ["artist", "style", "key", "tempo", "stage", "notes", "rating", "kinds", "ft"].iter().enumerate() {
                    let v: rusqlite::types::Value = r.get(i)?;
                    o.insert(k.to_string(), match v {
                        rusqlite::types::Value::Null => Value::Null,
                        rusqlite::types::Value::Integer(n) => json!(n),
                        rusqlite::types::Value::Real(f) => json!(f),
                        rusqlite::types::Value::Text(s) => json!(s),
                        rusqlite::types::Value::Blob(_) => Value::Null,
                    });
                }
                Ok(o)
            })
            .map_err(|e| e.to_string())?;
        let mut body = Map::new();
        let mut kinds: Vec<String> = Vec::new();
        for (i, c) in pv.columns.iter().enumerate() {
            let raw = row.get(i).map(|v| v.trim()).unwrap_or("");
            if c.target.is_empty() || c.target == "title" || raw.is_empty() {
                continue;
            }
            let v = match c.target.as_str() {
                "rating" => match stars(raw) {
                    Some(n) => json!(n),
                    None => continue,
                },
                "tempo" => match raw.replace(',', ".").parse::<f64>() {
                    Ok(f) if f > 0.0 => json!(f),
                    _ => continue,
                },
                "stage" => match stage_map.get(raw).map(String::as_str).filter(|s| !s.is_empty()) {
                    Some(s) => json!(s),
                    None => continue,
                },
                "kinds" => {
                    kinds.extend(raw.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()));
                    continue;
                }
                // multi-selects keep Notion's ", "; one key or style is what the app expects
                "key" => json!(raw.split(',').next().unwrap_or(raw).trim()),
                _ => json!(raw),
            };
            if only_empty && current.get(&c.target).is_some_and(|x| !x.is_null() && x != "") {
                continue;
            }
            body.insert(c.target.clone(), v);
        }
        if !kinds.is_empty() && !(only_empty && current.get("kinds").is_some_and(|x| !x.is_null())) {
            body.insert("kinds".into(), json!(kinds));
        }
        if body.is_empty() {
            continue;
        }
        actions::song_edit(tx, sid, &body).map_err(|e| e.to_string())?;
        changed.push(sid);
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quotes_breaks_and_bom() {
        let (h, r) = read_csv("\u{feff}Nom,Style,Note\n\"Lantern, v2\",\"Electro, Pop\",\"ligne 1\nligne \"\"2\"\"\"\nDriftwood,,\n");
        assert_eq!(h, vec!["Nom", "Style", "Note"]);
        assert_eq!(r[0], vec!["Lantern, v2", "Electro, Pop", "ligne 1\nligne \"2\""]);
        assert_eq!(r[1], vec!["Driftwood", "", ""]);
    }

    #[test]
    fn label_columns_guessed() {
        let cols = ["Nom", "Projet", "Type", "Style", "Tonality", "Bpm", "Interêt", "État", "Note", "Track", "Partner", "Contacts 1"];
        let g: Vec<&str> = cols.iter().enumerate().map(|(i, c)| guess(c, i == 0)).collect();
        assert_eq!(g, vec!["title", "artist", "kinds", "style", "key", "tempo", "rating", "stage", "notes", "", "ft", "ft"]);
        assert_eq!(stars("⭐⭐⭐⭐"), Some(4));
        assert_eq!(guess_stage("ID"), "draft");
        assert_eq!(guess_stage("Send to a Label"), "master");
        assert_eq!(guess_stage("Terminé"), "done");
        assert_eq!(guess_stage("Sound-Design"), "draft");
    }
}
