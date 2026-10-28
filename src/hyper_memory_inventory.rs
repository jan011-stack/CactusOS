//! Read-only inventory of physical memory ranges reported by Multiboot 1.

pub const MAX_MEMORY_REGIONS: usize = 64;
pub const MAX_MEMORY_MAP_BYTES: usize = 64 * 1024;
pub const MULTIBOOT1_AVAILABLE_MEMORY: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryRegion {
    pub base: u64,
    pub length: u64,
    pub kind: u32,
}

impl MemoryRegion {
    const EMPTY: Self = Self {
        base: 0,
        length: 0,
        kind: 0,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryInventory {
    regions: [MemoryRegion; MAX_MEMORY_REGIONS],
    count: usize,
    usable_bytes: u64,
    non_usable_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryError {
    EmptyMap,
    MapTooLarge,
    TruncatedEntry,
    InvalidEntrySize,
    TooManyRegions,
    EmptyRegion,
    AddressOverflow,
    TotalOverflow,
}

/// Parse a Multiboot 1 memory-map payload without reading the memory ranges.
/// Type 1 is usable RAM; all other types remain visible but are not allocatable.
pub fn parse_memory_map(bytes: &[u8]) -> Result<MemoryInventory, InventoryError> {
    if bytes.is_empty() {
        return Err(InventoryError::EmptyMap);
    }
    if bytes.len() > MAX_MEMORY_MAP_BYTES {
        return Err(InventoryError::MapTooLarge);
    }

    let mut inventory = MemoryInventory {
        regions: [MemoryRegion::EMPTY; MAX_MEMORY_REGIONS],
        count: 0,
        usable_bytes: 0,
        non_usable_bytes: 0,
    };
    let mut offset = 0usize;

    while offset < bytes.len() {
        let size_end = offset
            .checked_add(4)
            .ok_or(InventoryError::AddressOverflow)?;
        let size_bytes = bytes
            .get(offset..size_end)
            .ok_or(InventoryError::TruncatedEntry)?;
        let entry_size = u32::from_le_bytes(size_bytes.try_into().unwrap()) as usize;
        if entry_size < 20 {
            return Err(InventoryError::InvalidEntrySize);
        }
        let entry_end = size_end
            .checked_add(entry_size)
            .ok_or(InventoryError::AddressOverflow)?;
        let entry = bytes
            .get(size_end..entry_end)
            .ok_or(InventoryError::TruncatedEntry)?;
        if inventory.count == MAX_MEMORY_REGIONS {
            return Err(InventoryError::TooManyRegions);
        }

        let base = u64::from_le_bytes(entry[0..8].try_into().unwrap());
        let length = u64::from_le_bytes(entry[8..16].try_into().unwrap());
        let kind = u32::from_le_bytes(entry[16..20].try_into().unwrap());
        if length == 0 {
            return Err(InventoryError::EmptyRegion);
        }
        base.checked_add(length)
            .ok_or(InventoryError::AddressOverflow)?;

        let total = if kind == MULTIBOOT1_AVAILABLE_MEMORY {
            &mut inventory.usable_bytes
        } else {
            &mut inventory.non_usable_bytes
        };
        *total = total
            .checked_add(length)
            .ok_or(InventoryError::TotalOverflow)?;
        inventory.regions[inventory.count] = MemoryRegion { base, length, kind };
        inventory.count += 1;
        offset = entry_end;
    }

    Ok(inventory)
}

impl MemoryInventory {
    pub const fn region_count(&self) -> usize {
        self.count
    }

    pub fn region(&self, index: usize) -> Option<MemoryRegion> {
        self.regions.get(index).copied().filter(|_| index < self.count)
    }

    pub const fn usable_bytes(&self) -> u64 {
        self.usable_bytes
    }

    pub const fn non_usable_bytes(&self) -> u64 {
        self.non_usable_bytes
    }

    pub fn contains_usable_range(&self, base: u64, length: u64) -> bool {
        let Some(end) = base.checked_add(length) else {
            return false;
        };
        self.regions[..self.count].iter().any(|region| {
            if region.kind != MULTIBOOT1_AVAILABLE_MEMORY {
                return false;
            }
            let Some(region_end) = region.base.checked_add(region.length) else {
                return false;
            };
            base >= region.base && end <= region_end
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_memory_map, InventoryError, MULTIBOOT1_AVAILABLE_MEMORY};

    fn write_entry(
        bytes: &mut [u8],
        offset: usize,
        base: u64,
        length: u64,
        kind: u32,
    ) -> usize {
        bytes[offset..offset + 4].copy_from_slice(&20u32.to_le_bytes());
        bytes[offset + 4..offset + 12].copy_from_slice(&base.to_le_bytes());
        bytes[offset + 12..offset + 20].copy_from_slice(&length.to_le_bytes());
        bytes[offset + 20..offset + 24].copy_from_slice(&kind.to_le_bytes());
        offset + 24
    }

    #[test]
    fn inventory_preserves_all_ranges_and_sums_usable_and_non_usable_ranges() {
        let mut bytes = [0; 48];
        write_entry(&mut bytes, 0, 0, 100, MULTIBOOT1_AVAILABLE_MEMORY);
        write_entry(&mut bytes, 24, 100, 50, 2);

        let inventory = parse_memory_map(&bytes).unwrap();
        assert_eq!(inventory.region_count(), 2);
        assert_eq!(inventory.usable_bytes(), 100);
        assert_eq!(inventory.non_usable_bytes(), 50);
        assert!(inventory.contains_usable_range(10, 20));
        assert!(!inventory.contains_usable_range(90, 20));
        assert_eq!(inventory.region(1).unwrap().kind, 2);
    }

    #[test]
    fn malformed_memory_map_entries_are_rejected() {
        assert_eq!(parse_memory_map(&[]), Err(InventoryError::EmptyMap));
        assert_eq!(parse_memory_map(&[20, 0, 0, 0]), Err(InventoryError::TruncatedEntry));

        let mut invalid_size = [0; 24];
        invalid_size[0..4].copy_from_slice(&19u32.to_le_bytes());
        assert_eq!(
            parse_memory_map(&invalid_size),
            Err(InventoryError::InvalidEntrySize)
        );
    }

    #[test]
    fn range_and_total_overflow_are_rejected() {
        let mut bytes = [0; 24];
        write_entry(&mut bytes, 0, u64::MAX - 2, 4, MULTIBOOT1_AVAILABLE_MEMORY);
        assert_eq!(
            parse_memory_map(&bytes),
            Err(InventoryError::AddressOverflow)
        );

        let mut bytes = [0; 48];
        write_entry(&mut bytes, 0, 0, u64::MAX, MULTIBOOT1_AVAILABLE_MEMORY);
        write_entry(&mut bytes, 24, 0, 1, MULTIBOOT1_AVAILABLE_MEMORY);
        assert_eq!(parse_memory_map(&bytes), Err(InventoryError::TotalOverflow));
    }
}