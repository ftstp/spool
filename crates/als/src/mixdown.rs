//! What a track's mixer and a few plain devices do to its level and its tone, read from the
//! set so the app's playback of an arrangement sounds like it: the fader, the track's on/off,
//! Utility's gain and mute, and clear cuts (EQ Eight's low/high cut bands, Auto Filter's
//! low/high pass), each with its arrangement automation. Nothing else of the processing
//! (compressors, reverbs, saturation, plugins) is reproduced, and the main track is left out on
//! purpose: the app levels its output itself.
use crate::xml::{N, child, children, path, tag, val};
use serde::Serialize;
use std::collections::HashMap;
use ts_rs::TS;

/// Points in time, (beat, value), linear between two; a jump is two points at the same beat.
pub type Curve = Vec<(f64, f64)>;

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct TrackMix {
    /// Its level as set, linear (1: 0 dB): the fader times its Utilities, the automated ones
    /// left out (they are in `envs`).
    pub gain: f64,
    /// Multipliers moving in time (fader, on/off, Utility gain or mute automation).
    #[ts(type = "Array<Array<[number, number]>>")]
    pub envs: Vec<Curve>,
    /// Clear cuts, in the order of the devices.
    pub filters: Vec<MixFilter>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct MixFilter {
    /// "lp" (high cut) or "hp" (low cut).
    pub kind: String,
    /// Its frequency in Hz, as set.
    pub hz: f64,
    /// Its frequency moving in time, in Hz (empty: it stays at `hz`).
    #[ts(type = "Array<[number, number]>")]
    pub env: Curve,
    /// Steepness: 12, 24 or 48 dB per octave.
    pub slope: u8,
}

/// Live writes "before the song" as this time: the value from the start.
const BEFORE: f64 = -63_072_000.0;

/// The track's automation lanes, by the id of the parameter they move.
fn envelopes(t: N) -> HashMap<String, Curve> {
    let mut out = HashMap::new();
    let Some(envs) = path(t, "AutomationEnvelopes/Envelopes") else { return out };
    for e in children(envs) {
        let Some(id) = val(Some(e), "EnvelopeTarget/PointeeId") else { continue };
        let Some(events) = path(e, "Automation/Events") else { continue };
        let mut pts: Curve = Vec::new();
        let mut bool_lane = false;
        for ev in children(events) {
            let Some(time) = ev.attribute("Time").and_then(|v| v.trim().parse::<f64>().ok()) else { continue };
            let v = match tag(ev) {
                "BoolEvent" => {
                    bool_lane = true;
                    if ev.attribute("Value") == Some("true") { 1.0 } else { 0.0 }
                }
                "FloatEvent" | "EnumEvent" => match ev.attribute("Value").and_then(|v| v.trim().parse::<f64>().ok()) {
                    Some(v) => v,
                    None => continue,
                },
                _ => continue,
            };
            let at = if time <= BEFORE + 1.0 { 0.0 } else { time.max(0.0) };
            // an on/off lane jumps: the value before holds until the new one
            if bool_lane
                && let Some(&(_, prev)) = pts.last()
                && prev != v
            {
                pts.push((at, prev));
            }
            pts.push((at, v));
        }
        if pts.len() > 1 || pts.first().is_some_and(|p| p.0 > 0.0) {
            pts.sort_by(|a, b| a.0.total_cmp(&b.0));
            pts.truncate(2048);
            out.insert(id.to_string(), pts);
        }
    }
    out
}

/// A parameter: its set value and, when automated, its lane.
fn param<'a>(p: Option<N<'a, '_>>, lanes: &HashMap<String, Curve>) -> (Option<&'a str>, Option<Curve>) {
    let Some(p) = p else { return (None, None) };
    let manual = val(Some(p), "Manual");
    let lane = child(p, "AutomationTarget").and_then(|a| a.attribute("Id")).and_then(|id| lanes.get(id)).cloned();
    (manual, lane)
}

fn f(v: Option<&str>) -> Option<f64> {
    v.and_then(|x| x.trim().parse().ok())
}
fn on(v: Option<&str>) -> bool {
    v != Some("false")
}

/// Auto Filter's cutoff (20 to 135, a note number) in Hz.
fn note_hz(v: f64) -> f64 {
    440.0 * 2f64.powf((v - 69.0) / 12.0)
}

