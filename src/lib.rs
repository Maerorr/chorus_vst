use chorus::Chorus;
use nih_plug::prelude::*;
use std::{sync::{Arc, mpsc::channel}, collections::VecDeque, env};

use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

mod delay;
mod lfo;
mod editor;
mod chorus;

struct MaerorChorus {
    params: Arc<MaerorChorusParams>,
    l_delay_line1: delay::Delay,
    l_delay_line2: delay::Delay,
    l_delay_line3: delay::Delay,
    r_delay_line1: delay::Delay,
    r_delay_line2: delay::Delay,
    r_delay_line3: delay::Delay,
    l_lfo1: lfo::LFO,
    l_lfo2: lfo::LFO,
    l_lfo3: lfo::LFO,
    r_lfo1: lfo::LFO,
    r_lfo2: lfo::LFO,
    r_lfo3: lfo::LFO,
    sample_rate: f32,
    l_feedback_buffer: Box<VecDeque<f32>>,
    r_feedback_buffer: Box<VecDeque<f32>>,
    chorus: chorus::Chorus,
}

#[derive(Params)]
struct MaerorChorusParams {
    #[persist = "editor-state"]
    editor_state: Arc<ViziaState>,

    // parameters for chorus
    #[id = "depth"]
    pub depth: FloatParam,
    #[id = "rate"]
    pub rate: FloatParam,
    #[id = "delay_ms"]
    pub delay_ms: FloatParam,
    #[id = "feedback"]
    pub feedback: FloatParam,
    #[id = "wet"]
    pub wet: FloatParam,
    #[id = "dry"]
    pub dry: FloatParam,
}

impl Default for MaerorChorus {
    fn default() -> Self {
        Self {
            params: Arc::new(MaerorChorusParams::default()),
            l_delay_line1: delay::Delay::new(44100, 0, 0.0),
            l_delay_line2: delay::Delay::new(44100, 0, 0.0),
            l_delay_line3: delay::Delay::new(44100, 0, 0.0),
            r_delay_line1: delay::Delay::new(44100, 0, 0.0),
            r_delay_line2: delay::Delay::new(44100, 0, 0.0),
            r_delay_line3: delay::Delay::new(44100, 0, 0.0),
            l_lfo1: lfo::LFO::new(44100.0, 0.25),
            l_lfo2: lfo::LFO::new(44100.0, 0.25),
            l_lfo3: lfo::LFO::new(44100.0, 0.25),
            r_lfo1: lfo::LFO::new(44100.0, 0.25),
            r_lfo2: lfo::LFO::new(44100.0, 0.25),
            r_lfo3: lfo::LFO::new(44100.0, 0.25),
            sample_rate: 44100.0,
            l_feedback_buffer: Box::new(VecDeque::with_capacity(44100)),
            r_feedback_buffer: Box::new(VecDeque::with_capacity(44100)),
            chorus: Chorus::new(44100.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        }
    }
}

impl Default for MaerorChorusParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            // implement depth, rate, delay_ms, feedback, wet parameters
            // DEPTH
            depth: FloatParam::new("Depth", 5.0, FloatRange::Linear { min: 0.0, max: 25.0 })
            .with_unit("ms")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            
            // RATE
            rate: FloatParam::new("Rate", 0.5, FloatRange::Skewed { min: 0.02, max: 10.0, factor: 0.3 })
            .with_unit("Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            // DELAY
            delay_ms: FloatParam::new("Delay", 15.0, FloatRange::Linear { min: 0.1, max: 50.0 })
            .with_unit("ms")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            // FEEDBACK
            feedback: FloatParam::new("Feedback", 0.0, FloatRange::Linear { min: 0.0, max: 0.999 })
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            // WET
            wet: FloatParam::new("Wet", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            // DRY
            dry: FloatParam::new("Dry", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 })
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),
        }
    }
}

impl Plugin for MaerorChorus {
    const NAME: &'static str = "maeror_chorus";
    const VENDOR: &'static str = "maeror";
    const URL: &'static str = "none";
    const EMAIL: &'static str = "none";
    const VERSION: &'static str = "test";

