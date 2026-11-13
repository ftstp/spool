//! refs_check <set.als>: every file its devices load, and whether Live will find it here.
fn main() {
    let p = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let set = repair::xml::read_set(&p).unwrap();
    let proj = p.parent().unwrap();
    for t in &set.root.child("LiveSet").unwrap().child("Tracks").unwrap().children {
        let name = t.value("Name/EffectiveName").unwrap_or("");
        for fr in t.descendants().filter(|e| e.tag == "FileRef") {
            let path = fr.value("Path").unwrap_or("");
            let rel = fr.value("RelativePath").unwrap_or("");
            let ty = fr.value("RelativePathType").unwrap_or("");
            if path.is_empty() && rel.is_empty() { continue; }
            let here = (ty == "3" && proj.join(rel).is_file()) || std::path::Path::new(path).is_file();
            println!("{:8} {} type {} pack {:?} | {} | {}", name, if here { "OK " } else { "-- " }, ty, fr.value("LivePackName").unwrap_or(""), rel, path);
        }
    }
}
