use std::env;
use std::error::Error;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use dotenv::dotenv;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, SampleRate, StreamConfig};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::time;
use google_generativeai::{Client, ClientOptions, GenerativeModel, LiveConnectConfig, Modality, SpeechConfig, VoiceConfig, PrebuiltVoiceConfig, Content, Part};
use tokio::spawn;
use std::io::{self, Write};

// Constants
const FORMAT: SampleFormat = SampleFormat::I16;
const CHANNELS: u16 = 1; // Mono
const SEND_SAMPLE_RATE: u32 = 16000;
const RECEIVE_SAMPLE_RATE: u32 = 24000;
const FRAME_DURATION_MS: u64 = 20;
const FRAME_SAMPLES_16K: usize = (SEND_SAMPLE_RATE as usize * FRAME_DURATION_MS as usize) / 1000;
const FRAME_SIZE_BYTES_16K: usize = FRAME_SAMPLES_16K * 2; // 16-bit = 2 bytes per sample
const FRAME_SAMPLES_OUTPUT: usize = (RECEIVE_SAMPLE_RATE as usize * FRAME_DURATION_MS as usize) / 1000;
const FRAME_SIZE_BYTES_OUTPUT: usize = FRAME_SAMPLES_OUTPUT * 2;

// Default system prompt
const SYS_PROMPT: &str = "
You are Immy, a magical, AI-powered teddy bear who loves chatting with children. You're warm, funny, and full of wonder, always ready to share a story, answer curious questions, or offer gentle advice.
";

// Audio Processing struct (placeholder for WebRTC audio processing)
struct AudioProcessor {
    enable_ns: bool,
    enable_vad: bool,
    ns_level: u8,
    vad_level: u8,
}

impl AudioProcessor {
    fn new(enable_ns: bool, enable_vad: bool) -> Self {
        Self {
            enable_ns,
            enable_vad,
            ns_level: 3,
            vad_level: 3,
        }
    }

    fn set_ns_level(&mut self, level: u8) {
        self.ns_level = level;
    }

    fn set_vad_level(&mut self, level: u8) {
        self.vad_level = level;
    }

    fn process_reverse_stream(&mut self, _data: &[u8]) {
        // Placeholder for WebRTC audio processing
        // In a real implementation, this would process the reference audio
    }

    fn process_stream(&mut self, data: &[u8]) -> Vec<u8> {
        // Placeholder for WebRTC audio processing
        // In a real implementation, this would apply noise suppression and VAD
        data.to_vec()
    }
}

// Helper function to convert stereo to mono
fn stereo_to_mono(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len() / 2);
    for i in (0..data.len()).step_by(4) {
        if i + 1 < data.len() {
            result.push(data[i]);
            result.push(data[i + 1]);
        }
    }
    result
}

// Text-to-speech announcement function
fn speak_announcement(text: &str) {
    println!("Announcement: {}", text);
    // This would use a TTS library in Rust
    // For now, just print the message
    println!("(TTS would say: {})", text);
}

struct GeminiVoiceChat {
    voice_name: String,
    system_prompt: String,
    audio_in_tx: Option<Sender<Vec<u8>>>,
    audio_out_tx: Option<Sender<Vec<u8>>>,
    last_playback_end: Arc<Mutex<Instant>>,
    playback_cooldown: Duration,
    audio_processor: Option<AudioProcessor>,
}

impl GeminiVoiceChat {
    fn new(voice_name: &str, system_prompt: &str) -> Self {
        Self {
            voice_name: voice_name.to_string(),
            system_prompt: system_prompt.to_string(),
            audio_in_tx: None,
            audio_out_tx: None,
            last_playback_end: Arc::new(Mutex::new(Instant::now() - Duration::from_secs(10))),
            playback_cooldown: Duration::from_millis(300),
            audio_processor: Some(AudioProcessor::new(true, true)),
        }
    }

    async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // Set up audio channels
        let (audio_in_tx, audio_in_rx) = mpsc::channel::<Vec<u8>>(5);
        let (audio_out_tx, audio_out_rx) = mpsc::channel::<Vec<u8>>(5);
        self.audio_in_tx = Some(audio_in_tx);
        self.audio_out_tx = Some(audio_out_tx);

        // Set up and run audio handling tasks
        let mic_task = self.listen_mic_audio();
        let reverse_task = self.listen_reverse_audio();
        let playback_task = self.play_audio(audio_in_rx);

        // Set up Gemini client
        let use_vertexai = false;
        let api_key = if use_vertexai {
            "".to_string() // Not used in this mode, would use Google Cloud credentials
        } else {
            env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY must be set")
        };

        let client_options = ClientOptions::default()
            .with_api_key(api_key);
        let client = Client::new(client_options)?;

        // Configure the model
        let model_name = "models/gemini-2.0-flash-exp";
        let config = LiveConnectConfig::default()
            .with_response_modalities(vec![Modality::Audio])
            .with_speech_config(SpeechConfig::default()
                .with_voice_config(VoiceConfig::default()
                    .with_prebuilt_voice_config(PrebuiltVoiceConfig::default()
                        .with_voice_name(&self.voice_name)
                    )
                )
            )
            .with_system_instruction(Content::new(vec![Part::text(&self.system_prompt)]));

        let model = GenerativeModel::new(model_name.to_string())
            .with_live_connect_config(config);

        println!("Voice chat started. Speak into your microphone. Press Ctrl+C to quit.");
        println!("Note: For reliable echo removal, use headphones or a proper loopback device.");

        // Start the audio tasks
        let mic_handle = spawn(mic_task);
        let reverse_handle = spawn(reverse_task);
        let playback_handle = spawn(playback_task);

