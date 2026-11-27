//! assemble_check <out.als> <base.als> <bpm> <set.als>::<track id>::<name>::<colour> ...
//! Builds a clean start from tracks of other sets, then checks what Live is strict about.
use repair::xml::read_set;
use std::collections::{HashMap, HashSet};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let out = std::path::PathBuf::from(&a[0]);
    let picks = a[3..]
        .iter()
        .map(|s| {
            let f: Vec<&str> = s.split("::").collect();
            repair::assemble::Pick { set: f[0].into(), track: f[1].into(), name: f.get(2).map(|x| x.to_string()), color: f.get(3).and_then(|c| c.parse().ok()) }
        })
        .collect();
    let bars = |b: f64| b * 4.0;
    let o = repair::assemble::Order {
        base: a[1].clone().into(),
        picks,
        main_devices: vec!["StereoGain".into(), "Limiter".into()],
        donors: vec![],
        tempo: a[2].parse().ok(),
        scale: None,
        markers: vec![(bars(0.0), "Intro".into()), (bars(16.0), "Buildup".into()), (bars(32.0), "Drop".into()), (bars(64.0), "Break".into()), (bars(80.0), "Drop 2".into()), (bars(112.0), "Outro".into())],
        volume_db: -6.0,
        keep_mix: false,
        out: out.clone(),
    };
    let rep = repair::assemble::assemble(&o).unwrap_or_else(|e| panic!("{e}"));
    println!("{}", serde_json::to_string_pretty(&rep).unwrap());
    // checks
    let s = read_set(&out).unwrap();
    let ls = s.root.child("LiveSet").unwrap();
    let tracks = &ls.child("Tracks").unwrap().children;
    let ids: Vec<&str> = tracks.iter().map(|t| t.get("Id").unwrap()).collect();
    let uniq: HashSet<&&str> = ids.iter().collect();
    println!("tracks {} unique ids {}", ids.len(), uniq.len() == ids.len());
    let mut pointees: HashMap<String, usize> = HashMap::new();
    for e in s.root.descendants() {
        if repair::studio::is_pointee(&e.tag) && let Some(i) = e.get("Id") { *pointees.entry(i.to_string()).or_default() += 1; }
    }
    let dup = pointees.iter().filter(|(k, n)| **n > 1 && k.as_str() != "0").count();
    let next: i64 = ls.child("NextPointeeId").and_then(|e| e.get("Value")).unwrap().parse().unwrap();
    let maxp = pointees.keys().filter_map(|k| k.parse::<i64>().ok()).max().unwrap_or(0);
    println!("pointees duplicated {} next > max: {}", dup, next > maxp);
    let dangling = s.root.descendants().filter(|e| e.tag == "PointeeId").filter(|e| !pointees.contains_key(e.get("Value").unwrap_or(""))).count();
    println!("dangling PointeeId {dangling}");
    let returns = tracks.iter().filter(|t| t.tag == "ReturnTrack").count();
    let scenes = ls.child("Scenes").map(|s| s.children.len()).unwrap_or(0);
    let sends: Vec<usize> = s.root.descendants().filter(|e| e.tag == "Sends").map(|e| e.children.len()).collect();
    let sendspre = ls.child("SendsPre").map(|s| s.children.len()).unwrap_or(0);
    println!("returns {returns} sendspre {sendspre} sends lists off {}", sends.iter().filter(|n| **n != returns).count());
    let slots_off = s.root.descendants().filter(|e| repair::assemble::is_slot_list(e) && !e.children.is_empty() && e.children.len() != scenes).count();
    let refs = s.root.descendants().filter(|e| e.tag == "Target").filter_map(|e| e.get("Value")).filter(|v| v.contains("/Track.")).count();
    let clips = s.root.descendants().filter(|e| e.tag == "MidiClip" || e.tag == "AudioClip").count();
    println!("scenes {scenes} slot lists off {slots_off} track refs {refs} clips {clips}");
    let info = als::read_als(&out, &als::Env::detect(None));
    println!("re-read: error {:?} tempo {:?} main {:?} locators {:?}", info.error, info.tempo, info.main_chain, info.locators);
    for t in info.track_info { println!("  {:>4} {:6} {:6} col {:>2} vol {:?} {:20} chain {:?}", t.id, t.kind, t.role, t.color, t.volume, t.name, t.chain); }
    let mt = ls.child("MainTrack").or(ls.child("MasterTrack")).unwrap();
    for d in &mt.find("DeviceChain/DeviceChain/Devices").unwrap().children {
        let params: Vec<String> = d.children.iter().filter(|c| c.child("Manual").is_some()).map(|c| format!("{}={}", c.tag, c.value("Manual").unwrap_or(""))).take(12).collect();
        println!("  master {} {:?}", d.tag, params);
    }
}
