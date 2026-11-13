//! Live's own devices: XML tag -> the name shown in Live.

pub fn native_name(tag: &str) -> &str {
    match tag {
        "OriginalSimpler" => "Simpler",
        "MultiSampler" => "Sampler",
        "InstrumentVector" => "Wavetable",
        "Operator" => "Operator",
        "UltraAnalog" => "Analog",
        "LoungeLizard" => "Electric",
        "StringStudio" => "Tension",
        "Collision" => "Collision",
        "DrumGroupDevice" => "Drum Rack",
        "InstrumentMeld" => "Meld",
        "Drift" => "Drift",
        "InstrumentImpulse" => "Impulse",
        "Eq8" => "EQ Eight",
        "FilterEQ3" => "EQ Three",
        "ChannelEq" => "Channel EQ",
        "Compressor2" => "Compressor",
        "GlueCompressor" => "Glue Compressor",
        "MultibandDynamics" => "Multiband Dynamics",
        "Limiter" => "Limiter",
        "Gate" => "Gate",
        "StereoGain" => "Utility",
        "AutoFilter" => "Auto Filter",
        "AutoPan" => "Auto Pan",
        "Reverb" => "Reverb",
        "Hybrid" => "Hybrid Reverb",
        "Delay" => "Delay",
        "Echo" => "Echo",
        "PingPongDelay" => "Ping Pong Delay",
        "FilterDelay" => "Filter Delay",
        "GrainDelay" => "Grain Delay",
        "BeatRepeat" => "Beat Repeat",
        "Saturator" => "Saturator",
        "Overdrive" => "Overdrive",
        "Pedal" => "Pedal",
        "Amp" => "Amp",
        "Cabinet" => "Cabinet",
        "DrumBuss" => "Drum Buss",
        "Redux2" => "Redux",
        "Vinyl" => "Vinyl Distortion",
        "Erosion" => "Erosion",
        "Chorus2" => "Chorus-Ensemble",
        "PhaserNew" => "Phaser-Flanger",
        "Phaser" => "Phaser",
        "Flanger" => "Flanger",
        "Corpus" => "Corpus",
        "Resonator" => "Resonators",
        "FrequencyShifter" => "Frequency Shifter",
        "Shifter" => "Shifter",
        "Roar" => "Roar",
        "SpectralResonator" => "Spectral Resonator",
        "SpectralTime" => "Spectral Time",
        "Looper" => "Looper",
        "Tuner" => "Tuner",
        "SpectrumAnalyzer" => "Spectrum",
        "Vocoder" => "Vocoder",
        "Transmute" => "Transmute",
        "Tube" => "Dynamic Tube",
        "MidiArpeggiator" => "Arpeggiator",
        "MidiChord" => "Chord",
        "MidiPitcher" => "Pitch",
        "MidiRandom" => "Random",
        "MidiScale" => "Scale",
        "MidiVelocity" => "Velocity",
        "MidiNoteLength" => "Note Length",
        "MxDeviceAudioEffect" => "Max for Live (effet)",
        "MxDeviceInstrument" => "Max for Live (instrument)",
        "MxDeviceMidiEffect" => "Max for Live (MIDI)",
        other => other,
    }
}

pub fn is_native_instrument(tag: &str) -> bool {
    matches!(
        tag,
        "OriginalSimpler"
            | "MultiSampler"
            | "InstrumentVector"
            | "Operator"
            | "UltraAnalog"
            | "LoungeLizard"
            | "StringStudio"
            | "Collision"
            | "DrumGroupDevice"
            | "InstrumentMeld"
            | "Drift"
            | "InstrumentImpulse"
            | "MxDeviceInstrument"
    )
}

/// Racks and plugin wrappers: their content is counted, not the container itself.
pub fn is_container(tag: &str) -> bool {
    matches!(
        tag,
        "AudioEffectGroupDevice"
            | "InstrumentGroupDevice"
            | "MidiEffectGroupDevice"
            | "PluginDevice"
            | "AuPluginDevice"
    )
}
