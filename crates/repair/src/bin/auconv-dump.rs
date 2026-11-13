//! For comparisons with the prototype: every Mac (AU) device of the given sets converted onto
//! one VST2 template, as {"rel#n": the device written, or null}.
//!   auconv-dump <root> <template.xml> <rel>...
use std::path::Path;

fn main() {
    let mut a = std::env::args().skip(1);
    let (root, tpl) = (a.next().expect("root"), a.next().expect("template"));
    let tpl = std::fs::read_to_string(tpl).unwrap();
    let target =
        repair::auconv::Target { to: "Converted".into(), file: "Converted.dll".into(), id: "1095001651".into() };
    let mut out = std::collections::BTreeMap::new();
    for rel in a {
        let Ok(set) = repair::xml::read_set(&Path::new(&root).join(&rel)) else { continue };
        for (i, dev) in set.root.descendants().filter(|e| e.tag == "AuPluginDevice").enumerate() {
            let name = dev.find(".//AuPluginInfo").and_then(|x| x.value("Name")).unwrap_or("").to_string();
            let v = repair::auconv::convert(dev, &name, &tpl, &target).map(|e| {
                let mut s = String::new();
                repair::xml::serialize(&e, &mut s);
                s
            });
            out.insert(format!("{rel}#{i}"), v);
        }
    }
    println!("{}", serde_json::to_string(&out).unwrap());
}
