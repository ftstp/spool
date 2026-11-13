//! Songs: which song a .als belongs to, from the way projects are filed on disk.
//!
//! The SONG FOLDER carries the identity, not the file name: "electro downtmepo - Fit/Old/
//! Paris v8.als" is the song "Fit". Inside a song folder, files under old / olddies /
//! backup are superseded versions; the files left directly in it are the active ones, and
//! active files with genuinely different names (radio edit, dub version...) are separate
//! songs, called variants. Archiving is decided in the app only, never from a folder name.
//!
//! Ported from the prototype (`apf/catalog.py` and the grouping of `apf/indexer.py`).

mod board;
mod difflib;
mod group;
mod names;
mod style;
mod suggest;

pub use board::{
    BOARD_DIR, Board, CheckInput, Item, auto_checks, board_dir, card_picture, image_size, is_cover, kind_of,
    list_board, list_board_labeled, other_song_prefixes,
};
pub use difflib::SequenceMatcher;
pub use group::{Derived, FileIn, Grouping, Placed, group, song_dir_known, song_dir_of};
pub use names::{
    FolderName, Rules, artist_for, ascii_low, clean_component, is_project_dir, is_version_dir, parse_folder_name, strip_project_suffix,
    pretty_stem, similarity, split_processed, status_hint,
};
pub use style::{is_style_name, StyleData, StyleOption, StyleSong, canon, deduce};
pub use suggest::{
    Audio, Candidate, MP3_DIR, Newest, Reason, STAGES, best_stage, signature, stage_evidence, temoin_candidates, temoin_candidates_owned, own_dirs,
};
