//! Serving a file to the interface (audio to play, pictures to show), with byte ranges: an
//! <audio> element seeks by asking for a range. Which files may be served is `App::media_path`.
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// At most this much per answer: the player asks for the rest.
const CHUNK: u64 = 4 * 1024 * 1024;

pub struct Answer {
    pub status: u16,
    pub mime: &'static str,
    /// Content-Range, for a partial answer.
    pub range: Option<String>,
    pub body: Vec<u8>,
}

pub fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        Some("ogg") => "audio/ogg",
        Some("m4a" | "aac") => "audio/mp4",
        Some("aif" | "aiff") => "audio/aiff",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("mp4") => "video/mp4",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// "bytes=a-b", "bytes=a-" or "bytes=-n" (the last n bytes) against a file of `len` bytes.
fn parse_range(h: &str, len: u64) -> Option<(u64, u64)> {
    let (a, b) = h.trim().strip_prefix("bytes=")?.split(',').next()?.trim().split_once('-')?;
    let last = len.checked_sub(1)?;
    let (a, b) = if a.is_empty() {
        let n: u64 = b.parse().ok()?;
        (len.saturating_sub(n), last)
    } else {
        let a: u64 = a.parse().ok()?;
        (a, if b.is_empty() { last } else { b.parse::<u64>().ok()?.min(last) })
    };
    (a <= b).then_some((a, b.min(a + CHUNK - 1)))
}

pub fn serve(path: &Path, range: Option<&str>) -> Answer {
    let mime = mime(path);
    let fail = |status| Answer { status, mime, range: None, body: Vec::new() };
    let Ok(mut f) = std::fs::File::open(path) else { return fail(404) };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let Some(h) = range else {
        let mut body = Vec::with_capacity(len as usize);
        return match f.read_to_end(&mut body) {
            Ok(_) => Answer { status: 200, mime, range: None, body },
            Err(_) => fail(500),
        };
    };
    match parse_range(h, len) {
        Some((a, b)) => {
            let mut body = vec![0; (b - a + 1) as usize];
            if f.seek(SeekFrom::Start(a)).and_then(|_| f.read_exact(&mut body)).is_err() {
                return fail(500);
            }
            Answer { status: 206, mime, range: Some(format!("bytes {a}-{b}/{len}")), body }
        }
        None => Answer { status: 416, mime, range: Some(format!("bytes */{len}")), body: Vec::new() },
    }
}

/// "%20" and friends in a URL path.
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = (b[i] == b'%' && i + 2 < b.len())
            .then(|| std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()))
            .flatten();
        if let Some(v) = hex {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-", 10), Some((0, 9)));
        assert_eq!(parse_range("bytes=2-4", 10), Some((2, 4)));
        assert_eq!(parse_range("bytes=8-50", 10), Some((8, 9)));
        assert_eq!(parse_range("bytes=-3", 10), Some((7, 9)));
        assert_eq!(parse_range("bytes=12-", 10), None);
        assert_eq!(parse_range("bytes=0-", 0), None);
        assert_eq!(parse_range("bytes=0-", 10 * CHUNK), Some((0, CHUNK - 1)));
    }

    #[test]
    fn serves_a_slice() {
        let dir = std::env::temp_dir().join(format!("pf-media-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a b.mp3");
        std::fs::write(&p, b"0123456789").unwrap();
        let a = serve(&p, Some("bytes=3-5"));
        assert_eq!(
            (a.status, a.mime, a.range.as_deref(), a.body.as_slice()),
            (206, "audio/mpeg", Some("bytes 3-5/10"), &b"345"[..])
        );
        assert_eq!(serve(&p, None).body.len(), 10);
        assert_eq!(serve(&p, Some("bytes=20-")).status, 416);
        assert_eq!(serve(&dir.join("none.mp3"), None).status, 404);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(percent_decode("a%20b/%C3%A9t%C3%A9.wav"), "a b/été.wav");
        assert_eq!(percent_decode("100%"), "100%");
    }
}

/// A small JPEG of a picture (`size` px on its long side), made once and kept in `cache`
/// (named after the file's path, size and date, so an edited picture gets a new one). The
/// picture itself is only read.
pub fn thumb(path: &Path, cache: &Path, size: u32) -> Option<Vec<u8>> {
    use std::hash::{Hash, Hasher};
    let md = std::fs::metadata(path).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    md.len().hash(&mut h);
    md.modified().ok().hash(&mut h);
    size.hash(&mut h);
    let file = cache.join(format!("{:016x}.jpg", h.finish()));
    if let Ok(b) = std::fs::read(&file) {
        return Some(b);
    }
    let img = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.decode().ok()?;
    let small = img.thumbnail(size, size).to_rgb8();
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82).encode_image(&small).ok()?;
    let bytes = out.into_inner();
    let _ = std::fs::create_dir_all(cache);
    let _ = std::fs::write(&file, &bytes);
    Some(bytes)
}

#[cfg(test)]
mod thumb_tests {
    #[test]
    fn makes_and_keeps_a_small_jpeg() {
        let dir = std::env::temp_dir().join(format!("spool-thumb-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let src = dir.join("big.png");
        image::RgbImage::from_pixel(1600, 900, image::Rgb([200, 40, 30])).save(&src).unwrap();
        let before = std::fs::read(&src).unwrap();
        let t = super::thumb(&src, &dir.join("cache"), 240).unwrap();
        let img = image::load_from_memory(&t).unwrap();
        assert_eq!((img.width(), img.height()), (240, 135));
        assert_eq!(std::fs::read_dir(dir.join("cache")).unwrap().count(), 1);
        assert_eq!(super::thumb(&src, &dir.join("cache"), 240).unwrap(), t); // kept
        assert_eq!(std::fs::read(&src).unwrap(), before); // the picture untouched
        let _ = std::fs::remove_dir_all(&dir);
    }
}
