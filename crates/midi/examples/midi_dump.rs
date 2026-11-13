//! midi_dump <file>...: what each MIDI file holds.
fn main() {
    for a in std::env::args().skip(1) {
        let b = std::fs::read(&a).unwrap();
        let name = std::path::Path::new(&a).file_name().unwrap().to_string_lossy().into_owned();
        let i = midi::analyse(&b, &name);
        println!("{name:42} {:7} key {:5} {:4} bpm {:?} {} {:5} bars {:4} notes | {} | parts {:?}", i.kind, i.key.clone().unwrap_or_default(), i.key_strength, i.bpm, i.meter, i.bars, i.notes,
            i.chords.iter().zip(&i.degrees).map(|(c, d)| format!("{c}({d})")).collect::<Vec<_>>().join(" "),
            i.parts.iter().map(|p| format!("{}:{}", p.kind, p.notes)).collect::<Vec<_>>());
    }
}
