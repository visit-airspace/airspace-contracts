use alloc::vec::Vec;
use pinocchio::{error::ProgramError, Address, ProgramResult};

use crate::{assert_no_padding, errors::EscrowProgramError, require_len, traits::ExtensionData};

/// The shortest gap between the deposit deadline and the escape time: 7 days.
pub const MIN_ESCAPE_GAP_SECONDS: i64 = 7 * 24 * 60 * 60;

/// Settlement extension data (stored in TLV format)
///
/// AirSpace's addition to the upstream escrow. It turns an escrow into one that its
/// admin settles: deposits stop at the deposit deadline, a depositor cannot withdraw
/// before the escape time, and the admin can only refund a receipt to its depositor or
/// settle it to the beneficiary. It is set once and never changes or goes away, so a
/// depositor who reads it on chain knows the rules before depositing.
///
/// Upstream already has an `Arbiter` extension, which names a signer that must
/// authorize withdrawals. This one is separate and does not change it.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct SettlementData {
    pub beneficiary: Address,
    pub deposit_deadline: i64,
    pub escape_time: i64,
}

assert_no_padding!(SettlementData, 32 + 8 + 8);

impl SettlementData {
    pub const LEN: usize = 32 + 8 + 8;

    pub fn new(beneficiary: Address, deposit_deadline: i64, escape_time: i64) -> Self {
        Self { beneficiary, deposit_deadline, escape_time }
    }

    /// The terms are usable when the escape time lies at least 7 days after the deposit
    /// deadline, so an admin who vanishes can never trap a deposit for good.
    pub fn validate_terms(&self) -> ProgramResult {
        let earliest_escape =
            self.deposit_deadline.checked_add(MIN_ESCAPE_GAP_SECONDS).ok_or(ProgramError::ArithmeticOverflow)?;
        if self.escape_time < earliest_escape {
            return Err(EscrowProgramError::InvalidSettlementTerms.into());
        }
        Ok(())
    }

    /// A deposit is refused after the deposit deadline.
    pub fn validate_deposit(&self, now: i64) -> ProgramResult {
        if now > self.deposit_deadline {
            return Err(EscrowProgramError::DepositDeadlinePassed.into());
        }
        Ok(())
    }

    /// A depositor's own withdrawal is refused before the escape time and allowed from it on.
    pub fn validate_escape(&self, now: i64) -> ProgramResult {
        if now < self.escape_time {
            return Err(EscrowProgramError::EscapeTimeNotReached.into());
        }
        Ok(())
    }
}

/// Checks the deposit deadline of an escrow whose settlement slot may be empty.
///
/// An escrow without settlement terms behaves exactly as upstream, so an empty slot passes.
pub fn check_deposit_window(settlement: &Option<Vec<u8>>, now: i64) -> ProgramResult {
    match settlement {
        Some(bytes) => SettlementData::from_bytes(bytes)?.validate_deposit(now),
        None => Ok(()),
    }
}

/// Checks the escape time of an escrow whose settlement slot may be empty, for the
/// depositor's own withdrawal.
pub fn check_escape_window(settlement: &Option<Vec<u8>>, now: i64) -> ProgramResult {
    match settlement {
        Some(bytes) => SettlementData::from_bytes(bytes)?.validate_escape(now),
        None => Ok(()),
    }
}

impl ExtensionData for SettlementData {
    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(Self::LEN);
        data.extend_from_slice(self.beneficiary.as_ref());
        data.extend_from_slice(&self.deposit_deadline.to_le_bytes());
        data.extend_from_slice(&self.escape_time.to_le_bytes());
        data
    }

    fn from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        require_len!(data, Self::LEN);

        Ok(Self {
            beneficiary: Address::new_from_array(data[0..32].try_into().unwrap()),
            deposit_deadline: i64::from_le_bytes(data[32..40].try_into().unwrap()),
            escape_time: i64::from_le_bytes(data[40..48].try_into().unwrap()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(deposit_deadline: i64, escape_time: i64) -> SettlementData {
        SettlementData::new(Address::new_from_array([7u8; 32]), deposit_deadline, escape_time)
    }

    #[test]
    fn test_settlement_roundtrip() {
        let data = terms(1_000, 1_000 + MIN_ESCAPE_GAP_SECONDS);
        let parsed = SettlementData::from_bytes(&data.to_bytes()).unwrap();
        assert_eq!(parsed, data);
    }

    #[test]
    fn test_settlement_from_bytes_rejects_short_data() {
        assert!(SettlementData::from_bytes(&[0u8; 47]).is_err());
    }

    #[test]
    fn test_terms_need_seven_days_between_deadline_and_escape() {
        assert!(terms(1_000, 1_000 + MIN_ESCAPE_GAP_SECONDS).validate_terms().is_ok());
        assert_eq!(
            terms(1_000, 1_000 + MIN_ESCAPE_GAP_SECONDS - 1).validate_terms(),
            Err(EscrowProgramError::InvalidSettlementTerms.into())
        );
    }

    #[test]
    fn test_terms_refuse_an_overflowing_deadline() {
        assert_eq!(terms(i64::MAX, i64::MAX).validate_terms(), Err(ProgramError::ArithmeticOverflow));
    }

    #[test]
    fn test_empty_slot_behaves_as_upstream() {
        assert!(check_deposit_window(&None, i64::MAX).is_ok());
        assert!(check_escape_window(&None, 0).is_ok());
    }

    #[test]
    fn test_windows_read_the_stored_terms() {
        let stored = Some(terms(1_000, 1_000 + MIN_ESCAPE_GAP_SECONDS).to_bytes());
        assert!(check_deposit_window(&stored, 1_000).is_ok());
        assert!(check_deposit_window(&stored, 1_001).is_err());
        assert!(check_escape_window(&stored, 1_000).is_err());
        assert!(check_escape_window(&stored, 1_000 + MIN_ESCAPE_GAP_SECONDS).is_ok());
    }

    #[test]
    fn test_deposit_is_allowed_up_to_the_deadline_and_refused_after() {
        let data = terms(1_000, 1_000 + MIN_ESCAPE_GAP_SECONDS);
        assert!(data.validate_deposit(999).is_ok());
        assert!(data.validate_deposit(1_000).is_ok());
        assert_eq!(data.validate_deposit(1_001), Err(EscrowProgramError::DepositDeadlinePassed.into()));
    }

    #[test]
    fn test_escape_is_refused_before_the_escape_time_and_allowed_from_it() {
        let data = terms(1_000, 1_000 + MIN_ESCAPE_GAP_SECONDS);
        assert_eq!(
            data.validate_escape(1_000 + MIN_ESCAPE_GAP_SECONDS - 1),
            Err(EscrowProgramError::EscapeTimeNotReached.into())
        );
        assert!(data.validate_escape(1_000 + MIN_ESCAPE_GAP_SECONDS).is_ok());
    }
}
