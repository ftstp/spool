//! A project right in the projects folder, its render next to it: the render is its reference render.
fn main() {
    let r = catalog::Rules::new(&[], &[]);
    let a = catalog::Audio { rel: "312 Bbm_104.wav".into(), size: 5_000_000, mtime: 10.0 };
    let other = catalog::Audio { rel: "Master.wav".into(), size: 5_000_000, mtime: 10.0 };
    let audio = vec![(a, String::new()), (other, String::new())];
    let key = r.variant_key("312 Bbm_104");
    let c = catalog::temoin_candidates(&r, "", &key, "312 Bbm_104", &audio, 10.0, true);
    println!("key {key:?} -> {:?}", c.iter().map(|c| (&c.rel, c.pickable)).collect::<Vec<_>>());
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].rel, "312 Bbm_104.wav");
    println!("ok");
}
