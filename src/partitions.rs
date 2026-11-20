//! Validate the bootloader partition table before writing training data.

use core::ops::Range;
use md5::{Digest, Md5};

const ENTRY_SIZE: usize = 32;
const TABLE_SIZE: usize = 4096;
const RESERVED_END: u32 = 0x9000;
const SECTOR_SIZE: u32 = 4096;

/// The partition table is unusable for training storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartitionError {
    /// The table is malformed or an entry lies outside the flash.
    InvalidTable,
    /// The MD5 checksum entry does not match the table.
    InvalidChecksum,
    /// No partition is labelled `training`.
    MissingTraining,
    /// More than one partition is labelled `training`.
    DuplicateTraining,
    /// The `training` partition has the wrong type, flags, alignment or size.
    InvalidTraining,
    /// Two partitions overlap.
    OverlappingPartitions,
}

type Entry = [u8; ENTRY_SIZE];

fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// Flash range of a partition entry, checked against the reserved area and capacity.
fn entry_range(entry: &Entry, capacity: u32) -> Result<Range<u32>, PartitionError> {
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

/// The NUL-terminated partition label.
fn entry_label(entry: &Entry) -> &[u8] {
    let label = &entry[12..28];
    let end = label
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(label.len());
    &label[..end]
}

/// Require a data partition of subtype `0x06`, without flags, spanning at least two sectors.
fn validate_training_entry(entry: &Entry, range: &Range<u32>) -> Result<(), PartitionError> {
    let valid = entry[2] == 1
        && entry[3] == 0x06
        && read_u32(&entry[28..32]) == 0
        && range.start.is_multiple_of(SECTOR_SIZE)
        && range.end.is_multiple_of(SECTOR_SIZE)
        && range.end - range.start >= 2 * SECTOR_SIZE;
    valid.then_some(()).ok_or(PartitionError::InvalidTraining)
}

/// Reject `range` if it overlaps any of the `previous` entries.
fn check_no_overlap(
    previous: &[Entry],
    range: &Range<u32>,
    capacity: u32,
) -> Result<(), PartitionError> {
    for other in previous {
        let other = entry_range(other, capacity)?;
        if range.start < other.end && other.start < range.end {
            return Err(PartitionError::OverlappingPartitions);
        }
    }
    Ok(())
}

/// Validate the MD5 entry against all bytes before it.
fn validate_checksum(entry: &Entry, preceding: &[u8]) -> Result<(), PartitionError> {
    if entry[2..16].iter().any(|byte| *byte != 0xff)
        || Md5::digest(preceding).as_slice() != &entry[16..32]
    {
        return Err(PartitionError::InvalidChecksum);
    }
    Ok(())
}

/// Find the `training` partition range in a 4 KiB ESP partition table.
///
/// The range is returned only if the whole table is valid: every entry lies
/// within `capacity` and outside the bootloader area, no entries overlap, and
/// the MD5 checksum entry matches.
pub fn find_training_partition(table: &[u8], capacity: u32) -> Result<Range<u32>, PartitionError> {
    if table.len() != TABLE_SIZE {
        return Err(PartitionError::InvalidTable);
    }
    let (entries, _) = table.as_chunks::<ENTRY_SIZE>();
    let mut training = None;
    let mut checksum_found = false;
    for (index, entry) in entries.iter().enumerate() {
        match (entry[0], entry[1]) {
            (0xaa, 0x50) if !checksum_found => {
                let range = entry_range(entry, capacity)?;
                check_no_overlap(&entries[..index], &range, capacity)?;
                if entry_label(entry) == b"training" {
                    if training.is_some() {
                        return Err(PartitionError::DuplicateTraining);
                    }
                    validate_training_entry(entry, &range)?;
                    training = Some(range);
                }
            }
            (0xeb, 0xeb) if !checksum_found && index != 0 => {
                validate_checksum(entry, &table[..index * ENTRY_SIZE])?;
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
        assert_eq!(
            find_training_partition(&table, 0x800000),
            Ok(0x420000..0x440000)
        );
        table[5] ^= 1;
        assert_eq!(
            find_training_partition(&table, 0x800000),
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
            find_training_partition(&table, 0x800000),
            Err(PartitionError::OverlappingPartitions)
        );
        table.fill(0xff);
        write_entry(&mut table, 0, b"training", 0x420000, 0x20000);
        assert_eq!(
            find_training_partition(&table, 0x800000),
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
        assert_eq!(
            find_training_partition(&table, 0x800000),
            Ok(0x420000..0x440000)
        );
    }
}
