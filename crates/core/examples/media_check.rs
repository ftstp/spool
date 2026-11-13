//! Loose media folders of a base (read only): media_check <base.sqlite>
fn main() {
    let st = store::Store::open(std::path::Path::new(&std::env::args().nth(1).unwrap())).unwrap();
    let mut q = st.con.prepare("select coalesce(song_dir, ''), count(*) from songs where gone=0 group by song_dir").unwrap();
    let dirs: Vec<(String, i64)> = q.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().flatten().collect();
    let t = std::time::Instant::now();
    let m = app_core::fsops::media_folders(&store::Roots::load(&st.con), &dirs);
    println!("{} folders, {} files, {} ms", m.len(), m.iter().map(|x| x.n).sum::<i64>(), t.elapsed().as_millis());
    for f in m.iter().take(25) { println!("  {:4} {:?} {}", f.n, f.counts, f.dir); }
}
