//! What changed between two saves of a project: tempo, length, tracks added, removed or
//! renamed, the devices and plugins of each track, the samples, the locators. Read from what
//! the scan kept of each file (`files.data`): no project is opened again.
use als::{AlsInfo, TrackInfo};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use store::rusqlite::{Connection, OptionalExtension};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS, Default, PartialEq)]
#[ts(export)]
pub struct Change {
    /// "tempo", "length", "sig", "key", "live", "tracks", "samples"
    pub what: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, TS, Default)]
#[ts(export)]
pub struct TrackChange {
    pub name: String,
    /// Renamed: its old name.
    pub was: Option<String>,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    /// MIDI notes, audio clips: (before, after) when it changed.
    #[ts(type = "[number, number] | null")]
    pub notes: Option<(u32, u32)>,
    #[ts(type = "[number, number] | null")]
    pub clips: Option<(u32, u32)>,
}

#[derive(Debug, Clone, Serialize, TS, Default)]
#[ts(export)]
pub struct AlsDiff {
    pub a: String,
    pub b: String,
    pub a_mtime: f64,
    pub b_mtime: f64,
    pub changes: Vec<Change>,
    pub tracks_added: Vec<String>,
    pub tracks_removed: Vec<String>,
    pub tracks_changed: Vec<TrackChange>,
    pub plugins_added: Vec<String>,
    pub plugins_removed: Vec<String>,
    pub samples_added: Vec<String>,
    pub samples_removed: Vec<String>,
    pub locators_added: Vec<String>,
    pub locators_removed: Vec<String>,
    /// Nothing differs in what the app reads (the change is elsewhere: automation, mixing...).
    pub same: bool,
}

