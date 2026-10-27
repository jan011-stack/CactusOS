//! Bounds-checked, allocation-free GGUF parsing over caller-provided mapped bytes.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GgufError {
    Truncated,
    BadMagic,
    UnsupportedVersion,
    InvalidUtf8,
    InvalidMetadataType,
    InvalidTensorDimensions,
    UnsupportedTensorType,
    MisalignedTensor,
    InvalidAlignment,
    SizeOverflow,
    TensorOutOfBounds,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GgmlType {
    F32,
    F16,
    Q4_0,
    Q4_1,
    Q5_0,
    Q5_1,
    Q8_0,
    Q2K,
    Q3K,
    Q4K,
    Q5K,
    Q6K,
    Q8K,
}

impl GgmlType {
    fn from_id(id: u32) -> Result<Self, GgufError> {
        match id {
            0 => Ok(Self::F32),
            1 => Ok(Self::F16),
            2 => Ok(Self::Q4_0),
            3 => Ok(Self::Q4_1),
            6 => Ok(Self::Q5_0),
            7 => Ok(Self::Q5_1),
            8 => Ok(Self::Q8_0),
            10 => Ok(Self::Q2K),
            11 => Ok(Self::Q3K),
            12 => Ok(Self::Q4K),
            13 => Ok(Self::Q5K),
            14 => Ok(Self::Q6K),
            15 => Ok(Self::Q8K),
            _ => Err(GgufError::UnsupportedTensorType),
        }
    }

    pub const fn block_layout(self) -> (u64, u64) {
        match self {
            Self::F32 => (1, 4),
            Self::F16 => (1, 2),
            Self::Q4_0 => (32, 18),
            Self::Q4_1 => (32, 20),
            Self::Q5_0 => (32, 22),
            Self::Q5_1 => (32, 24),
            Self::Q8_0 => (32, 34),
            Self::Q2K => (256, 84),
            Self::Q3K => (256, 110),
            Self::Q4K => (256, 144),
            Self::Q5K => (256, 176),
            Self::Q6K => (256, 210),
            Self::Q8K => (256, 292),
        }
    }

    pub const fn bits_per_weight(self) -> Option<u8> {
        match self {
            Self::Q4_0 | Self::Q4_1 | Self::Q4K => Some(4),
            Self::Q8_0 | Self::Q8K => Some(8),
            Self::Q5_0 | Self::Q5_1 | Self::Q5K => Some(5),
            Self::Q2K => Some(2),
            Self::Q3K => Some(3),
            Self::Q6K => Some(6),
            Self::F16 | Self::F32 => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TensorInfo<'a> {
    pub name: &'a str,
    pub dimensions: [u64; 4],
    pub dimension_count: u32,
    pub tensor_type: GgmlType,
    pub relative_offset: u64,
    pub byte_length: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct GgufTensor<'a> {
    pub info: TensorInfo<'a>,
    pub data: &'a [u8],
}

/// Public loader name for the parsed, borrowed GGUF mapping.
pub type HyperModelLoader<'a> = GgufFile<'a>;

impl<'a> GgufTensor<'a> {
    /// View tensor storage as fixed power-of-two blocks without copying it.
    pub fn blocks<const BLOCK_SIZE: usize>(&self) -> Result<TensorBlocks<'a>, GgufError> {
        if BLOCK_SIZE == 0 || !BLOCK_SIZE.is_power_of_two() {
            return Err(GgufError::InvalidAlignment);
        }
        Ok(TensorBlocks {
            data: self.data,
            block_size: BLOCK_SIZE,
            offset: 0,
        })
    }
}

pub struct TensorBlocks<'a> {
    data: &'a [u8],
    block_size: usize,
    offset: usize,
}

impl<'a> Iterator for TensorBlocks<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.data.len() {
            return None;
        }
        let end = self
            .offset
            .saturating_add(self.block_size)
            .min(self.data.len());
        let block = &self.data[self.offset..end];
        self.offset = end;
        Some(block)
    }
}

pub struct GgufFile<'a> {
    bytes: &'a [u8],
    tensor_count: usize,
    tensor_info_offset: usize,
    data_offset: usize,
    alignment: usize,
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], GgufError> {
        let end = self.position.checked_add(length).ok_or(GgufError::SizeOverflow)?;
        let slice = self.bytes.get(self.position..end).ok_or(GgufError::Truncated)?;
        self.position = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, GgufError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, GgufError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, GgufError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, GgufError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn string(&mut self) -> Result<&'a str, GgufError> {
        let length = usize::try_from(self.u64()?).map_err(|_| GgufError::SizeOverflow)?;
        core::str::from_utf8(self.take(length)?).map_err(|_| GgufError::InvalidUtf8)
    }
}

