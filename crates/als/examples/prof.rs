use std::io::Read;
use std::time::Instant;
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let t = Instant::now();
    let mut raw = Vec::new();
    flate2::read::MultiGzDecoder::new(std::fs::File::open(&p).unwrap()).read_to_end(&mut raw).unwrap();
    println!("gunzip {:?} ({} MB)", t.elapsed(), raw.len() / 1_000_000);
    let t = Instant::now();
    let text = std::str::from_utf8(&raw).unwrap();
    let doc = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions { allow_dtd: true, nodes_limit: u32::MAX },
    )
    .unwrap();
    println!("parse {:?} ({} nodes)", t.elapsed(), doc.descendants().count());
    let tracks = doc
        .root_element()
        .children()
        .find(|c| c.has_tag_name("LiveSet"))
        .unwrap()
        .children()
        .find(|c| c.has_tag_name("Tracks"))
        .unwrap();
    let t = Instant::now();
    let k = als::estimate_key(tracks);
    println!("key {:?} {:?}", t.elapsed(), k.key);
    let t = Instant::now();
    for tr in tracks.children().filter(|c| c.is_element()) {
        let _ = als::track_role(tr);
    }
    println!("roles {:?}", t.elapsed());
    let t = Instant::now();
    let _ = als::read_als_bytes(&raw, std::path::Path::new(&p), &als::Env::detect(Some("D".into())));
    println!("full {:?}", t.elapsed());
}
