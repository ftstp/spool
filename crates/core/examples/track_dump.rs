//! track_dump <set.als>: what each track is made of (read-only).
fn main() {
    let p = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let info = als::read_als(&p, &als::Env::detect(None));
    println!("format {:?} locators {:?}", info.format, info.locators);
    for t in &info.track_info {
        println!("{:>4} {:6} {:7} g{:>4} {:28} inst={:?} plug={} samp={}/{} notes={} aclips={} sc={} chain={:?}",
            t.id, t.kind, t.role, t.group, t.name.chars().take(28).collect::<String>(), t.instrument, t.plugins.len(), t.samples, t.samples_missing, t.notes, t.audio_clips, t.sidechain, t.chain);
    }
}
