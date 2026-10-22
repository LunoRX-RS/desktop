//! Requested PCM format and negotiated audio API format are separate facts.
use libmpv2::Mpv;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AudioQuality {
    Low,
    Normal,
    #[default]
    High,
}
impl AudioQuality {
    pub fn bitrate_limit(self) -> u32 {
        match self {
            Self::Low => 64_000,
            Self::Normal => 128_000,
            Self::High => u32::MAX,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PcmFormat {
    Int16,
    Int32,
    #[default]
    Float32,
}
impl PcmFormat {
    fn mpv(self) -> &'static str {
        match self {
            Self::Int16 => "s16",
            Self::Int32 => "s32",
            Self::Float32 => "float",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SampleRate {
    #[default]
    Auto,
    Hz44100,
    Hz48000,
    Hz96000,
    Hz192000,
}
impl SampleRate {
    fn hz(self) -> i64 {
        match self {
            Self::Auto => 0,
            Self::Hz44100 => 44100,
            Self::Hz48000 => 48000,
            Self::Hz96000 => 96000,
            Self::Hz192000 => 192000,
        }
    }
}
/// Loudness is applied as an mpv `af` chain. mpv applies `volume-gain` after `af`, so a gain
/// placed there would land behind the limiter. The gain therefore lives inside the chain.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Loudness {
    Off,
    #[default]
    Balanced,
    Loud,
}
impl Loudness {
    /// dynaudnorm levels quiet and loud parts toward a common peak (0.9, max 18 dB gain).
    /// Loud adds +4 dB, then alimiter caps peaks at 0.89 (about -1 dBFS) with auto-level off.
    pub fn filter(self) -> &'static str {
        match self {
            Self::Off => "",
            Self::Balanced => "lavfi=[dynaudnorm=f=500:g=31:p=0.9:m=8:s=0]",
            Self::Loud => {
                "lavfi=[dynaudnorm=f=500:g=31:p=0.9:m=8:s=0,volume=4dB,alimiter=limit=0.89:level=0]"
            }
        }
    }
    pub fn apply(self, mpv: &Mpv) -> libmpv2::Result<()> {
        mpv.set_property("af", self.filter())
    }
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioSettings {
    pub quality: AudioQuality,
    pub pcm_format: PcmFormat,
    pub sample_rate: SampleRate,
    #[serde(default)]
    pub loudness: Loudness,
}
impl AudioSettings {
    pub fn apply(self, mpv: &Mpv) -> libmpv2::Result<()> {
        mpv.set_property("options/audio-format", self.pcm_format.mpv())?;
        mpv.set_property("options/audio-samplerate", self.sample_rate.hz())
    }
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioOutput {
    pub driver: Option<String>,
    pub format: Option<String>,
    pub sample_rate: Option<i64>,
    pub source_format: Option<String>,
    pub source_sample_rate: Option<i64>,
}
#[derive(Clone, Default)]
pub struct AudioControl {
    pub settings: Arc<Mutex<AudioSettings>>,
    pub output: Arc<Mutex<AudioOutput>>,
}
impl AudioControl {
    pub fn settings(&self) -> AudioSettings {
        *self.settings.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn observe(&self, mpv: &Mpv) {
        *self.output.lock().unwrap_or_else(|e| e.into_inner()) = AudioOutput {
            driver: mpv.get_property("current-ao").ok(),
            format: mpv.get_property("audio-out-params/format").ok(),
            sample_rate: mpv.get_property("audio-out-params/samplerate").ok(),
            source_format: mpv.get_property("audio-params/format").ok(),
            source_sample_rate: mpv.get_property("audio-params/samplerate").ok(),
        };
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mpv_accepts_all_requested_pcm_formats_and_rates() {
        let mpv = Mpv::with_initializer(|init| {
            init.set_option("config", "no")?;
            init.set_option("ao", "null")
        })
        .unwrap();
        for pcm_format in [PcmFormat::Int16, PcmFormat::Int32, PcmFormat::Float32] {
            for sample_rate in [
                SampleRate::Auto,
                SampleRate::Hz44100,
                SampleRate::Hz48000,
                SampleRate::Hz96000,
                SampleRate::Hz192000,
            ] {
                AudioSettings {
                    quality: AudioQuality::High,
                    pcm_format,
                    sample_rate,
                    loudness: Loudness::Off,
                }
                .apply(&mpv)
                .unwrap();
                assert_eq!(
                    mpv.get_property::<i64>("options/audio-samplerate").unwrap(),
                    sample_rate.hz()
                );
            }
        }
    }
    #[test]
    fn settings_accept_only_supported_formats_and_rates() {
        let mut value = serde_json::to_value(AudioSettings::default()).unwrap();
        assert_eq!(
            serde_json::from_value::<AudioSettings>(value.clone()).unwrap(),
            AudioSettings::default()
        );
        for bad in ["s24", "--script=evil", "int64"] {
            value["pcmFormat"] = bad.into();
            assert!(serde_json::from_value::<AudioSettings>(value.clone()).is_err());
        }
        value = serde_json::to_value(AudioSettings::default()).unwrap();
        value["sampleRate"] = "hz999999".into();
        assert!(serde_json::from_value::<AudioSettings>(value).is_err());
        assert_eq!(PcmFormat::Float32.mpv(), "float");
        assert_eq!(SampleRate::Hz192000.hz(), 192000);
    }
    #[test]
    fn loudness_defaults_to_balanced_and_old_payloads_still_parse() {
        let old = serde_json::json!({"quality":"high","pcmFormat":"float32","sampleRate":"auto"});
        let parsed: AudioSettings = serde_json::from_value(old).unwrap();
        assert_eq!(parsed.loudness, Loudness::Balanced);
        let json = serde_json::to_value(parsed).unwrap();
        assert_eq!(json["loudness"], "balanced");
        for bad in ["quiet", "lavfi=[evil]", "LOUD"] {
            let mut v = json.clone();
            v["loudness"] = bad.into();
            assert!(serde_json::from_value::<AudioSettings>(v).is_err());
        }
    }
    #[test]
    fn loudness_maps_to_filters_and_mpv_reads_them_back() {
        assert_eq!(Loudness::Off.filter(), "");
        assert!(Loudness::Balanced.filter().contains("dynaudnorm"));
        assert!(!Loudness::Balanced.filter().contains("volume="));
        let loud = Loudness::Loud.filter();
        assert!(loud.contains("dynaudnorm") && loud.contains("volume=4dB"));
        assert!(
            loud.find("volume=4dB") < loud.find("alimiter"),
            "limiter must follow the gain"
        );
        let mpv = Mpv::with_initializer(|init| {
            init.set_option("config", "no")?;
            init.set_option("ao", "null")
        })
        .unwrap();
        for l in [Loudness::Loud, Loudness::Balanced, Loudness::Off] {
            l.apply(&mpv).unwrap();
            let af: String = mpv.get_property("af").unwrap();
            assert_eq!(af.contains("dynaudnorm"), l != Loudness::Off, "{af}");
            assert_eq!(af.contains("alimiter"), l == Loudness::Loud, "{af}");
        }
    }
}