fn load(con: &Connection, rel: &str) -> Result<(AlsInfo, f64), String> {
    let row: Option<(Option<String>, Option<f64>)> = con
        .query_row("select unz(data), mtime from files where rel=?", [rel], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .map_err(|e| e.to_string())?;
    let (data, mtime) = row.ok_or_else(|| i18n::tr!("err.projectNotFound"))?;
    let info: AlsInfo = data.as_deref().and_then(|d| serde_json::from_str(d).ok()).ok_or_else(|| i18n::tr!("diff.notRead"))?;
    if let Some(e) = &info.error {
        return Err(i18n::tr!("diff.unreadable", error = e));
    }
    Ok((info, mtime.unwrap_or(0.0)))
}

fn set<I: IntoIterator<Item = String>>(i: I) -> BTreeSet<String> {
    i.into_iter().filter(|x| !x.trim().is_empty()).collect()
}
fn minus(a: &BTreeSet<String>, b: &BTreeSet<String>) -> Vec<String> {
    a.difference(b).cloned().collect()
}
/// A track's devices, as a set ("Operator", "Serum", "EQ Eight"...).
fn devices(t: &TrackInfo) -> BTreeSet<String> {
    set(t.chain.iter().chain(t.plugins.iter()).cloned().chain(t.instrument.clone()))
}
fn mmss(s: i64) -> String {
    format!("{}:{:02}", s / 60, s % 60)
}
fn num(x: f64) -> String {
    let r = (x * 100.0).round() / 100.0;
    if r.fract() == 0.0 { format!("{r:.0}") } else { format!("{r}") }
}

/// What changed from `a` (older) to `b` (newer).
pub fn diff(con: &Connection, a: &str, b: &str) -> Result<AlsDiff, String> {
    let (x, xm) = load(con, a)?;
    let (y, ym) = load(con, b)?;
    Ok(compare(&x, &y, a, b, xm, ym))
}

pub fn compare(x: &AlsInfo, y: &AlsInfo, a: &str, b: &str, xm: f64, ym: f64) -> AlsDiff {
    let mut d = AlsDiff { a: a.into(), b: b.into(), a_mtime: xm, b_mtime: ym, ..Default::default() };
    let mut fact = |what: &str, from: String, to: String| {
        if from != to {
            d.changes.push(Change { what: what.into(), from, to });
        }
    };
    let opt = |v: Option<String>| v.unwrap_or_default();
    fact("tempo", opt(x.tempo.map(num)), opt(y.tempo.map(num)));
    fact("length", opt(x.length_s.map(mmss)), opt(y.length_s.map(mmss)));
    fact("sig", opt(x.time_sig.clone()), opt(y.time_sig.clone()));
    fact("key", opt(x.key.clone()), opt(y.key.clone()));
    fact("live", opt(x.live_version.clone()), opt(y.live_version.clone()));
    let tracks = |i: &AlsInfo| i.track_info.iter().filter(|t| t.kind != "return" || !t.name.is_empty()).count();
    fact("tracks", tracks(x).to_string(), tracks(y).to_string());
    fact("samples", x.samples_total.to_string(), y.samples_total.to_string());

    // tracks: the same track keeps its id across saves (a rename keeps it), else its name
    let by_id: HashMap<&str, &TrackInfo> = x.track_info.iter().filter(|t| !t.id.is_empty()).map(|t| (t.id.as_str(), t)).collect();
    let mut matched: BTreeSet<usize> = BTreeSet::new();
    for t in &y.track_info {
        let old = by_id
            .get(t.id.as_str())
            .copied()
            .filter(|o| o.kind == t.kind)
            .or_else(|| x.track_info.iter().find(|o| o.name == t.name && o.kind == t.kind));
        let Some(o) = old else {
            d.tracks_added.push(t.name.clone());
            continue;
        };
        if let Some(i) = x.track_info.iter().position(|z| std::ptr::eq(z, o)) {
            matched.insert(i);
        }
        let (da, db) = (devices(o), devices(t));
        let c = TrackChange {
            name: t.name.clone(),
            was: (o.name != t.name).then(|| o.name.clone()),
            added: minus(&db, &da),
            removed: minus(&da, &db),
            notes: (o.notes != t.notes).then_some((o.notes, t.notes)),
            clips: (o.audio_clips != t.audio_clips).then_some((o.audio_clips, t.audio_clips)),
        };
        if c.was.is_some() || !c.added.is_empty() || !c.removed.is_empty() || c.notes.is_some() || c.clips.is_some() {
            d.tracks_changed.push(c);
        }
    }
    for (i, t) in x.track_info.iter().enumerate() {
        if !matched.contains(&i) {
            d.tracks_removed.push(t.name.clone());
        }
    }
    let plugs = |i: &AlsInfo| set(i.plugins.iter().map(|p| p.name.clone()));
    let (pa, pb) = (plugs(x), plugs(y));
    d.plugins_added = minus(&pb, &pa);
    d.plugins_removed = minus(&pa, &pb);
    let (sa, sb) = (set(x.sample_names.clone()), set(y.sample_names.clone()));
    d.samples_added = minus(&sb, &sa);
    d.samples_removed = minus(&sa, &sb);
    let locs = |i: &AlsInfo| set(i.locators.iter().map(|(t, n)| format!("{} {}", mmss(*t as i64), n)));
    let (la, lb) = (locs(x), locs(y));
    d.locators_added = minus(&lb, &la);
    d.locators_removed = minus(&la, &lb);
    d.same = d.changes.is_empty()
        && d.tracks_added.is_empty()
        && d.tracks_removed.is_empty()
        && d.tracks_changed.is_empty()
        && d.plugins_added.is_empty()
        && d.plugins_removed.is_empty()
        && d.samples_added.is_empty()
        && d.samples_removed.is_empty()
        && d.locators_added.is_empty()
        && d.locators_removed.is_empty();
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tr(id: &str, kind: &str, name: &str, chain: &[&str], notes: u32) -> TrackInfo {
        TrackInfo { id: id.into(), kind: kind.into(), name: name.into(), chain: chain.iter().map(|s| s.to_string()).collect(), notes, ..Default::default() }
    }
    #[test]
    fn tracks_renamed_added_removed_and_their_devices() {
        let x = AlsInfo {
            tempo: Some(172.0),
            length_s: Some(200),
            track_info: vec![tr("1", "midi", "Bass", &["Operator", "EQ Eight"], 40), tr("2", "audio", "Vox", &[], 0), tr("3", "midi", "Pad", &["Wavetable"], 12)],
            sample_names: vec!["kick 01".into(), "snare 04".into()],
            ..Default::default()
        };
        let y = AlsInfo {
            tempo: Some(174.0),
            length_s: Some(215),
            track_info: vec![tr("1", "midi", "Sub Bass", &["Operator", "Saturator"], 52), tr("3", "midi", "Pad", &["Wavetable"], 12), tr("9", "midi", "Lead", &["Serum"], 30)],
            sample_names: vec!["kick 01".into(), "clap 02".into()],
            ..Default::default()
        };
        let d = compare(&x, &y, "a.als", "b.als", 0.0, 1.0);
        assert!(d.changes.contains(&Change { what: "tempo".into(), from: "172".into(), to: "174".into() }));
        assert!(d.changes.contains(&Change { what: "length".into(), from: "3:20".into(), to: "3:35".into() }));
        assert_eq!(d.tracks_added, ["Lead"]);
        assert_eq!(d.tracks_removed, ["Vox"]);
        assert_eq!(d.tracks_changed.len(), 1, "Pad did not change");
        let b = &d.tracks_changed[0];
        assert_eq!((b.name.as_str(), b.was.as_deref()), ("Sub Bass", Some("Bass")));
        assert_eq!((b.added.clone(), b.removed.clone()), (vec!["Saturator".to_string()], vec!["EQ Eight".to_string()]));
        assert_eq!(b.notes, Some((40, 52)));
        assert_eq!((d.samples_added.clone(), d.samples_removed.clone()), (vec!["clap 02".to_string()], vec!["snare 04".to_string()]));
        assert!(!d.same);
        assert!(compare(&x, &x, "a", "a", 0.0, 0.0).same);
    }
}
