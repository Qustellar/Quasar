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
    fn play(
        &mut self,
        clip: Handle<AudioClip>,
        data: &AudioClip,
        volume: f32,
        looping: bool,
    ) -> EngineResult<()>;
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
pub struct RodioAudioBackend {
    stream: Option<rodio::OutputStream>,
    sinks: HashMap<Handle<AudioClip>, rodio::Sink>,
    fallback: NullAudioBackend,
}

#[cfg(feature = "rodio-backend")]
impl Default for RodioAudioBackend {
    fn default() -> Self {
        let stream = rodio::OutputStreamBuilder::open_default_stream().ok();
        Self {
            stream,
            sinks: HashMap::new(),
            fallback: NullAudioBackend::default(),
        }
    }
}

#[cfg(feature = "rodio-backend")]
impl AudioBackend for RodioAudioBackend {
    fn play(
        &mut self,
        clip: Handle<AudioClip>,
        data: &AudioClip,
        volume: f32,
        looping: bool,
    ) -> EngineResult<()> {
        let Some(stream) = &self.stream else {
            return self.fallback.play(clip, data, volume, looping);
        };
        let source = rodio::Decoder::try_from(std::io::Cursor::new(data.bytes.clone()))
            .map_err(|error| EngineError::Runtime(error.to_string()))?;
        let sink = rodio::Sink::connect_new(stream.mixer());
        sink.set_volume(volume.max(0.0));
        if looping {
            use rodio::Source;
            sink.append(source.repeat_infinite());
        } else {
            sink.append(source);
        }
        sink.play();
        self.sinks.insert(clip, sink);
        Ok(())
    }
    fn pause(&mut self, clip: Handle<AudioClip>) -> EngineResult<()> {
        if let Some(sink) = self.sinks.get(&clip) {
            sink.pause();
        }
        Ok(())
    }
    fn stop(&mut self, clip: Handle<AudioClip>) -> EngineResult<()> {
        if let Some(sink) = self.sinks.remove(&clip) {
            sink.stop();
        }
        Ok(())
    }
}

impl NullAudioBackend {
    pub fn state(&self, clip: Handle<AudioClip>) -> AudioPlaybackState {
        self.states.get(&clip).copied().unwrap_or_default()
    }
}

impl AudioBackend for NullAudioBackend {
    fn play(
        &mut self,
        clip: Handle<AudioClip>,
        _data: &AudioClip,
        _volume: f32,
        _looping: bool,
    ) -> EngineResult<()> {
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
    clips: HashMap<Handle<AudioClip>, AudioClip>,
}

impl<B: AudioBackend + Default> Default for AudioPlayer<B> {
    fn default() -> Self {
        Self {
            backend: B::default(),
            states: HashMap::new(),
            clips: HashMap::new(),
        }
    }
}

impl<B: AudioBackend> AudioPlayer<B> {
    pub fn play(&mut self, source: AudioSource) -> EngineResult<()> {
        let clip = source
            .clip
            .ok_or_else(|| EngineError::AssetNotFound("audio clip".into()))?;
        let data = self
            .clips
            .get(&clip)
            .ok_or_else(|| EngineError::AssetNotFound("audio clip data".into()))?;
        self.backend
            .play(clip, data, source.volume, source.looping)?;
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

    pub fn register_clip(&mut self, clip: Handle<AudioClip>, data: AudioClip) {
        self.clips.insert(clip, data);
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