impl<'a> GgufFile<'a> {
    /// Parse a GGUF v2/v3 file supplied as bytes, typically an OS mmap slice.
    /// This function borrows the mapping and does not allocate or copy weights.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, GgufError> {
        let mut reader = Reader { bytes, position: 0 };
        if reader.take(4)? != b"GGUF" {
            return Err(GgufError::BadMagic);
        }
        let version = reader.u32()?;
        if version != 2 && version != 3 {
            return Err(GgufError::UnsupportedVersion);
        }
        let tensor_count = usize::try_from(reader.u64()?).map_err(|_| GgufError::SizeOverflow)?;
        let metadata_count = usize::try_from(reader.u64()?).map_err(|_| GgufError::SizeOverflow)?;
        let mut alignment = 32usize;

        for _ in 0..metadata_count {
            let key = reader.string()?;
            let value_type = reader.u32()?;
            let numeric_value = skip_metadata_value(&mut reader, value_type)?;
            if key == "general.alignment" {
                let value = numeric_value.ok_or(GgufError::InvalidAlignment)?;
                alignment = usize::try_from(value).map_err(|_| GgufError::InvalidAlignment)?;
                if alignment == 0 || !alignment.is_power_of_two() || alignment > (1 << 20) {
                    return Err(GgufError::InvalidAlignment);
                }
            }
        }

        let tensor_info_offset = reader.position;
        for _ in 0..tensor_count {
            read_tensor_info(&mut reader)?;
        }
        let data_offset = align_up(reader.position, alignment)?;
        if data_offset > bytes.len() {
            return Err(GgufError::Truncated);
        }

        let mut validation = Reader {
            bytes,
            position: tensor_info_offset,
        };
        for _ in 0..tensor_count {
            let info = read_tensor_info(&mut validation)?;
            if info.relative_offset % alignment as u64 != 0 {
                return Err(GgufError::MisalignedTensor);
            }
            let start = usize::try_from(info.relative_offset).map_err(|_| GgufError::SizeOverflow)?;
            let end = start.checked_add(info.byte_length).ok_or(GgufError::SizeOverflow)?;
            let absolute_end = data_offset.checked_add(end).ok_or(GgufError::SizeOverflow)?;
            if absolute_end > bytes.len() {
                return Err(GgufError::TensorOutOfBounds);
            }
        }

        Ok(Self {
            bytes,
            tensor_count,
            tensor_info_offset,
            data_offset,
            alignment,
        })
    }

    /// Parse a memory-mapped GGUF file without taking ownership of the mapping.
    pub fn load_from_mapping(mapped_bytes: &'a [u8]) -> Result<Self, GgufError> {
        Self::parse(mapped_bytes)
    }

    pub const fn tensor_count(&self) -> usize {
        self.tensor_count
    }

    pub const fn alignment(&self) -> usize {
        self.alignment
    }

    pub fn tensors(&'a self) -> TensorIter<'a> {
        TensorIter {
            file: self,
            reader: Reader {
                bytes: self.bytes,
                position: self.tensor_info_offset,
            },
            remaining: self.tensor_count,
        }
    }

    /// Enforce power-of-two spatial token dimensions for vision tensors.
    pub fn validate_spatial_dimensions(width: u64, height: u64) -> bool {
        width.is_power_of_two() && height.is_power_of_two()
    }
}

pub struct TensorIter<'a> {
    file: &'a GgufFile<'a>,
    reader: Reader<'a>,
    remaining: usize,
}

impl<'a> Iterator for TensorIter<'a> {
    type Item = Result<GgufTensor<'a>, GgufError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        Some((|| {
            let info = read_tensor_info(&mut self.reader)?;
            let start = self
                .file
                .data_offset
                .checked_add(usize::try_from(info.relative_offset).map_err(|_| GgufError::SizeOverflow)?)
                .ok_or(GgufError::SizeOverflow)?;
            let end = start.checked_add(info.byte_length).ok_or(GgufError::SizeOverflow)?;
            let data = self.file.bytes.get(start..end).ok_or(GgufError::TensorOutOfBounds)?;
            Ok(GgufTensor { info, data })
        })())
    }
}

fn read_tensor_info<'a>(reader: &mut Reader<'a>) -> Result<TensorInfo<'a>, GgufError> {
    let name = reader.string()?;
    let dimension_count = reader.u32()?;
    if dimension_count == 0 || dimension_count > 4 {
        return Err(GgufError::InvalidTensorDimensions);
    }
    let mut dimensions = [1u64; 4];
    let mut element_count = 1u64;
    for dimension in dimensions.iter_mut().take(dimension_count as usize) {
        *dimension = reader.u64()?;
        if *dimension == 0 {
            return Err(GgufError::InvalidTensorDimensions);
        }
        element_count = element_count
            .checked_mul(*dimension)
            .ok_or(GgufError::SizeOverflow)?;
    }
    let tensor_type = GgmlType::from_id(reader.u32()?)?;
    let relative_offset = reader.u64()?;
    let (block_elements, block_bytes) = tensor_type.block_layout();
    if element_count % block_elements != 0 {
        return Err(GgufError::InvalidTensorDimensions);
    }
    let byte_length = usize::try_from(
        element_count
            .checked_div(block_elements)
            .and_then(|blocks| blocks.checked_mul(block_bytes))
            .ok_or(GgufError::SizeOverflow)?,
    )
    .map_err(|_| GgufError::SizeOverflow)?;

    Ok(TensorInfo {
        name,
        dimensions,
        dimension_count,
        tensor_type,
        relative_offset,
        byte_length,
    })
}