    // The first audio IO layout is used as the default. The other layouts may be selected either
    // explicitly or automatically by the host or the user depending on the plugin API/backend.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    // If the plugin can send or receive SysEx messages, it can define a type to wrap around those
    // messages here. The type implements the `SysExMessage` trait, which allows conversion to and
    // from plain byte buffers.
    type SysExMessage = ();
    // More advanced plugins can use this to run expensive background tasks. See the field's
    // documentation for more information. `()` means that the plugin does not have any background
    // tasks.
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        _buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.l_delay_line1.resize_buffers(_buffer_config.sample_rate as usize);
        self.l_delay_line2.resize_buffers(_buffer_config.sample_rate as usize);
        self.l_delay_line3.resize_buffers(_buffer_config.sample_rate as usize);
        self.r_delay_line1.resize_buffers(_buffer_config.sample_rate as usize);
        self.r_delay_line2.resize_buffers(_buffer_config.sample_rate as usize);
        self.r_delay_line3.resize_buffers(_buffer_config.sample_rate as usize);

        self.l_lfo1 = lfo::LFO::new_random_phase(_buffer_config.sample_rate as f32, 0.25);
        self.l_lfo2 = lfo::LFO::new_random_phase(_buffer_config.sample_rate as f32, 0.25);
        self.l_lfo3 = lfo::LFO::new_random_phase(_buffer_config.sample_rate as f32, 0.25);
        self.r_lfo1 = lfo::LFO::new_random_phase(_buffer_config.sample_rate as f32, 0.25);
        self.r_lfo2 = lfo::LFO::new_random_phase(_buffer_config.sample_rate as f32, 0.25);
        self.r_lfo3 = lfo::LFO::new_random_phase(_buffer_config.sample_rate as f32, 0.25);

        self.sample_rate = 2.0 * _buffer_config.sample_rate as f32;

        self.l_feedback_buffer = Box::new(VecDeque::with_capacity(_buffer_config.sample_rate as usize));
        self.l_feedback_buffer.make_contiguous();
        self.r_feedback_buffer = Box::new(VecDeque::with_capacity(_buffer_config.sample_rate as usize));
        self.r_feedback_buffer.make_contiguous();

        self.chorus.resize_buffers(self.sample_rate);
        
