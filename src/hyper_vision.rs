//! Cooperative, allocation-free visual projection step for quantized tensors.

use crate::hyper_math::hyper_sin;
use crate::hyper_model::{GgmlType, GgufTensor};
use crate::terminal::VECTOR_BYTES;

pub const VISION_FEATURES: usize = VECTOR_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisionError {
    InvalidSpatialDimensions,
    ImageSizeMismatch,
    UnsupportedQuantization,
    InvalidQuantizedScale,
    ProjectionTooSmall,
    KvCacheFull,
}

/// Stateless one-token vision/text projection. All working memory is caller-owned.
pub struct HyperVisionEngine;

impl HyperVisionEngine {
    pub const fn new() -> Self {
        Self
    }

    /// Run one bounded cooperative step and append its 256-bit state to KV storage.
    ///
    /// `image` is a grayscale byte tensor. The projection view must contain at
    /// least 32 quantized rows of 32 weights each (Q4_0 or Q8_0). This is a
    /// compact projection primitive, not a complete Llama/Vision transformer.
    pub fn process_step(
        &self,
        text_vector: &[u8; VECTOR_BYTES],
        image: &[u8],
        width: u32,
        height: u32,
        projection: &GgufTensor<'_>,
        phase: f64,
        kv_cache: &mut [[u8; VECTOR_BYTES]],
        kv_cursor: &mut usize,
    ) -> Result<[u8; VECTOR_BYTES], VisionError> {
        if !width.is_power_of_two() || !height.is_power_of_two() {
            return Err(VisionError::InvalidSpatialDimensions);
        }
        let pixel_count = (width as usize)
            .checked_mul(height as usize)
            .ok_or(VisionError::ImageSizeMismatch)?;
        if image.len() != pixel_count {
            return Err(VisionError::ImageSizeMismatch);
        }
        if *kv_cursor >= kv_cache.len() {
            return Err(VisionError::KvCacheFull);
        }

        let (block_elements, block_bytes) = match projection.info.tensor_type {
            GgmlType::Q4_0 => (32usize, 18usize),
            GgmlType::Q8_0 => (32usize, 34usize),
            _ => return Err(VisionError::UnsupportedQuantization),
        };
        let required_bytes = VISION_FEATURES
            .checked_mul(block_bytes)
            .ok_or(VisionError::ProjectionTooSmall)?;
        if projection.data.len() < required_bytes || block_elements != 32 {
            return Err(VisionError::ProjectionTooSmall);
        }

        let gain = ((hyper_sin(phase) + 1.0) * 4.0) as i32;
        let mut output = [0u8; VECTOR_BYTES];
        for output_index in 0..VISION_FEATURES {
            let row_start = output_index * block_bytes;
            let scale_q8 = f16_scale_q8(projection.data[row_start], projection.data[row_start + 1])
                .ok_or(VisionError::InvalidQuantizedScale)?;
            let mut accumulator = 0i64;
            for feature_index in 0..VISION_FEATURES {
                let pixel_index = feature_index * pixel_count / VISION_FEATURES;
                let text_value = (text_vector[feature_index] >> 3) as i32 - 16;
                let pixel_value = (image[pixel_index] >> 3) as i32 - 16;
                let fused = (text_value + pixel_value).clamp(-32, 30);
                let weight = match projection.info.tensor_type {
                    GgmlType::Q4_0 => {
                        let packed = projection.data[row_start + 2 + (feature_index & 15)];
                        let nibble = if feature_index < 16 { packed & 0x0f } else { packed >> 4 };
                        nibble as i32 - 8
                    }
                    GgmlType::Q8_0 => {
                        projection.data[row_start + 2 + feature_index] as i8 as i32
                    }
                    _ => unreachable!(),
                };
                accumulator += fused as i64 * weight as i64 * scale_q8 as i64;
            }
            let scaled = ((accumulator * gain as i64) >> 11) as i32;
            output[output_index] = (scaled + 128).clamp(0, 255) as u8;
        }

        kv_cache[*kv_cursor] = output;
        *kv_cursor += 1;
        Ok(output)
    }
}

fn f16_scale_q8(low: u8, high: u8) -> Option<i32> {
    let bits = u16::from_le_bytes([low, high]);
    let sign = if bits & 0x8000 == 0 { 1i32 } else { -1i32 };
    let exponent = ((bits >> 10) & 0x1f) as i32;
    let fraction = (bits & 0x03ff) as i32;
    if exponent == 0x1f {
        return None;
    }

    if exponent == 0 {
        Some(sign * (fraction >> 16))
    } else {
        let significand = 1024 + fraction;
        let shift = exponent - 17;
        if shift >= 0 {
            Some(sign.saturating_mul(significand.checked_shl(shift as u32)?))
        } else {
            Some(sign * (significand >> (-shift as u32)))
        }
    }
}

impl Default for HyperVisionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{f16_scale_q8, HyperVisionEngine, VisionError};
    use crate::hyper_model::{GgmlType, GgufTensor, TensorInfo};
    use crate::terminal::VECTOR_BYTES;

    #[test]
    fn processes_quantized_projection_into_caller_kv_cache() {
        let mut weights = [0x11; 32 * 18];
        for row in 0..32 {
            weights[row * 18] = 0x00;
            weights[row * 18 + 1] = 0x3c;
        }
        let tensor = GgufTensor {
            info: TensorInfo {
                name: "vision.projection",
                dimensions: [32, 32, 1, 1],
                dimension_count: 2,
                tensor_type: GgmlType::Q4_0,
                relative_offset: 0,
                byte_length: weights.len(),
            },
            data: &weights,
        };
        let text = [0x80; VECTOR_BYTES];
        let image = [0x80; 64];
        let mut cache = [[0; VECTOR_BYTES]; 2];
        let mut cursor = 0;
        let output = HyperVisionEngine::new()
            .process_step(&text, &image, 8, 8, &tensor, 2.0, &mut cache, &mut cursor)
            .unwrap();
        assert_eq!(cursor, 1);
        assert_eq!(cache[0], output);
    }

    #[test]
    fn rejects_non_power_of_two_spatial_dimensions() {
        let weights = [0; 32 * 18];
        let tensor = GgufTensor {
            info: TensorInfo {
                name: "vision.projection",
                dimensions: [32, 32, 1, 1],
                dimension_count: 2,
                tensor_type: GgmlType::Q4_0,
                relative_offset: 0,
                byte_length: weights.len(),
            },
            data: &weights,
        };
        let mut cache = [[0; VECTOR_BYTES]; 1];
        let mut cursor = 0;
        let error = HyperVisionEngine::new().process_step(
            &[0; VECTOR_BYTES],
            &[0; 15],
            3,
            5,
            &tensor,
            0.0,
            &mut cache,
            &mut cursor,
        );
        assert_eq!(error, Err(VisionError::InvalidSpatialDimensions));
    }

    #[test]
    fn decodes_f16_scales_to_q8_without_floating_point() {
        assert_eq!(f16_scale_q8(0x00, 0x3c), Some(256));
        assert_eq!(f16_scale_q8(0x00, 0xbc), Some(-256));
        assert_eq!(f16_scale_q8(0x00, 0x7c), None);
    }
}