fn skip_metadata_value(reader: &mut Reader<'_>, value_type: u32) -> Result<Option<u64>, GgufError> {
    match value_type {
        0 => { reader.u8()?; Ok(None) }
        1 => { reader.take(1)?; Ok(None) }
        2 => { reader.u16()?; Ok(None) }
        3 => { reader.take(2)?; Ok(None) }
        4 => Ok(Some(reader.u32()? as u64)),
        5 => { reader.take(4)?; Ok(None) }
        6 => { reader.take(4)?; Ok(None) }
        7 => {
            if reader.u8()? > 1 { return Err(GgufError::InvalidMetadataType); }
            Ok(None)
        }
        8 => { reader.string()?; Ok(None) }
        9 => {
            let element_type = reader.u32()?;
            let count = usize::try_from(reader.u64()?).map_err(|_| GgufError::SizeOverflow)?;
            for _ in 0..count {
                skip_array_element(reader, element_type)?;
            }
            Ok(None)
        }
        10 => Ok(Some(reader.u64()?)),
        11 | 12 => { reader.take(8)?; Ok(None) }
        _ => Err(GgufError::InvalidMetadataType),
    }
}

fn skip_array_element(reader: &mut Reader<'_>, value_type: u32) -> Result<(), GgufError> {
    match value_type {
        0 => { reader.take(1)?; }
        1 => { reader.take(1)?; }
        2 => { reader.take(2)?; }
        3 => { reader.take(2)?; }
        4 | 5 | 6 => { reader.take(4)?; }
        7 => { reader.take(1)?; }
        8 => { reader.string()?; }
        10 | 11 | 12 => { reader.take(8)?; }
        _ => return Err(GgufError::InvalidMetadataType),
    }
    Ok(())
}

fn align_up(value: usize, alignment: usize) -> Result<usize, GgufError> {
    value
        .checked_add(alignment - 1)
        .map(|sum| sum & !(alignment - 1))
        .ok_or(GgufError::SizeOverflow)
}

#[cfg(test)]
mod tests {
    use super::{GgufError, GgufFile, GgmlType};
    use std::vec::Vec;

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u64(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn parses_quantized_tensor_as_zero_copy_view() {
        let mut mapped = Vec::new();
        mapped.extend_from_slice(b"GGUF");
        push_u32(&mut mapped, 3);
        push_u64(&mut mapped, 1);
        push_u64(&mut mapped, 0);
        push_u64(&mut mapped, 6);
        mapped.extend_from_slice(b"weight");
        push_u32(&mut mapped, 1);
        push_u64(&mut mapped, 32);
        push_u32(&mut mapped, 2);
        push_u64(&mut mapped, 0);
        mapped.resize(64, 0);
        mapped.extend_from_slice(&[0x5a; 18]);

        let file = GgufFile::parse(&mapped).unwrap();
        let tensor = file.tensors().next().unwrap().unwrap();
        assert_eq!(tensor.info.name, "weight");
        assert_eq!(tensor.info.tensor_type, GgmlType::Q4_0);
        assert_eq!(tensor.data.len(), 18);
        assert_eq!(tensor.data.as_ptr(), mapped[64..].as_ptr());
        let blocks = tensor.blocks::<16>().unwrap().collect::<Vec<_>>();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].as_ptr(), tensor.data.as_ptr());
    }

    #[test]
    fn rejects_truncated_tensor_data_and_invalid_spatial_sizes() {
        let mut truncated = Vec::new();
        truncated.extend_from_slice(b"GGUF");
        push_u32(&mut truncated, 3);
        push_u64(&mut truncated, 1);
        push_u64(&mut truncated, 0);
        push_u64(&mut truncated, 1);
        truncated.push(b'x');
        push_u32(&mut truncated, 1);
        push_u64(&mut truncated, 32);
        push_u32(&mut truncated, 2);
        push_u64(&mut truncated, 0);
        truncated.resize(64, 0);
        truncated.extend_from_slice(&[0; 17]);

        assert_eq!(GgufFile::parse(&truncated).err(), Some(GgufError::TensorOutOfBounds));
        assert!(GgufFile::validate_spatial_dimensions(16, 32));
        assert!(!GgufFile::validate_spatial_dimensions(15, 32));
    }
}