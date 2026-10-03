//! AEC runs after the external processor, using selected apps as its reference.
use crate::error::{AudioError, Result};
use aec3::{nodes::audio::AudioFormat, pipelines::linear::{self, LinearPipeline}};
pub const FRAME_SAMPLES: usize = 960; // Stereo, 48 kHz, 10 ms.
pub struct EchoCancellation { pipeline: LinearPipeline }
impl EchoCancellation {
    pub fn new() -> Result<Self> {
        let format = AudioFormat::ten_ms(48_000, 2);
        let pipeline = linear::builder(format, format)
            .enable_high_pass_filter(false)
            .enable_noise_suppression(false)
            .enable_gain_controller2(false)
            .enable_post_filter(false)
            .build().map_err(|e| AudioError::message(e.to_string()))?;
        Ok(Self { pipeline })
    }
    pub fn process(&mut self, reference: &[f32], processed: &[f32], out: &mut [f32]) -> Result<()> {
        if reference.len() != FRAME_SAMPLES || processed.len() != FRAME_SAMPLES || out.len() != FRAME_SAMPLES {
            return Err(AudioError::message("El bloque de cancelación debe tener 10 ms de audio estéreo."));
        }
        self.pipeline.handle_render_frame(reference).map_err(|e| AudioError::message(e.to_string()))?;
        let produced = self.pipeline.process_capture_frame(processed, out).map_err(|e| AudioError::message(e.to_string()))?;
        if !produced { out.fill(0.0); }
        Ok(())
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn removes_delayed_reference_after_processor_and_preserves_independent_audio() {
        let mut aec = EchoCancellation::new().unwrap();
        let mut seed = 123u32;
        let mut history = std::collections::VecDeque::from(vec![0.0; 48_000 / 20 * 2]);
        let mut out = vec![0.0; FRAME_SAMPLES];
        let mut input_energy = 0.0f64;
        let mut output_energy = 0.0f64;
        for block in 0..1200 {
            let mut reference = vec![0.0; FRAME_SAMPLES];
            for frame in reference.chunks_mut(2) {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let x = (seed as i32 as f32 / i32::MAX as f32) * 0.15;
                frame.fill(x);
            }
            let mut mixed = vec![0.0; FRAME_SAMPLES];
            for i in 0..FRAME_SAMPLES {
                history.push_back(reference[i]);
                mixed[i] = history.pop_front().unwrap() * if block < 600 {0.75} else {0.4};
            }
            aec.process(&reference, &mixed, &mut out).unwrap();
            if block > 1000 {
                input_energy += mixed.iter().map(|x| (*x as f64).powi(2)).sum::<f64>();
                output_energy += out.iter().map(|x| (*x as f64).powi(2)).sum::<f64>();
            }
        }
        let reduction = 10.0 * (input_energy / output_energy.max(1e-20)).log10();
        eprintln!("Delayed private source reduction after gain change: {reduction:.1} dB");
        assert!(reduction > 20.0, "Cancellation: {reduction:.1} dB");
        // Independent voice/music must remain audible once the private source stops.
        let reference = vec![0.0; FRAME_SAMPLES];
        let mut preserved_energy = 0.0;
        for block in 0..400 {
            let mixed: Vec<_> = (0..FRAME_SAMPLES).map(|i| ((block*480+i/2) as f32 * 0.031).sin()*0.1).collect();
            aec.process(&reference, &mixed, &mut out).unwrap();
            if block > 300 { preserved_energy += out.iter().map(|x| (*x as f64).powi(2)).sum::<f64>(); }
        }
        assert!(preserved_energy > 50.0, "Independent audio lost: {preserved_energy}");
    }
    #[test] fn simultaneous_voice_remains_audible_with_private_source() {
        let mut aec = EchoCancellation::new().unwrap();
        let mut seed = 345u32;
        let mut history = std::collections::VecDeque::from(vec![0.0; 960*4]);
        let mut out = vec![0.0; FRAME_SAMPLES];
        let mut voice_projection = 0.0f64;
        let mut voice_energy = 0.0f64;
        for block in 0..1200 {
            let mut reference = vec![0.0; FRAME_SAMPLES];
            let mut voice = vec![0.0; FRAME_SAMPLES];
            let mut mixed = vec![0.0; FRAME_SAMPLES];
            for i in 0..480 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let x = (seed as i32 as f32 / i32::MAX as f32)*0.12;
                let near = if block > 600 { ((block*480+i) as f32*0.043).sin()*0.1 } else {0.0};
                for c in 0..2 {
                    let j = i*2+c;
                    reference[j]=x;
                    voice[j]=near;
                    history.push_back(x);
                    mixed[j]=history.pop_front().unwrap()*0.7+near;
                }
            }
            aec.process(&reference,&mixed,&mut out).unwrap();
            if block > 1000 {
                // AEC's filterbank adds a small algorithmic delay. Compare energy
                // rather than phase against the simultaneous independent source.
                voice_projection += out.iter().map(|x| (*x as f64).powi(2)).sum::<f64>();
                voice_energy += voice.iter().map(|x| (*x as f64).powi(2)).sum::<f64>();
            }
        }
        let ratio = voice_projection/voice_energy;
        eprintln!("Simultaneous independent audio energy ratio: {ratio:.3}");
        assert!(ratio>0.4 && ratio<1.6, "Independent source energy ratio: {ratio}");
    }
}
