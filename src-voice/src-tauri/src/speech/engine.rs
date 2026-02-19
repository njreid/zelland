use anyhow::{Context, Result};
use ndarray::{arr1, Array1, Array2, Array4, ArrayViewD, Ix4};
use ort::session::Session;
use ort::value::{Tensor, TensorRef};
use std::path::Path;
use tokenizers::Tokenizer;
use std::time::Instant;
use tracing::info;

const MAX_NEW_TOKENS: usize = 500;
// Moonshine special tokens
const SOT_TOKEN: i64 = 1;
const EOT_TOKEN: i64 = 2;

// Moonshine-tiny decoder: 6 layers, 8 heads, head_dim 36
const NUM_LAYERS: usize = 6;
const NUM_HEADS: usize = 8;
const HEAD_DIM: usize = 36;

// Encoder conv layers need minimum input length (~64*15 + 127 padding)
const MIN_AUDIO_SAMPLES: usize = 1600;

pub struct SpeechEngine {
    encoder: Session,
    decoder: Session,
    tokenizer: Tokenizer,
}

// ort::Session is Send+Sync, tokenizers::Tokenizer is Send+Sync
unsafe impl Send for SpeechEngine {}
unsafe impl Sync for SpeechEngine {}

impl SpeechEngine {
    pub fn new(model_dir: &Path) -> Result<Self> {
        let encoder_path = model_dir.join("encoder_model.onnx");
        let decoder_path = model_dir.join("decoder_model_merged.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");

        if !encoder_path.exists() {
            anyhow::bail!(
                "Encoder model not found at {}. Run scripts/download-model.sh",
                encoder_path.display()
            );
        }
        if !decoder_path.exists() {
            anyhow::bail!(
                "Decoder model not found at {}. Run scripts/download-model.sh",
                decoder_path.display()
            );
        }
        if !tokenizer_path.exists() {
            anyhow::bail!(
                "Tokenizer not found at {}. Run scripts/download-model.sh",
                tokenizer_path.display()
            );
        }

        info!("Loading encoder from {}", encoder_path.display());
        let encoder = Session::builder()?
            .with_intra_threads(4)?
            .commit_from_file(&encoder_path)
            .context("Failed to load encoder model")?;

        info!("Loading decoder from {}", decoder_path.display());
        let decoder = Session::builder()?
            .with_intra_threads(4)?
            .commit_from_file(&decoder_path)
            .context("Failed to load decoder model")?;

        info!("Loading tokenizer from {}", tokenizer_path.display());
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        info!("Speech engine loaded successfully");
        Ok(Self {
            encoder,
            decoder,
            tokenizer,
        })
    }

    /// Run a dummy transcription with silence to force ONNX Runtime graph optimization.
    pub fn warm_up(&mut self) {
        let start = Instant::now();
        let silence = vec![0.0f32; MIN_AUDIO_SAMPLES];
        match self.transcribe(&silence) {
            Ok(_) => info!("Model warm-up completed in {:?}", start.elapsed()),
            Err(e) => info!("Model warm-up failed (non-fatal): {}", e),
        }
    }