        // Set up stream handler for Gemini API
        let mut session = model.start_live_session(&client).await?;
        
        // Process audio from mic_task and send to Gemini
        let audio_out_tx_clone = self.audio_out_tx.clone().unwrap();
        let audio_handler = spawn(async move {
            let mut audio_out_rx = audio_out_rx;
            while let Some(audio_data) = audio_out_rx.recv().await {
                if let Err(e) = session.send_audio(&audio_data).await {
                    eprintln!("Error sending audio to Gemini: {}", e);
                }
            }
        });

        // Process responses from Gemini
        while let Some(response) = session.next_response().await {
            if let Some(audio_data) = response.audio {
                println!("Received {} bytes from Gemini", audio_data.len());
                if let Some(tx) = &self.audio_in_tx {
                    let _ = tx.send(audio_data).await;
                }
            }
            if let Some(text) = response.text {
                print!("Gemini: {}", text);
                io::stdout().flush().unwrap();
            }
        }

        // Clean up
        mic_handle.abort();
        reverse_handle.abort();
        playback_handle.abort();
        audio_handler.abort();

        println!("Voice chat session ended.");
        Ok(())
    }

    async fn listen_mic_audio(&self) -> Result<(), Box<dyn Error>> {
        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or("No input device available")?;
        
        println!("Using input device: {}", device.name()?);
        
        let config = StreamConfig {
            channels: CHANNELS,
            sample_rate: SampleRate(SEND_SAMPLE_RATE),
            buffer_size: BufferSize::Fixed(FRAME_SAMPLES_16K as u32),
        };

        let last_playback_end = self.last_playback_end.clone();
        let playback_cooldown = self.playback_cooldown;
        let audio_out_tx = self.audio_out_tx.clone().unwrap();
        let mut processor = self.audio_processor.clone();

        let stream = device.build_input_stream(
            &config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                let now = Instant::now();
                let last_end = {
                    let guard = last_playback_end.lock().unwrap();
                    *guard
                };
                
                if now.duration_since(last_end) < playback_cooldown {
                    return;
                }
                
                // Convert i16 samples to bytes
                let bytes: Vec<u8> = data.iter()
                    .flat_map(|&sample| sample.to_le_bytes().to_vec())
                    .collect();
                
                // Apply audio processing if available
                let processed_bytes = if let Some(proc) = &mut processor {
                    proc.process_stream(&bytes)
                } else {
                    bytes
                };
                
                // Send to Gemini
                let audio_out_tx_clone = audio_out_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = audio_out_tx_clone.send(processed_bytes).await {
                        eprintln!("Error sending mic data: {}", e);
                    }
                });
            },
            |err| eprintln!("An error occurred on the input stream: {}", err),
            None,
        )?;

        stream.play()?;
        
        // Keep the stream alive
        loop {
            time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn listen_reverse_audio(&self) -> Result<(), Box<dyn Error>> {
        let host = cpal::default_host();
        
        // In a real implementation, you would select the loopback device
        // For now, we'll just use a dummy implementation
        println!("Reverse audio capture (loopback) would be initialized here");
        
        if let Some(mut processor) = self.audio_processor.clone() {
            loop {
                // Dummy implementation - in a real app, this would read from the loopback device
                let dummy_data = vec![0u8; FRAME_SIZE_BYTES_16K];
                processor.process_reverse_stream(&dummy_data);
                time::sleep(Duration::from_millis(FRAME_DURATION_MS)).await;
            }
        }
        
        Ok(())
    }

    async fn play_audio(&self, mut rx: Receiver<Vec<u8>>) -> Result<(), Box<dyn Error>> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or("No output device available")?;
        
        println!("Using output device: {}", device.name()?);
        
        let config = StreamConfig {
            channels: CHANNELS,
            sample_rate: SampleRate(RECEIVE_SAMPLE_RATE),
            buffer_size: BufferSize::Default,
        };

        let last_playback_end = self.last_playback_end.clone();
        let (audio_tx, audio_rx) = mpsc::channel::<Vec<u8>>(5);

        let stream = device.build_output_stream(
            &config,
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                // This is called by the audio device when it needs more samples
                // In a real implementation, we would fill the buffer with
                // audio data received from Gemini
                
                // For simplicity, just generate silence
                for sample in data.iter_mut() {
                    *sample = 0;
                }
            },
            |err| eprintln!("An error occurred on the output stream: {}", err),
            None,
        )?;

        stream.play()?;

        // Process incoming audio and play it
        while let Some(audio_data) = rx.recv().await {
            println!("Playing {} bytes", audio_data.len());
            
            // In a real implementation, we would write the audio data to the output stream
            // For now, we'll just update the playback timestamp
            {
                let mut guard = last_playback_end.lock().unwrap();
                *guard = Instant::now();
            }
            
            // Simulate audio playback time
            let play_duration = Duration::from_millis(
                (audio_data.len() as u64 * 1000) / (RECEIVE_SAMPLE_RATE as u64 * 2)
            );
            time::sleep(play_duration).await;
        }
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load environment variables from .env file
    dotenv().ok();
    
    // Initialize
    speak_announcement("chat is ready");
    println!("Announcement complete. Starting Gemini session.");
    
    // Allow audio resources to settle
    time::sleep(Duration::from_secs(5)).await;
    
    // Create and run the voice chat
    let mut voice_chat = GeminiVoiceChat::new("Aoede", SYS_PROMPT);
    voice_chat.run().await?;
    
    println!("Audio resources released.");
    Ok(())
}