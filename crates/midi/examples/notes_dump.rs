//! Dumps the notes of MIDI files, for working on the key estimate outside Rust.
//!     cargo run --release -p midi --example notes_dump -- <list.tsv> <out.jsonl>
//! The list: one file per line (first column a path). Files are only read.
use std::io::{BufRead, Write};

fn main() {
    let mut a = std::env::args().skip(1);
    let (list, out) = (a.next().expect("list"), a.next().expect("out"));
    let mut w = std::io::BufWriter::new(std::fs::File::create(out).unwrap());
    for line in std::io::BufReader::new(std::fs::File::open(list).unwrap()).lines() {
        let line = line.unwrap();
        let path = line.split('\t').next().unwrap().to_string();
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(smf) = midly::Smf::parse(&bytes) else { continue };
        let tpq = match smf.header.timing {
            midly::Timing::Metrical(t) => t.as_int().max(1) as u64,
            _ => 480,
        };
        let mut notes: Vec<[u64; 5]> = Vec::new();
        for (ti, track) in smf.tracks.iter().enumerate() {
            let mut t = 0u64;
            let mut open = std::collections::HashMap::new();
            for ev in track {
                t += ev.delta.as_int() as u64;
                if let midly::TrackEventKind::Midi { channel, message } = ev.kind {
                    let ch = channel.as_int();
                    match message {
                        midly::MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            open.insert((ch, key.as_int()), t);
                        }
                        midly::MidiMessage::NoteOn { key, .. } | midly::MidiMessage::NoteOff { key, .. } => {
                            if let Some(s) = open.remove(&(ch, key.as_int())) {
                                notes.push([s, t.max(s + 1), key.as_int() as u64, ch as u64, ti as u64]);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        notes.sort();
        notes.truncate(6000);
        writeln!(w, "{}", serde_json::json!({"path": path, "tpq": tpq, "notes": notes})).unwrap();
    }
}
