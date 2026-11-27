//! The Artists page's "who is it?" on a COPY of a data folder: a name made a guest, another
//! made an alias then one of the user's names again, a suggestion said not to be an artist.
//!     role_check <data folder copy> <guest> <alias> <alias of>
fn tally(app: &app_core::App, name: &str) -> (usize, usize) {
    let songs = app.songs().unwrap();
    let mine = songs.iter().filter(|s| s.artist.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(name))).count();
    let ft = songs.iter().filter(|s| s.ft.as_deref().is_some_and(|f| f.split(',').any(|x| x.trim().eq_ignore_ascii_case(name)))).count();
    (mine, ft)
}

fn main() {
    let mut a = std::env::args().skip(1);
    let dir = a.next().expect("data folder (a copy)");
    let (guest, alias, of) = (a.next().unwrap(), a.next().unwrap(), a.next().unwrap());
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    let ft_songs = |app: &app_core::App| app.songs().unwrap().iter().filter(|s| s.ft.as_deref().is_some_and(|f| !f.trim().is_empty())).count();
    eprintln!("songs with a featuring: {}", ft_songs(&app));
    let sugg = app.artist_suggestions().unwrap();
    eprintln!("to sort: {} {:?}", sugg.len(), sugg.iter().take(8).map(|g| g.name.clone()).collect::<Vec<_>>());
    eprintln!("{guest}: {:?} (songs, featured on)", tally(&app, &guest));
    app.artist_role(&guest, "guest", "").unwrap();
    eprintln!("{guest} a guest: {:?}; with a featuring now: {}", tally(&app, &guest), ft_songs(&app));
    app.artist_role(&guest, "me", "").unwrap();
    eprintln!("{guest} mine again: {:?}", tally(&app, &guest));
    eprintln!("{alias}: {:?}, {of}: {:?}", tally(&app, &alias), tally(&app, &of));
    app.artist_role(&alias, "alias", &of).unwrap();
    eprintln!("{alias} read as {of}: {:?}, {of}: {:?}; aliases {:?}", tally(&app, &alias), tally(&app, &of), app.config().artist_aliases);
    app.artist_role(&alias, "me", "").unwrap();
    eprintln!("{alias} mine again: {:?}, {of}: {:?}; aliases {:?}", tally(&app, &alias), tally(&app, &of), app.config().artist_aliases);
    if let Some(g) = sugg.first() {
        app.artist_role(&g.name, "none", "").unwrap();
        let after = app.artist_suggestions().unwrap();
        eprintln!("{} not an artist: still to sort? {}; to sort now {}", g.name, after.iter().any(|x| x.name == g.name), after.len());
    }
}
