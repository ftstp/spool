//! clips_check <set.als>...: the MIDI clips of each set (read only).
fn main() {
    for a in std::env::args().skip(1) {
        let (tempo, clips) = als::midi_clips(std::path::Path::new(&a));
        println!("{} | tempo {:?} | {} clips", a.rsplit('/').next().unwrap_or(&a), tempo, clips.len());
        for c in clips.iter().take(6) {
            let lo = c.notes.iter().map(|n| n.pitch).min().unwrap_or(0);
            let hi = c.notes.iter().map(|n| n.pitch).max().unwrap_or(0);
            println!("   {:?} / {:?} drums={} len={} beats, {} notes, pitch {}-{}", c.track, c.name, c.drums, c.len, c.notes.len(), lo, hi);
        }
    }
}
