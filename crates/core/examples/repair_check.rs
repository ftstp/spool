//! A real repair, on a COPY of one project in a scratch folder (nothing written next to the
//! user's projects): plan from the base, samples really copied into the scratch project,
//! the copy written gzipped, then read back by the .als reader.
//!   repair_check <base.sqlite> <rel> <scratch dir> <templates dir>
use std::path::{Path, PathBuf};

fn main() {
    let mut a = std::env::args().skip(1);
    let (db, rel, scratch, tpl) =
        (a.next().unwrap(), a.next().unwrap(), PathBuf::from(a.next().unwrap()), PathBuf::from(a.next().unwrap()));
    let st = store::Store::open(Path::new(&db)).unwrap();
    let root: String = st.get("root").unwrap();
    let (data, states): (String, String) = st
        .con
        .query_row("select data, plugstate from files where rel=?", [&rel], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    let p = repair::fix::plan(
        &rel,
        &serde_json::from_str(&data).unwrap(),
        &serde_json::from_str(&states).unwrap(),
        &app_core::repairs::choices(&st),
    );
    println!(
        "plan: {} renames, {} copies, {} removes, {} trials, complete {}",
        p.renames.len(),
        p.copies.len(),
        p.removes.len(),
        p.trials.len(),
        p.complete
    );

    // the project, copied: its set and its "Ableton Project Info" marker only
    let src = Path::new(&root).join(&rel);
    let proj = als::project_root(&src);
    let inside = src.strip_prefix(&proj).unwrap();
    let dst_proj = scratch.join(proj.file_name().unwrap());
    let dst = dst_proj.join(inside);
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    std::fs::copy(&src, &dst).unwrap();
    std::fs::create_dir_all(dst_proj.join("Ableton Project Info")).unwrap();

    let mut set = repair::xml::read_set(&dst).unwrap();
    let t = std::time::Instant::now();
    let done = repair::fix::repair_tree(&p, &mut set.root, &dst, &tpl, &mut repair::fix::Disk).unwrap();
    let out = repair::paths::processed_path(&dst, "R", 1);
    repair::xml::write_set(&set, &out).unwrap();
    println!("done in {:.0} ms: {done:?} -> {}", t.elapsed().as_secs_f64() * 1000.0, out.display());

    let imported: Vec<String> = std::fs::read_dir(dst_proj.join("Samples/Imported"))
        .map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    println!("samples now in the project: {}", imported.len());
    let env = als::Env::detect(None);
    let before = als::read_als(&dst, &env);
    let after = als::read_als(&out, &env);
    assert!(after.error.is_none(), "the copy does not read: {:?}", after.error);
    let names = |i: &als::AlsInfo| i.plugins.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
    println!("plugins before: {:?}", names(&before));
    println!("plugins after:  {:?}", names(&after));
    println!(
        "missing samples: before {} (in the scratch copy), after {} still missing",
        before.samples_missing.len(),
        after.samples_missing.iter().filter(|s| s.found_at.is_none()).count()
    );
    println!("tracks {} -> {}", before.tracks.values().sum::<u32>(), after.tracks.values().sum::<u32>());
}