        for _ in 0.._buffer_config.sample_rate as usize {
            self.l_feedback_buffer.push_back(0.0);
            self.r_feedback_buffer.push_back(0.0);
        }

        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.
        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        for (i, channel_samples) in buffer.iter_samples().enumerate() {
            // Smoothing is optionally built into the parameters themselves
            // let gain = self.params.gain.smoothed.next();
            let depth = self.params.depth.smoothed.next();
            let rate = self.params.rate.smoothed.next();
            let delay_ms = self.params.delay_ms.smoothed.next();
            let feedback = self.params.feedback.smoothed.next();
            let wet = self.params.wet.smoothed.next();
            let dry = self.params.dry.smoothed.next();

            let delay_samples: usize = ((delay_ms / 1000.0) * self.sample_rate).round() as usize;

            self.chorus.set_params(self.sample_rate, delay_ms, feedback, depth, rate, wet, dry);

            for (num, sample) in channel_samples.into_iter().enumerate() {
                if num == 0 {
                    *sample = self.chorus.process_left(*sample);
                } else {
                    *sample = self.chorus.process_right(*sample);
                }
            }

            // for (j, sample) in channel_samples.into_iter().enumerate() {
            //     if j == 0 {
            //         self.l_lfo1.rate = rate;
            //         self.l_lfo2.rate = rate;
            //         self.l_lfo3.rate = rate;

            //         self.l_delay_line1.delay = delay_samples;
            //         self.l_delay_line2.delay = delay_samples;
            //         self.l_delay_line3.delay = delay_samples;

            //         let mut calculated_depth = (depth / 1000.0) * self.sample_rate;
            //         if calculated_depth > delay_samples as f32 / 2.0 {
            //             calculated_depth = delay_samples as f32 / 2.0;
            //         }
            //         let offset1 = (self.l_lfo1.next_value() * calculated_depth / 2.0).round() as i32;
            //         let offset2 = (self.l_lfo2.next_value() * calculated_depth / 2.0).round() as i32;
            //         let offset3 = (self.l_lfo3.next_value() * calculated_depth / 2.0).round() as i32;
                    
            //         let x = *sample as f32 + wet * feedback * self.l_feedback_buffer.get(delay_samples).unwrap();
            //         //nih_log!("{}", (delay_samples as i32 + offset1) as usize);
            //         let mut y = wet * 1.0/3.0 * (
            //             self.l_delay_line1.process_sample(x, (delay_samples as i32 + offset1) as usize) 
            //             + self.l_delay_line2.process_sample(x, (delay_samples as i32 + offset2) as usize) 
            //             + self.l_delay_line3.process_sample(x, (delay_samples as i32 + offset3) as usize)
            //         ) + x * dry;
                    

            //         if wet + dry > 1.0 {
            //             y = y / (wet + dry);
            //         }

            //         *sample = y;
                    
            //         self.l_lfo1.update_lfo();
            //         self.l_lfo2.update_lfo();
            //         self.l_lfo3.update_lfo();
    
            //         self.l_feedback_buffer.rotate_right(1);
            //         self.l_feedback_buffer[0] = *sample;
            //     } else {
            //         self.r_lfo1.rate = rate;
            //         self.r_lfo2.rate = rate;
            //         self.r_lfo3.rate = rate;

            //         self.r_delay_line1.delay = delay_samples;
            //         self.r_delay_line2.delay = delay_samples;
            //         self.r_delay_line3.delay = delay_samples;

            //         let mut calculated_depth = (depth / 1000.0) * self.sample_rate;
            //         if calculated_depth > delay_samples as f32 / 2.0 {
            //             calculated_depth = delay_samples as f32 / 2.0;
            //         }

            //         let offset1 = (self.r_lfo1.next_value() * calculated_depth / 2.0).round() as i32;
            //         let offset2 = (self.r_lfo2.next_value() * calculated_depth / 2.0).round() as i32;
            //         let offset3 = (self.r_lfo3.next_value() * calculated_depth / 2.0).round() as i32;
                    
            //         let x = *sample as f32 + wet * feedback * self.r_feedback_buffer.get(delay_samples).unwrap();
            //         let mut y = wet * 1.0/3.0 * (
            //             self.r_delay_line1.process_sample(x, (delay_samples as i32 + offset1) as usize) 
            //             + self.r_delay_line2.process_sample(x, (delay_samples as i32 + offset2) as usize) 
            //             + self.r_delay_line3.process_sample(x, (delay_samples as i32 + offset3) as usize)
            //         ) + x * dry;
                    
            //         if wet + dry > 1.0 {
            //             y = y / (wet + dry);
            //         }

            //         *sample = y;
                    
            //         self.r_lfo1.update_lfo();
            //         self.r_lfo2.update_lfo();
            //         self.r_lfo3.update_lfo();
    
            //         self.r_feedback_buffer.rotate_right(1);
            //         self.r_feedback_buffer[0] = *sample;
            //     }
            // }
        }

        ProcessStatus::Normal
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.params.editor_state.clone(),
        )
    }
}

impl ClapPlugin for MaerorChorus {
    const CLAP_ID: &'static str = "{{ cookiecutter.clap_id }}";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("{{ cookiecutter.description }}");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;

    // Don't forget to change these features
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Stereo];
}

impl Vst3Plugin for MaerorChorus {
    const VST3_CLASS_ID: [u8; 16] = *b"MaerorChorsRvdH.";

    // And also don't forget to change these categories
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Delay, Vst3SubCategory::Modulation, Vst3SubCategory::Fx];
}

//nih_export_clap!(MaerorChorus);
nih_export_vst3!(MaerorChorus);
