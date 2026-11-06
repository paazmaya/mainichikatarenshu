//! Validate the bootloader partition table before writing training data.

use core::ops::Range;
use md5::{Digest, Md5};

const ENTRY_SIZE: usize = 32;
const TABLE_SIZE: usize = 4096;
const RESERVED_END: u32 = 0x9000;
const SECTOR_SIZE: u32 = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartitionError {
    InvalidTable,
    InvalidChecksum,
    MissingTraining,
    DuplicateTraining,
    InvalidTraining,
    OverlappingPartitions,
}

fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn entry_range(entry: &[u8], capacity: u32) -> Result<Range<u32>, PartitionError> {
    let start = read_u32(&entry[4..8]);
    let size = read_u32(&entry[8..12]);
    let end = start
        .checked_add(size)
        .ok_or(PartitionError::InvalidTable)?;
    if start < RESERVED_END || size == 0 || end > capacity {
        return Err(PartitionError::InvalidTable);
    }
    Ok(start..end)
}

/// Return the training range only if the whole partition table is valid.
pub fn training_partition(table: &[u8], capacity: u32) -> Result<Range<u32>, PartitionError> {
    if table.len() != TABLE_SIZE {
        return Err(PartitionError::InvalidTable);
    }
    let mut count = 0;
    let mut training = None;
    let mut checksum_found = false;
    for (index, entry) in table.chunks_exact(ENTRY_SIZE).enumerate() {
        match (entry[0], entry[1]) {
            (0xaa, 0x50) if !checksum_found => {
                let range = entry_range(entry, capacity)?;
                for other in table[..index * ENTRY_SIZE].chunks_exact(ENTRY_SIZE) {
                    let other_range = entry_range(other, capacity)?;
                    if range.start < other_range.end && other_range.start < range.end {
                        return Err(PartitionError::OverlappingPartitions);
                    }
                }
                let label_end = entry[12..28]
                    .iter()
                    .position(|byte| *byte == 0)
                    .unwrap_or(16);
                if &entry[12..12 + label_end] == b"training" {
                    if training.is_some() {
                        return Err(PartitionError::DuplicateTraining);
                    }
                    if entry[2] != 1
                        || entry[3] != 0x06
                        || read_u32(&entry[28..32]) != 0
                        || range.start % SECTOR_SIZE != 0
                        || range.end % SECTOR_SIZE != 0
                        || range.end - range.start < 2 * SECTOR_SIZE
                    {
                        return Err(PartitionError::InvalidTraining);
                    }
                    training = Some(range);
                }
                count += 1;
            }
            (0xeb, 0xeb) if !checksum_found && count != 0 => {
                if entry[2..16].iter().any(|byte| *byte != 0xff)
                    || Md5::digest(&table[..index * ENTRY_SIZE]).as_slice() != &entry[16..32]
                {
                    return Err(PartitionError::InvalidChecksum);
                }
                checksum_found = true;
            }
            (0xff, 0xff) if checksum_found && entry.iter().all(|byte| *byte == 0xff) => {}
            _ => return Err(PartitionError::InvalidTable),
        }
    }
    if !checksum_found {
        return Err(PartitionError::InvalidTable);
    }
    training.ok_or(PartitionError::MissingTraining)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_entry(table: &mut [u8], index: usize, label: &[u8], start: u32, size: u32) {
        let entry = &mut table[index * ENTRY_SIZE..(index + 1) * ENTRY_SIZE];
        entry.fill(0);
        entry[0..4].copy_from_slice(&[0xaa, 0x50, 1, 0x06]);
        entry[4..8].copy_from_slice(&start.to_le_bytes());
        entry[8..12].copy_from_slice(&size.to_le_bytes());
        entry[12..12 + label.len()].copy_from_slice(label);
    }

    fn seal(table: &mut [u8], count: usize) {
        let offset = count * ENTRY_SIZE;
        let digest = Md5::digest(&table[..offset]);
        table[offset..offset + 2].copy_from_slice(&[0xeb, 0xeb]);
        table[offset + 16..offset + 32].copy_from_slice(&digest);
    }

    #[test]
    fn validates_reserved_partition_and_checksum() {
        let mut table = [0xff; TABLE_SIZE];
        write_entry(&mut table, 0, b"training", 0x420000, 0x20000);
        seal(&mut table, 1);
        assert_eq!(training_partition(&table, 0x800000), Ok(0x420000..0x440000));
        table[5] ^= 1;
        assert_eq!(
            training_partition(&table, 0x800000),
            Err(PartitionError::InvalidTraining)
        );
    }

    #[test]
    fn rejects_overlap_and_missing_checksum() {
        let mut table = [0xff; TABLE_SIZE];
        write_entry(&mut table, 0, b"other", 0x410000, 0x20000);
        write_entry(&mut table, 1, b"training", 0x420000, 0x20000);
        seal(&mut table, 2);
        assert_eq!(
            training_partition(&table, 0x800000),
            Err(PartitionError::OverlappingPartitions)
        );
        table.fill(0xff);
        write_entry(&mut table, 0, b"training", 0x420000, 0x20000);
        assert_eq!(
            training_partition(&table, 0x800000),
            Err(PartitionError::InvalidTable)
        );
    }

    #[test]
    fn accepts_espflash_partition_table() {
        let mut table = [0xff; TABLE_SIZE];
        write_entry(&mut table, 0, b"nvs", 0x9000, 0x8000);
        table[3] = 2;
        write_entry(&mut table, 1, b"phy_init", 0x11000, 0x8000);
        table[ENTRY_SIZE + 3] = 1;
        write_entry(&mut table, 2, b"factory", 0x20000, 0x400000);
        table[2 * ENTRY_SIZE + 2..2 * ENTRY_SIZE + 4].copy_from_slice(&[0, 0]);
        write_entry(&mut table, 3, b"training", 0x420000, 0x20000);
        let checksum_offset = 4 * ENTRY_SIZE;
        table[checksum_offset..checksum_offset + 2].copy_from_slice(&[0xeb, 0xeb]);
        table[checksum_offset + 16..checksum_offset + 32].copy_from_slice(&[
            0x97, 0x62, 0xd0, 0x9b, 0xbe, 0x71, 0x4d, 0x5a, 0xb9, 0x5b, 0x35, 0x4d, 0x2d, 0x6e,
            0x22, 0x3c,
        ]);
        assert_eq!(training_partition(&table, 0x800000), Ok(0x420000..0x440000));
    }
}
