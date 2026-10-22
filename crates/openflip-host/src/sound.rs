//! The sound of a flap striking its card: twelve short recordings (see
//! `assets/sounds/CREDITS.md`), one picked at random for every landing so a
//! running clock does not sound mechanical.

use std::time::{Duration, Instant};

use rodio::buffer::SamplesBuffer;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Source};

const RECORDINGS: [&[u8]; 12] = [
    include_bytes!("../assets/sounds/flap-01.wav"),
    include_bytes!("../assets/sounds/flap-02.wav"),
    include_bytes!("../assets/sounds/flap-03.wav"),
    include_bytes!("../assets/sounds/flap-04.wav"),
    include_bytes!("../assets/sounds/flap-05.wav"),
    include_bytes!("../assets/sounds/flap-06.wav"),
    include_bytes!("../assets/sounds/flap-07.wav"),
    include_bytes!("../assets/sounds/flap-08.wav"),
    include_bytes!("../assets/sounds/flap-09.wav"),
    include_bytes!("../assets/sounds/flap-10.wav"),
    include_bytes!("../assets/sounds/flap-11.wav"),
    include_bytes!("../assets/sounds/flap-12.wav"),
];

/// Loudness of a flap; the recordings are normalised to a 0.9 peak.
const VOLUME: f32 = 0.4;
/// Flaps landing in the same frame are spread out a little, as on a real
/// clock, instead of adding up to one loud click.
const STAGGER: Duration = Duration::from_millis(12);
/// The audio device is released after this long without a flap, so an idle
/// clock does not keep the system's audio running.
const IDLE_CLOSE: Duration = Duration::from_secs(10);

/// One recording, ready to play.
struct Clip {
    sample_rate: u32,
    samples: Vec<f32>,
}

enum Output {
    Closed,
    Open {
        sink: MixerDeviceSink,
        last_used: Instant,
    },
    /// Opening the device failed; stay silent until sound is switched off
    /// and on again.
    Failed,
}

/// Plays flap sounds on the default audio device, which is opened only
/// while sound is on and flaps land.
pub struct FlapSound {
    clips: Vec<Clip>,
    output: Output,
    random: u64,
}

impl Default for FlapSound {
    fn default() -> Self {
        Self::new()
    }
}

impl FlapSound {
    pub fn new() -> Self {
        let clips = RECORDINGS
            .iter()
            .map(|wav| parse_wav(wav).expect("bundled flap sounds are 16-bit PCM WAV"))
            .collect();
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64);
        Self {
            clips,
            output: Output::Closed,
            random: seed | 1,
        }
    }

    /// Call after every frame with whether sound is on and the frame's
    /// `flap_sounds`. Never fails: without a working audio device the clock
    /// simply stays silent.
    pub fn update(&mut self, enabled: bool, flaps: usize) {
        let now = Instant::now();
        if !enabled {
            self.output = Output::Closed;
            return;
        }
        if flaps == 0 {
            if let Output::Open { last_used, .. } = self.output {
                if now.duration_since(last_used) >= IDLE_CLOSE {
                    self.output = Output::Closed;
                }
            }
            return;
        }
        if let Output::Closed = self.output {
            self.output = match DeviceSinkBuilder::open_default_sink() {
                Ok(mut sink) => {
                    sink.log_on_drop(false);
                    Output::Open {
                        sink,
                        last_used: now,
                    }
                }
                Err(e) => {
                    log::warn!("no sound: cannot open the audio device: {e}");
                    Output::Failed
                }
            };
        }
        let Output::Open { sink, last_used } = &mut self.output else {
            return;
        };
        *last_used = now;
        for i in 0..flaps {
            let clip = &self.clips[next_random(&mut self.random) as usize % self.clips.len()];
            let source = SamplesBuffer::new(
                std::num::NonZero::new(1).unwrap(),
                std::num::NonZero::new(clip.sample_rate).unwrap(),
                clip.samples.clone(),
            )
            .amplify(VOLUME)
            .delay(STAGGER * i as u32);
            sink.mixer().add(source);
        }
    }
}

fn next_random(state: &mut u64) -> u64 {
    // xorshift64: plenty for picking one of twelve clips.
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// Reads a mono 16-bit PCM WAV file.
fn parse_wav(data: &[u8]) -> Option<Clip> {
    if data.get(0..4)? != b"RIFF" || data.get(8..12)? != b"WAVE" {
        return None;
    }
    let u16_at = |i: usize| Some(u16::from_le_bytes(data.get(i..i + 2)?.try_into().ok()?));
    let u32_at = |i: usize| Some(u32::from_le_bytes(data.get(i..i + 4)?.try_into().ok()?));
    let mut pos = 12;
    let mut sample_rate = None;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let len = u32_at(pos + 4)? as usize;
        let body = pos + 8;
        match id {
            b"fmt " => {
                let (format, channels, bits) =
                    (u16_at(body)?, u16_at(body + 2)?, u16_at(body + 14)?);
                if (format, channels, bits) != (1, 1, 16) {
                    return None;
                }
                sample_rate = Some(u32_at(body + 4)?);
            }
            b"data" => {
                let samples = data
                    .get(body..body + len)?
                    .chunks_exact(2)
                    .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0)
                    .collect();
                return Some(Clip {
                    sample_rate: sample_rate?,
                    samples,
                });
            }
            _ => {}
        }
        // Chunks are padded to an even length.
        pos = body + len + (len & 1);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_flaps_are_short_mono_clips() {
        for wav in RECORDINGS {
            let clip = parse_wav(wav).expect("a 16-bit mono WAV");
            assert_eq!(clip.sample_rate, 44_100);
            let ms = clip.samples.len() as f32 / 44.1;
            assert!((40.0..100.0).contains(&ms), "{ms} ms");
            let peak = clip.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!((0.5..=1.0).contains(&peak), "peak {peak}");
        }
    }

    #[test]
    fn rejects_other_formats() {
        assert!(parse_wav(b"not a wav file").is_none());
        let mut stereo = RECORDINGS[0].to_vec();
        stereo[22] = 2; // channel count in the fmt chunk
        assert!(parse_wav(&stereo).is_none());
    }

    #[test]
    fn picks_every_clip_sometimes() {
        let mut state = 0x1234_5678;
        let mut seen = [false; 12];
        for _ in 0..500 {
            seen[next_random(&mut state) as usize % 12] = true;
        }
        assert!(seen.iter().all(|s| *s));
    }

    #[test]
    fn switched_off_it_never_touches_the_audio_device() {
        let mut sound = FlapSound::new();
        sound.update(false, 3);
        assert!(matches!(sound.output, Output::Closed));
    }
}
