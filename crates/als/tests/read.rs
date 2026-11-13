use als::{Env, read_als_bytes, refine_mode};
use std::path::Path;

fn hist(notes: &[(usize, f64)]) -> [f64; 12] {
    let mut h = [0.0; 12];
    for &(n, w) in notes {
        h[n % 12] = w;
    }
    h
}

#[test]
fn dorian_from_its_major_sixth() {
    let h = hist(&[(2, 10.0), (5, 6.0), (9, 7.0), (4, 3.0), (7, 3.0), (0, 3.0), (11, 4.0)]);
    assert_eq!(refine_mode(&h, 2, true), Some("Dorian"));
}

#[test]
fn plain_minor_when_unclear() {
    let h = hist(&[(2, 10.0), (5, 6.0), (9, 7.0), (4, 3.0), (7, 3.0), (0, 3.0), (11, 2.0), (10, 2.0)]);
    assert_eq!(refine_mode(&h, 2, true), None);
}

#[test]
fn contradicting_modes_give_nothing() {
    let h = hist(&[(2, 10.0), (5, 6.0), (9, 7.0), (11, 4.0), (3, 4.0)]);
    assert_eq!(refine_mode(&h, 2, true), None);
}

#[test]
fn lydian() {
    let h = hist(&[(0, 10.0), (4, 6.0), (7, 7.0), (6, 4.0), (2, 3.0), (9, 3.0), (11, 3.0)]);
    assert_eq!(refine_mode(&h, 0, false), Some("Lydian"));
}

/// A minimal Live 12 set: tempo, 4/4, a Live scale, a MIDI bass line in E minor,
/// two native devices and a VST2 with a readable preset name.
const SET: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton Creator="Ableton Live 12.1.5">
 <LiveSet>
  <InKey Value="true"/>
  <ScaleInformation><Root Value="4"/><Name Value="1"/></ScaleInformation>
  <Tracks>
   <MidiTrack>
    <Name><EffectiveName Value="Sub bass"/><UserName Value="Sub bass"/></Name>
    <DeviceChain><DeviceChain><Devices>
      <Eq8/>
      <PluginDevice><PluginDesc><VstPluginInfo>
        <PlugName Value="FabFilter Pro-Q 3"/><UniqueId Value="1180000000"/><Path Value="C:/VST/Pro-Q 3.dll"/>
        <Preset><VstPreset><Buffer>4C6F7720637574000000000000000000000000</Buffer></VstPreset></Preset>
      </VstPluginInfo></PluginDesc></PluginDevice>
    </Devices></DeviceChain>
    <MainSequencer><ClipTimeable><ArrangerAutomation><Events>
     <MidiClip><CurrentEnd Value="64"/><Name Value="bassline"/><Notes><KeyTracks>
      KEYTRACKS
     </KeyTracks></Notes></MidiClip>
    </Events></ArrangerAutomation></ClipTimeable></MainSequencer></DeviceChain>
   </MidiTrack>
  </Tracks>
  <MainTrack><DeviceChain><Mixer><Tempo><Manual Value="174"/></Tempo><TimeSignature><Manual Value="201"/></TimeSignature></Mixer>
   <DeviceChain><Devices><Limiter/></Devices></DeviceChain></DeviceChain></MainTrack>
 </LiveSet>
</Ableton>"#;

fn set_with_notes() -> String {
    // E minor: E, G, B, D, plus some A and C; the bass sits on E
    let mut kts = String::new();
    for (pitch, n) in [(28, 12), (31, 6), (35, 6), (38, 4), (33, 3), (36, 3)] {
        let events: String =
            (0..n).map(|i| format!(r#"<MidiNoteEvent Time="{i}" Duration="1" IsEnabled="true"/>"#)).collect();
        kts += &format!(r#"<KeyTrack><Notes>{events}</Notes><MidiKey Value="{pitch}"/></KeyTrack>"#);
    }
    SET.replace("KEYTRACKS", &kts)
}

#[test]
fn reads_the_basics() {
    let xml = set_with_notes();
    let info = read_als_bytes(xml.as_bytes(), Path::new("C:/x/Song.als"), &Env::default());
    assert_eq!(info.error, None);
    assert_eq!(info.live_version.as_deref(), Some("12.1.5"));
    assert_eq!(info.tempo, Some(174.0));
    assert_eq!(info.time_sig.as_deref(), Some("4/4"));
    assert_eq!(info.key.as_deref(), Some("E Minor"));
    assert_eq!(info.length_s, Some(22)); // 64 beats at 174 bpm
    assert_eq!(info.tracks.get("midi"), Some(&1));
    assert_eq!(info.key_est, Some(Some("Em".into())));
    assert_eq!(info.key_bass, Some(true));
    let names: Vec<_> = info.devices.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["EQ Eight", "Limiter"]);
    assert_eq!(info.plugins.len(), 1);
    assert_eq!(info.plugins[0].preset.as_ref().map(|p| p.name.as_str()), Some("Low cut"));
    assert_eq!(info.main_chain, ["Limiter"]);
    assert_eq!(info.names, ["Sub bass", "bassline"]);
}

#[test]
fn a_broken_file_reports_an_error() {
    let info = read_als_bytes(b"<Ableton><Nope/></Ableton>", Path::new("x.als"), &Env::default());
    assert!(info.error.is_some());
}