/// A level parameter in dB (older Utility) or linear (Live 12): its range says which.
fn level(p: Option<N>, v: f64) -> f64 {
    let db = f(p.and_then(|p| val(Some(p), "MidiControllerRange/Min"))).is_some_and(|m| m < 0.0);
    if db { 10f64.powf(v / 20.0) } else { v.max(0.0) }
}

pub fn track_mix(t: N) -> TrackMix {
    let lanes = envelopes(t);
    let mut m = TrackMix { gain: 1.0, ..Default::default() };
    let mixer = path(t, "DeviceChain/Mixer");
    // the fader
    let vol = mixer.and_then(|x| child(x, "Volume"));
    match param(vol, &lanes) {
        (_, Some(lane)) => m.envs.push(lane),
        (v, None) => m.gain *= f(v).unwrap_or(1.0).max(0.0),
    }
    // the track's on/off, when automated (switched off for good: `muted`)
    if let (_, Some(lane)) = param(mixer.and_then(|x| child(x, "Speaker")), &lanes) {
        m.envs.push(lane);
    }
    let Some(devices) = path(t, "DeviceChain/DeviceChain/Devices") else { return m };
    for d in children(devices) {
        if !on(val(Some(d), "On/Manual")) {
            continue;
        }
        match tag(d) {
            "StereoGain" => {
                let g = child(d, "Gain");
                match param(g, &lanes) {
                    (_, Some(lane)) => m.envs.push(lane.into_iter().map(|(b, v)| (b, level(g, v))).collect()),
                    (v, None) => m.gain *= f(v).map(|v| level(g, v)).unwrap_or(1.0),
                }
                match param(child(d, "Mute"), &lanes) {
                    (_, Some(lane)) => m.envs.push(lane.into_iter().map(|(b, v)| (b, 1.0 - v)).collect()),
                    (v, None) if v == Some("true") => m.gain = 0.0,
                    _ => {}
                }
            }
            "Eq8" => {
                for b in 0..8 {
                    let Some(band) = path(d, &format!("Bands.{b}/ParameterA")) else { continue };
                    if !on(val(Some(band), "IsOn/Manual")) {
                        continue;
                    }
                    let (kind, slope) = match f(val(Some(band), "Mode/Manual")).map(|x| x as i64) {
                        Some(0) => ("hp", 48),
                        Some(1) => ("hp", 12),
                        Some(6) => ("lp", 12),
                        Some(7) => ("lp", 48),
                        _ => continue,
                    };
                    let (hz, lane) = param(child(band, "Freq"), &lanes);
                    push_filter(&mut m, kind, f(hz).unwrap_or(1000.0), lane.unwrap_or_default(), slope);
                }
            }
            "AutoFilter" => {
                // Live 12's Auto Filter: 0 low pass, 1 high pass (band, notch and morph left out)
                let kind = match f(val(Some(d), "FilterType/Manual")).map(|x| x as i64) {
                    Some(0) => "lp",
                    Some(1) => "hp",
                    _ => continue,
                };
                let (cut, lane) = param(child(d, "Cutoff"), &lanes);
                let slope = if val(Some(d), "Slope/Manual") == Some("true") { 24 } else { 12 };
                let lane: Curve = lane.unwrap_or_default().into_iter().map(|(b, v)| (b, note_hz(v))).collect();
                push_filter(&mut m, kind, note_hz(f(cut).unwrap_or(135.0)), lane, slope);
            }
            _ => {}
        }
    }
    m
}

