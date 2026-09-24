use qst_core::{EngineError, EngineResult, Handle};

#[derive(Clone, Debug)]
pub struct AudioClip {
    pub bytes: Vec<u8>,
}

pub trait AudioOutput {
    fn play(&mut self, clip: Handle<AudioClip>) -> EngineResult<()>;
    fn stop_all(&mut self) -> EngineResult<()>;
}

pub struct UnavailableAudioOutput;

impl AudioOutput for UnavailableAudioOutput {
    fn play(&mut self, _clip: Handle<AudioClip>) -> EngineResult<()> {
        Err(EngineError::Unsupported(
            "audio playback is outside Phase 1".into(),
        ))
    }
    fn stop_all(&mut self) -> EngineResult<()> {
        Ok(())
    }
}
