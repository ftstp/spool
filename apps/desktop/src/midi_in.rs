//! The MIDI keyboard, read by the app itself (Windows' own MIDI input, WinMM): opened only
//! while the keyboard is on, closed when it is turned off, so Live can take it. The web
//! view's MIDI kept every input open as long as the window lived. Notes go to the page as
//! "midi-in" events: [status, data1, data2].
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

/// The inputs plugged in now, by name (an input's number changes when another is plugged).
pub fn inputs() -> Vec<String> {
    imp::inputs()
}

static OPEN: Mutex<Option<imp::Port>> = Mutex::new(None);
static HANDLE: Mutex<Option<AppHandle>> = Mutex::new(None);

fn send(msg: [u8; 3]) {
    if let Some(h) = HANDLE.lock().unwrap().as_ref() {
        let _ = h.emit("midi-in", msg);
    }
}

/// Opens the input of this name (the one open before is closed first).
pub fn open(app: &AppHandle, name: &str) -> Result<(), String> {
    *HANDLE.lock().unwrap() = Some(app.clone());
    let mut cur = OPEN.lock().unwrap();
    *cur = None;
    let i = imp::inputs().iter().position(|n| n == name).ok_or_else(|| i18n::tr!("keyboard.notFound", name = name))?;
    *cur = Some(imp::Port::open(i as u32, send)?);
    Ok(())
}

/// Closes the input: the keyboard is free for other programs.
pub fn close() {
    *OPEN.lock().unwrap() = None;
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Media::Audio::{
        CALLBACK_FUNCTION, HMIDIIN, MIDIINCAPSW, midiInClose, midiInGetDevCapsW, midiInGetNumDevs, midiInOpen, midiInReset, midiInStart,
        midiInStop,
    };
    const MIM_DATA: u32 = 0x3C3;

    pub fn inputs() -> Vec<String> {
        // SAFETY: plain WinMM calls with a buffer of the size given
        let n = unsafe { midiInGetNumDevs() };
        (0..n as usize)
            .filter_map(|i| {
                let mut caps: MIDIINCAPSW = unsafe { std::mem::zeroed() };
                let ok = unsafe { midiInGetDevCapsW(i, &mut caps, std::mem::size_of::<MIDIINCAPSW>() as u32) } == 0;
                let name = caps.szPname;
                ok.then(|| String::from_utf16_lossy(&name[..name.iter().position(|c| *c == 0).unwrap_or(name.len())]))
            })
            .collect()
    }

    type Sink = fn([u8; 3]);

    pub struct Port {
        h: HMIDIIN,
        _sink: Box<Sink>,
    }
    // the handle is only used to close it, from one place at a time (behind a Mutex)
    unsafe impl Send for Port {}

    unsafe extern "system" fn on_msg(_h: HMIDIIN, msg: u32, inst: usize, p1: usize, _p2: usize) {
        if msg == MIM_DATA && inst != 0 {
            // SAFETY: `inst` is the Box<Sink> kept alive by the Port until it is closed
            let sink = unsafe { &*(inst as *const Sink) };
            let d = p1 as u32;
            let status = (d & 0xff) as u8;
            // real-time bytes (clock, active sensing) say nothing a keyboard plays
            if status < 0xF0 {
                sink([status, ((d >> 8) & 0x7f) as u8, ((d >> 16) & 0x7f) as u8]);
            }
        }
    }

    impl Port {
        pub fn open(i: u32, sink: Sink) -> Result<Port, String> {
            let sink = Box::new(sink);
            let mut h: HMIDIIN = std::ptr::null_mut();
            // SAFETY: the callback and its instance (the boxed sink) outlive the handle
            let r = unsafe { midiInOpen(&mut h, i, on_msg as *const () as usize, &*sink as *const Sink as usize, CALLBACK_FUNCTION) };
            if r != 0 {
                // 4: MMSYSERR_ALLOCATED, another program has it
                return Err(if r == 4 { i18n::tr!("keyboard.busy") } else { i18n::tr!("keyboard.openFailed", code = r) });
            }
            unsafe { midiInStart(h) };
            Ok(Port { h, _sink: sink })
        }
    }

    impl Drop for Port {
        fn drop(&mut self) {
            // SAFETY: the handle opened above, closed once
            unsafe {
                midiInStop(self.h);
                midiInReset(self.h);
                midiInClose(self.h);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn inputs() -> Vec<String> {
        Vec::new()
    }
    pub struct Port;
    impl Port {
        pub fn open(_i: u32, _sink: fn([u8; 3])) -> Result<Port, String> {
            Err("not available".into())
        }
    }
}
