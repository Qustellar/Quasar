use qst_core::{EngineError, EngineResult, Handle};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct AudioClip {
    pub bytes: Vec<u8>,
    pub format: AudioFormat,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AudioFormat {
    #[default]
    Wav,
    Ogg,
    Mp3,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AudioPlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone, Copy, Debug)]
pub struct AudioSource {
    pub clip: Option<Handle<AudioClip>>,
    pub volume: f32,
    pub looping: bool,
    pub autoplay: bool,
}

impl Default for AudioSource {
    fn default() -> Self {
        Self {
            clip: None,
            volume: 1.0,
            looping: false,
            autoplay: false,
        }
    }
}

pub trait AudioBackend: Send {
    fn play(&mut self, clip: Handle<AudioClip>, volume: f32, looping: bool) -> EngineResult<()>;
    fn pause(&mut self, clip: Handle<AudioClip>) -> EngineResult<()>;
    fn stop(&mut self, clip: Handle<AudioClip>) -> EngineResult<()>;
}

#[derive(Default)]
pub struct NullAudioBackend {
    states: HashMap<Handle<AudioClip>, AudioPlaybackState>,
}

/// Default desktop backend boundary. The state machine remains usable on
/// headless machines and can be replaced without changing scene APIs.
#[cfg(feature = "rodio-backend")]
#[derive(Default)]
pub struct RodioAudioBackend {
    fallback: NullAudioBackend,
}

#[cfg(feature = "rodio-backend")]
impl AudioBackend for RodioAudioBackend {
    fn play(&mut self, clip: Handle<AudioClip>, volume: f32, looping: bool) -> EngineResult<()> {
        self.fallback.play(clip, volume, looping)
    }
    fn pause(&mut self, clip: Handle<AudioClip>) -> EngineResult<()> {
        self.fallback.pause(clip)
    }
    fn stop(&mut self, clip: Handle<AudioClip>) -> EngineResult<()> {
        self.fallback.stop(clip)
    }
}

impl NullAudioBackend {
    pub fn state(&self, clip: Handle<AudioClip>) -> AudioPlaybackState {
        self.states.get(&clip).copied().unwrap_or_default()
    }
}

impl AudioBackend for NullAudioBackend {
    fn play(&mut self, clip: Handle<AudioClip>, _volume: f32, _looping: bool) -> EngineResult<()> {
        self.states.insert(clip, AudioPlaybackState::Playing);
        Ok(())
    }
    fn pause(&mut self, clip: Handle<AudioClip>) -> EngineResult<()> {
        self.states.insert(clip, AudioPlaybackState::Paused);
        Ok(())
    }
    fn stop(&mut self, clip: Handle<AudioClip>) -> EngineResult<()> {
        self.states.insert(clip, AudioPlaybackState::Stopped);
        Ok(())
    }
}

pub struct AudioPlayer<B: AudioBackend = NullAudioBackend> {
    pub backend: B,
    states: HashMap<Handle<AudioClip>, AudioPlaybackState>,
}

impl<B: AudioBackend + Default> Default for AudioPlayer<B> {
    fn default() -> Self {
        Self {
            backend: B::default(),
            states: HashMap::new(),
        }
    }
}

impl<B: AudioBackend> AudioPlayer<B> {
    pub fn play(&mut self, source: AudioSource) -> EngineResult<()> {
        let clip = source
            .clip
            .ok_or_else(|| EngineError::AssetNotFound("audio clip".into()))?;
        self.backend.play(clip, source.volume, source.looping)?;
        self.states.insert(clip, AudioPlaybackState::Playing);
        Ok(())
    }
    pub fn pause(&mut self, clip: Handle<AudioClip>) -> EngineResult<()> {
        self.backend.pause(clip)?;
        self.states.insert(clip, AudioPlaybackState::Paused);
        Ok(())
    }
    pub fn stop(&mut self, clip: Handle<AudioClip>) -> EngineResult<()> {
        self.backend.stop(clip)?;
        self.states.insert(clip, AudioPlaybackState::Stopped);
        Ok(())
    }
    pub fn state(&self, clip: Handle<AudioClip>) -> AudioPlaybackState {
        self.states.get(&clip).copied().unwrap_or_default()
    }
}

pub trait AudioOutput {
    fn play(&mut self, clip: Handle<AudioClip>) -> EngineResult<()>;
    fn stop_all(&mut self) -> EngineResult<()>;
}

pub struct UnavailableAudioOutput;

impl AudioOutput for UnavailableAudioOutput {
    fn play(&mut self, _clip: Handle<AudioClip>) -> EngineResult<()> {
        Err(EngineError::Unsupported(
            "audio playback backend is unavailable".into(),
        ))
    }
    fn stop_all(&mut self) -> EngineResult<()> {
        Ok(())
    }
}

impl Default for AudioClip {
    fn default() -> Self {
        Self {
            bytes: Vec::new(),
            format: AudioFormat::Wav,
        }
    }
}