/// A cut that does something: one that stays out of the audible range is left out.
fn push_filter(m: &mut TrackMix, kind: &str, hz: f64, env: Curve, slope: u8) {
    let hz = hz.clamp(10.0, 22_000.0);
    let idle = |x: f64| if kind == "lp" { x >= 18_000.0 } else { x <= 25.0 };
    if env.is_empty() && idle(hz) {
        return;
    }
    if !env.is_empty() && env.iter().all(|p| idle(p.1)) {
        return;
    }
    m.filters.push(MixFilter { kind: kind.into(), hz, env, slope });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mix(xml: &str) -> TrackMix {
        let doc = roxmltree::Document::parse(xml).unwrap();
        track_mix(doc.root_element())
    }

    #[test]
    fn fader_utility_cuts_and_their_automation() {
        let m = mix(r#"<AudioTrack>
          <AutomationEnvelopes><Envelopes>
            <AutomationEnvelope><EnvelopeTarget><PointeeId Value="10" /></EnvelopeTarget><Automation><Events>
              <FloatEvent Time="-63072000" Value="1" /><FloatEvent Time="16" Value="1" /><FloatEvent Time="32" Value="0" />
            </Events></Automation></AutomationEnvelope>
            <AutomationEnvelope><EnvelopeTarget><PointeeId Value="30" /></EnvelopeTarget><Automation><Events>
              <FloatEvent Time="-63072000" Value="135" /><FloatEvent Time="8" Value="69" />
            </Events></Automation></AutomationEnvelope>
            <AutomationEnvelope><EnvelopeTarget><PointeeId Value="21" /></EnvelopeTarget><Automation><Events>
              <BoolEvent Time="-63072000" Value="false" /><BoolEvent Time="64" Value="true" />
            </Events></Automation></AutomationEnvelope>
          </Envelopes></AutomationEnvelopes>
          <DeviceChain>
            <Mixer><Volume><Manual Value="0.5" /><AutomationTarget Id="10" /></Volume><Speaker><Manual Value="true" /><AutomationTarget Id="11" /></Speaker></Mixer>
            <DeviceChain><Devices>
              <StereoGain><On><Manual Value="true" /></On>
                <Gain><Manual Value="0.25" /><MidiControllerRange><Min Value="0" /><Max Value="56.2341309" /></MidiControllerRange><AutomationTarget Id="20" /></Gain>
                <Mute><Manual Value="false" /><AutomationTarget Id="21" /></Mute></StereoGain>
              <Eq8><On><Manual Value="true" /></On>
                <Bands.0><ParameterA><IsOn><Manual Value="true" /></IsOn><Mode><Manual Value="1" /></Mode><Freq><Manual Value="20" /><AutomationTarget Id="40" /></Freq></ParameterA></Bands.0>
                <Bands.7><ParameterA><IsOn><Manual Value="true" /></IsOn><Mode><Manual Value="7" /></Mode><Freq><Manual Value="800" /><AutomationTarget Id="41" /></Freq></ParameterA></Bands.7>
              </Eq8>
              <AutoFilter><On><Manual Value="true" /></On><FilterType><Manual Value="0" /></FilterType><Slope><Manual Value="true" /></Slope>
                <Cutoff><Manual Value="135" /><AutomationTarget Id="30" /></Cutoff></AutoFilter>
              <AutoFilter><On><Manual Value="false" /></On><FilterType><Manual Value="1" /></FilterType><Cutoff><Manual Value="100" /></Cutoff></AutoFilter>
            </Devices></DeviceChain>
          </DeviceChain></AudioTrack>"#);
        assert!((m.gain - 0.25).abs() < 1e-9, "the automated fader leaves `gain`, the Utility stays: {}", m.gain);
        assert_eq!(m.envs.len(), 2, "the fader's lane and the Utility's mute lane");
        assert_eq!(m.envs[0], vec![(0.0, 1.0), (16.0, 1.0), (32.0, 0.0)]);
        assert_eq!(m.envs[1], vec![(0.0, 1.0), (64.0, 1.0), (64.0, 0.0)], "muted at 64: a jump to silence");
        let kinds: Vec<(&str, u8)> = m.filters.iter().map(|x| (x.kind.as_str(), x.slope)).collect();
        assert_eq!(kinds, [("lp", 48), ("lp", 24)], "the 20 Hz low cut does nothing, the switched-off filter neither");
        assert_eq!(m.filters[0].hz, 800.0);
        assert!((m.filters[1].env[1].1 - 440.0).abs() < 1e-6, "Auto Filter's cutoff 69 is 440 Hz");
    }

    #[test]
    fn older_utility_in_db_and_a_muted_one() {
        let m = mix(r#"<MidiTrack><DeviceChain><Mixer><Volume><Manual Value="1" /></Volume></Mixer><DeviceChain><Devices>
            <StereoGain><On><Manual Value="true" /></On><Gain><Manual Value="-6" /><MidiControllerRange><Min Value="-35" /><Max Value="35" /></MidiControllerRange></Gain></StereoGain>
          </Devices></DeviceChain></DeviceChain></MidiTrack>"#);
        assert!((m.gain - 0.501).abs() < 0.001);
        let m = mix(r#"<MidiTrack><DeviceChain><DeviceChain><Devices>
            <StereoGain><On><Manual Value="true" /></On><Mute><Manual Value="true" /></Mute></StereoGain>
          </Devices></DeviceChain></DeviceChain></MidiTrack>"#);
        assert_eq!(m.gain, 0.0);
    }
}
