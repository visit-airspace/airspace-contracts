use alloc::vec::Vec;
use pinocchio::error::ProgramError;

use crate::{errors::EscrowProgramError, require_len, traits::ExtensionData};

/// Block token extensions data (stored in TLV format)
///
/// Stores a list of Token-2022 ExtensionType values that should be blocked
/// for mints used with this escrow.
///
/// Uses dynamic sizing - serialized format is:
/// - 1 byte: count
/// - 2 bytes × count: each blocked extension
#[derive(Clone, Debug, PartialEq)]
pub struct BlockTokenExtensionsData {
    pub count: u8,
    pub blocked_extensions: Vec<u16>,
}

impl BlockTokenExtensionsData {
    pub fn new(blocked_extensions: &[u16]) -> Result<Self, ProgramError> {
        if blocked_extensions.len() > u8::MAX as usize {
            return Err(ProgramError::InvalidArgument);
        }

        let data = Self { count: blocked_extensions.len() as u8, blocked_extensions: blocked_extensions.to_vec() };
        assert_eq!(data.count as usize, data.blocked_extensions.len(), "blocklist count does not describe its entries");
        Ok(data)
    }

    /// Returns the serialized byte length: 1 (count) + 2 * count (extensions)
    pub fn byte_len(&self) -> usize {
        1 + (self.count as usize * 2)
    }

    /// Check if a token extension type is blocked
    pub fn is_blocked(&self, extension_type: u16) -> bool {
        self.blocked_extensions.contains(&extension_type)
    }

    /// Get the list of blocked extensions
    pub fn blocked_extensions(&self) -> &[u16] {
        &self.blocked_extensions
    }

    /// Add a single extension to the list
    ///
    /// Returns an error if the extension already exists
    pub fn add_extension(&mut self, extension: u16) -> Result<(), ProgramError> {
        if self.count as usize != self.blocked_extensions.len() {
            return Err(ProgramError::InvalidAccountData);
        }
        if self.is_blocked(extension) {
            return Err(EscrowProgramError::TokenExtensionAlreadyBlocked.into());
        }

        if self.count == u8::MAX {
            return Err(ProgramError::InvalidArgument);
        }

        let previous_len = self.blocked_extensions.len();
        self.blocked_extensions.push(extension);
        self.count += 1;
        assert_eq!(self.blocked_extensions.len(), previous_len + 1, "blocking an extension did not add one entry");
        assert_eq!(self.count as usize, self.blocked_extensions.len(), "blocklist count drifted during insertion");
        Ok(())
    }

    /// Remove a single extension from the list.
    ///
    /// Returns an error if the extension does not exist.
    pub fn remove_extension(&mut self, extension: u16) -> Result<(), ProgramError> {
        if self.count as usize != self.blocked_extensions.len() {
            return Err(ProgramError::InvalidAccountData);
        }
        let Some(index) = self.blocked_extensions.iter().position(|&ext| ext == extension) else {
            return Err(EscrowProgramError::TokenExtensionNotBlocked.into());
        };

        let previous_len = self.blocked_extensions.len();
        self.blocked_extensions.remove(index);
        self.count = self.count.checked_sub(1).ok_or(ProgramError::InvalidAccountData)?;
        assert_eq!(self.blocked_extensions.len() + 1, previous_len, "unblocking an extension did not remove one entry");
        assert_eq!(self.count as usize, self.blocked_extensions.len(), "blocklist count drifted during removal");
        Ok(())
    }
}

