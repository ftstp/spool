//! Every project file (.als) as its own row (the Songs page's "Projects" view): a catalogue
//! grouped one song a folder still shows each project saved, to find old ideas again.
use serde::Serialize;
use store::rusqlite::{self, Connection};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ProjectFile {
    /// Root-relative path.
    pub rel: String,
    #[ts(type = "number | null")]
    pub song: Option<i64>,
    pub title: Option<String>,
    pub mtime: f64,
    #[ts(type = "number")]
    pub size: i64,
    /// An older version of its song (superseded), or archived.
    pub old: bool,
    pub archived: bool,
    pub tempo: Option<f64>,
    pub key: Option<String>,
    /// Length of its arrangement, seconds.
    pub len: Option<f64>,
    /// 0 opens as it is, 1 the app repairs it, 2 settings lost, 3 something to install.
    #[ts(type = "number")]
    pub sev: i64,
}

pub fn project_files(con: &Connection) -> rusqlite::Result<Vec<ProjectFile>> {
    let mut q = con.prepare(
        "select f.rel, f.song_id, s.title, coalesce(f.mtime, 0), coalesce(f.size, 0), coalesce(f.in_version, 0), coalesce(f.archived, 0) or coalesce(s.archived, 0),          json_extract(unz(f.data), '$.tempo'), json_extract(unz(f.data), '$.key'), json_extract(unz(f.data), '$.length_s'), coalesce(f.sev, 0)          from files f left join songs s on s.id = f.song_id where coalesce(s.gone, 0) = 0 order by f.mtime desc",
    )?;
    q.query_map([], |r| {
        Ok(ProjectFile {
            rel: r.get(0)?,
            song: r.get(1)?,
            title: r.get(2)?,
            mtime: r.get(3)?,
            size: r.get(4)?,
            old: r.get::<_, i64>(5)? != 0,
            archived: r.get::<_, i64>(6)? != 0,
            tempo: r.get(7)?,
            key: r.get(8)?,
            len: r.get(9)?,
            sev: r.get(10)?,
        })
    })?
    .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_project_its_row() {
        let st = store::Store::memory().unwrap();
        st.con
            .execute_batch(
                r#"insert into songs(id, title, gone, archived) values (1, 'Idea', 0, 0);
                   insert into files(rel, song_id, size, mtime, in_version, data) values
                   ('A/Idea.als', 1, 10, 2, 0, '{"tempo": 128, "key": "Am", "length_s": 90}'),
                   ('A/Idea old.als', 1, 9, 1, 1, '{"tempo": 120}');"#,
            )
            .unwrap();
        let l = super::project_files(&st.con).unwrap();
        assert_eq!(l.iter().map(|p| (p.rel.as_str(), p.old)).collect::<Vec<_>>(), [("A/Idea.als", false), ("A/Idea old.als", true)]);
        assert_eq!((l[0].tempo, l[0].key.as_deref()), (Some(128.0), Some("Am")));
    }
}