    pub fn transcribe(&mut self, samples: &[f32]) -> Result<String> {
        if samples.len() < MIN_AUDIO_SAMPLES {
            return Ok(String::new());
        }

        // Encode audio: input shape [1, num_samples]
        let num_samples = samples.len();
        let audio_input =
            Array2::from_shape_vec((1, num_samples), samples.to_vec())?.into_dyn();
        let audio_tensor = Tensor::from_array(audio_input)?;

        info!("Running encoder on {} samples", num_samples);
        let encoder_outputs = self.encoder.run(ort::inputs![audio_tensor])?;
        let encoder_hidden: ArrayViewD<'_, f32> = encoder_outputs[0].try_extract_array::<f32>()?;
        let encoder_hidden_owned = encoder_hidden.into_owned();
        let enc_seq_len = encoder_hidden_owned.shape()[1];
        info!(
            "Encoder output shape: {:?} (seq_len={})",
            encoder_hidden_owned.shape(),
            enc_seq_len
        );

        // Encoder attention mask: all 1s (no padding in PTT audio)
        let enc_attn_mask = Array1::from_vec(vec![1i64; enc_seq_len]);

        // Check if decoder expects encoder_attention_mask
        let decoder_expects_enc_mask = self
            .decoder
            .inputs()
            .iter()
            .any(|inp| inp.name() == "encoder_attention_mask");

        // Initialize KV cache: 6 layers x 4 tensors each (decoder key/value, encoder key/value)
        // First step: zero-length past sequence dimension (no cache yet)
        let mut past_kvs: Vec<Array4<f32>> = (0..NUM_LAYERS * 4)
            .map(|_| Array4::<f32>::zeros((1, NUM_HEADS, 0, HEAD_DIM)))
            .collect();

        let mut token_ids: Vec<i64> = vec![SOT_TOKEN];
        let mut use_cache = false;

        for _step in 0..MAX_NEW_TOKENS {
            // On first step: pass full token sequence, use_cache_branch=false
            // On subsequent steps: pass only the last token, use_cache_branch=true (use KV cache)
            let input_ids_data: Vec<i64>;
            let seq_len: usize;
            if use_cache {
                input_ids_data = vec![*token_ids.last().unwrap()];
                seq_len = 1;
            } else {
                input_ids_data = token_ids.clone();
                seq_len = token_ids.len();
            };

            let input_ids = Array2::from_shape_vec((1, seq_len), input_ids_data)?.into_dyn();
            let ids_tensor = Tensor::from_array(input_ids)?;
            let enc_ref = TensorRef::from_array_view(&encoder_hidden_owned)?;
            let use_cache_tensor = Tensor::from_array(arr1(&[use_cache]).into_dyn())?;

            let mut model_inputs = ort::inputs! {
                "input_ids" => ids_tensor,
                "encoder_hidden_states" => enc_ref,
                "use_cache_branch" => use_cache_tensor,
            };

            // Add encoder attention mask if the model expects it
            if decoder_expects_enc_mask {
                let mask_tensor =
                    TensorRef::from_array_view(enc_attn_mask.view().into_dyn())?;
                model_inputs.push((
                    "encoder_attention_mask".into(),
                    mask_tensor.into(),
                ));
            }

            // Add KV cache inputs for each layer
            for i in 0..NUM_LAYERS {
                let dk_ref = TensorRef::from_array_view(&past_kvs[i * 4])?;
                let dv_ref = TensorRef::from_array_view(&past_kvs[i * 4 + 1])?;
                let ek_ref = TensorRef::from_array_view(&past_kvs[i * 4 + 2])?;
                let ev_ref = TensorRef::from_array_view(&past_kvs[i * 4 + 3])?;

                model_inputs.push((
                    format!("past_key_values.{}.decoder.key", i).into(),
                    dk_ref.into(),
                ));
                model_inputs.push((
                    format!("past_key_values.{}.decoder.value", i).into(),
                    dv_ref.into(),
                ));
                model_inputs.push((
                    format!("past_key_values.{}.encoder.key", i).into(),
                    ek_ref.into(),
                ));
                model_inputs.push((
                    format!("past_key_values.{}.encoder.value", i).into(),
                    ev_ref.into(),
                ));
            }

            let outputs = self.decoder.run(model_inputs)?;

            // Extract logits: [1, seq_len, vocab_size]
            let logits = outputs["logits"].try_extract_array::<f32>()?;
            let shape = logits.shape();
            let vocab_size = shape[shape.len() - 1];
            let last_pos = shape[1] - 1;

            // Greedy decode: argmax of last position
            let mut max_val = f32::NEG_INFINITY;
            let mut max_idx: i64 = 0;
            for v in 0..vocab_size {
                let val = logits[[0, last_pos, v]];
                if val > max_val {
                    max_val = val;
                    max_idx = v as i64;
                }
            }

            if max_idx == EOT_TOKEN {
                break;
            }

            token_ids.push(max_idx);

            // Update KV cache from "present" outputs
            // Encoder KV cache only changes on first iteration; decoder cache every step
            for i in 0..NUM_LAYERS {
                past_kvs[i * 4] = outputs[format!("present.{}.decoder.key", i)]
                    .try_extract_array::<f32>()?
                    .into_dimensionality::<Ix4>()?
                    .to_owned();
                past_kvs[i * 4 + 1] = outputs[format!("present.{}.decoder.value", i)]
                    .try_extract_array::<f32>()?
                    .into_dimensionality::<Ix4>()?
                    .to_owned();
                if !use_cache {
                    past_kvs[i * 4 + 2] = outputs[format!("present.{}.encoder.key", i)]
                        .try_extract_array::<f32>()?
                        .into_dimensionality::<Ix4>()?
                        .to_owned();
                    past_kvs[i * 4 + 3] = outputs[format!("present.{}.encoder.value", i)]
                        .try_extract_array::<f32>()?
                        .into_dimensionality::<Ix4>()?
                        .to_owned();
                }
            }

            use_cache = true;
        }

        // Decode tokens (skip SOT)
        let decode_ids: Vec<u32> = token_ids[1..]
            .iter()
            .map(|&id| id as u32)
            .collect();

        let text = self
            .tokenizer
            .decode(&decode_ids, true)
            .map_err(|e| anyhow::anyhow!("Tokenizer decode error: {}", e))?;

        info!("Transcribed: {:?}", text);
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_dir() -> std::path::PathBuf {
        let xdg_data = std::env::var("XDG_DATA_HOME")
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap();
                format!("{}/.local/share", home)
            });
        std::path::PathBuf::from(xdg_data).join("voice-terminal/models/moonshine-tiny")
    }

    #[test]
    fn test_engine_loads() {
        let dir = model_dir();
        if !dir.exists() {
            eprintln!("Skipping test: model not downloaded. Run scripts/download-model.sh");
            return;
        }
        let engine = SpeechEngine::new(&dir);
        assert!(engine.is_ok(), "Engine failed to load: {:?}", engine.err());
    }

    #[test]
    fn test_transcribe_too_short() {
        let dir = model_dir();
        if !dir.exists() {
            return;
        }
        let mut engine = SpeechEngine::new(&dir).unwrap();
        let result = engine.transcribe(&[0.0; 500]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_transcribe_dummy_audio() {
        let dir = model_dir();
        if !dir.exists() {
            eprintln!("Skipping: model not downloaded");
            return;
        }
        let mut engine = SpeechEngine::new(&dir).unwrap();
        // 1 second of silence at 16kHz
        let samples = vec![0.0f32; 16000];
        let result = engine.transcribe(&samples);
        assert!(result.is_ok(), "Transcription failed: {:?}", result.err());
    }
}