impl ExtensionData for BlockTokenExtensionsData {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.byte_len());
        bytes.push(self.count);
        for ext in &self.blocked_extensions[..self.count as usize] {
            bytes.extend_from_slice(&ext.to_le_bytes());
        }
        assert_eq!(bytes.len(), self.byte_len(), "blocklist encoder disagrees with its counted wire layout");
        assert_eq!(bytes[0], self.count, "blocklist encoder placed the count at the wrong offset");
        bytes
    }

    fn from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        require_len!(data, 1);

        let count = data[0];
        let expected_len = 1 + (count as usize * 2);

        require_len!(data, expected_len);

        let mut blocked_extensions = Vec::with_capacity(count as usize);
        for i in 0..count as usize {
            let offset = 1 + (i * 2);
            blocked_extensions.push(u16::from_le_bytes([data[offset], data[offset + 1]]));
        }

        let parsed = Self { count, blocked_extensions };
        assert_eq!(parsed.blocked_extensions.len(), count as usize, "blocklist decoder did not read every entry");
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    #[test]
    fn test_mismatched_count_cannot_partially_mutate_blocklist() {
        let mut data = BlockTokenExtensionsData { count: 0, blocked_extensions: vec![1, 2] };
        let original = data.clone();
        assert_eq!(data.add_extension(3), Err(ProgramError::InvalidAccountData));
        assert_eq!(data, original);
        assert_eq!(data.remove_extension(1), Err(ProgramError::InvalidAccountData));
        assert_eq!(data, original);
    }

    #[test]
    fn test_full_blocklist_rejects_insertion_without_mutation() {
        let entries: Vec<u16> = (0..u8::MAX as u16).collect();
        let mut data = BlockTokenExtensionsData::new(&entries).unwrap();
        let original = data.clone();
        assert_eq!(data.add_extension(u8::MAX as u16), Err(ProgramError::InvalidArgument));
        assert_eq!(data, original);
        assert_eq!(BlockTokenExtensionsData::from_bytes(&data.to_bytes()).unwrap(), original);
    }

    #[test]
    fn test_block_token_extensions_data_new() {
        let extensions = [1u16, 2u16, 3u16];
        let data = BlockTokenExtensionsData::new(&extensions).unwrap();
        assert_eq!(data.count, 3);
        assert_eq!(data.blocked_extensions[0], 1);
        assert_eq!(data.blocked_extensions[1], 2);
        assert_eq!(data.blocked_extensions[2], 3);
    }

    #[test]
    fn test_block_token_extensions_data_new_empty() {
        let data = BlockTokenExtensionsData::new(&[]).unwrap();
        assert_eq!(data.count, 0);
    }

    #[test]
    fn test_block_token_extensions_data_large_count() {
        let extensions: Vec<u16> = (0..100).collect();
        let data = BlockTokenExtensionsData::new(&extensions).unwrap();
        assert_eq!(data.count, 100);
        assert_eq!(data.blocked_extensions.len(), 100);
    }

    #[test]
    fn test_block_token_extensions_data_roundtrip() {
        let extensions = [5u16, 10u16, 15u16];
        let data = BlockTokenExtensionsData::new(&extensions).unwrap();
        let bytes = data.to_bytes();
        let parsed = BlockTokenExtensionsData::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, data);
    }

    #[test]
    fn test_block_token_extensions_data_roundtrip_large() {
        let extensions: Vec<u16> = (0..50).collect();
        let data = BlockTokenExtensionsData::new(&extensions).unwrap();
        let bytes = data.to_bytes();
        assert_eq!(bytes.len(), 1 + 50 * 2);
        let parsed = BlockTokenExtensionsData::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, data);
    }

    #[test]
    fn test_block_token_extensions_byte_len() {
        let data = BlockTokenExtensionsData::new(&[1, 2, 3]).unwrap();
        assert_eq!(data.byte_len(), 7);

        let empty = BlockTokenExtensionsData::new(&[]).unwrap();
        assert_eq!(empty.byte_len(), 1);
    }

    #[test]
    fn test_block_token_extensions_is_blocked() {
        let extensions = [1u16, 2u16, 3u16];
        let data = BlockTokenExtensionsData::new(&extensions).unwrap();
        assert!(data.is_blocked(1));
        assert!(data.is_blocked(2));
        assert!(data.is_blocked(3));
        assert!(!data.is_blocked(4));
    }

    #[test]
    fn test_block_token_extensions_blocked_extensions() {
        let extensions = [1u16, 2u16, 3u16];
        let data = BlockTokenExtensionsData::new(&extensions).unwrap();
        let blocked = data.blocked_extensions();
        assert_eq!(blocked, &[1u16, 2u16, 3u16]);
    }

    #[test]
    fn test_block_token_extensions_add_extension() {
        let mut data = BlockTokenExtensionsData::new(&[]).unwrap();
        assert_eq!(data.count, 0);

        data.add_extension(1u16).unwrap();
        assert_eq!(data.count, 1);
        assert_eq!(data.blocked_extensions[0], 1);

        data.add_extension(2u16).unwrap();
        assert_eq!(data.count, 2);
        assert_eq!(data.blocked_extensions[1], 2);
    }

    #[test]
    fn test_block_token_extensions_add_extension_duplicate() {
        let mut data = BlockTokenExtensionsData::new(&[1u16]).unwrap();
        let result = data.add_extension(1u16);
        assert!(result.is_err());
        assert_eq!(data.count, 1);
    }

    #[test]
    fn test_block_token_extensions_remove_extension() {
        let mut data = BlockTokenExtensionsData::new(&[1u16, 2u16, 3u16]).unwrap();
        data.remove_extension(2u16).unwrap();
        assert_eq!(data.count, 2);
        assert_eq!(data.blocked_extensions, vec![1u16, 3u16]);
    }

    #[test]
    fn test_block_token_extensions_remove_extension_last_item() {
        let mut data = BlockTokenExtensionsData::new(&[1u16]).unwrap();
        data.remove_extension(1u16).unwrap();
        assert_eq!(data.count, 0);
        assert!(data.blocked_extensions.is_empty());
    }

    #[test]
    fn test_block_token_extensions_remove_extension_missing() {
        let mut data = BlockTokenExtensionsData::new(&[1u16, 2u16]).unwrap();
        let result = data.remove_extension(3u16);
        assert!(result.is_err());
        assert_eq!(data.count, 2);
        assert_eq!(data.blocked_extensions, vec![1u16, 2u16]);
    }

    #[test]
    fn test_block_token_extensions_from_bytes_empty_fails() {
        let result = BlockTokenExtensionsData::from_bytes(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_block_token_extensions_from_bytes_truncated_fails() {
        let result = BlockTokenExtensionsData::from_bytes(&[3, 0, 0]);
        assert!(result.is_err());
    }
}
