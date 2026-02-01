//! Account model for Mbongo Chain.
//!
//! Provides the `Account` struct and state transition logic for tracking
//! user balances, nonces, and validator state.

use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::Address;

/// Errors that can occur during account state transitions.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AccountError {
    /// Insufficient balance for the requested transfer.
    #[error("insufficient balance: have {have}, need {need}")]
    InsufficientBalance {
        /// Current balance.
        have: u128,
        /// Required balance.
        need: u128,
    },

    /// Invalid nonce provided (expected different value).
    #[error("invalid nonce: expected {expected}, got {got}")]
    InvalidNonce {
        /// Expected nonce value.
        expected: u64,
        /// Provided nonce value.
        got: u64,
    },

    /// Arithmetic overflow during balance operation.
    #[error("balance overflow")]
    BalanceOverflow,

    /// Insufficient stake for the requested operation.
    #[error("insufficient stake: have {have}, need {need}")]
    InsufficientStake {
        /// Current stake.
        have: u128,
        /// Required stake.
        need: u128,
    },
}

/// Validator-specific data for staking accounts.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Encode, Decode)]
pub struct ValidatorData {
    /// Amount of MBO staked by this validator.
    pub stake: u128,
    /// Whether the validator is currently active in consensus.
    pub is_active: bool,
    /// Compute reputation score (0-1000, used for PoX weighting).
    pub compute_score: u32,
}

impl ValidatorData {
    /// Creates new validator data with zero stake.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            stake: 0,
            is_active: false,
            compute_score: 0,
        }
    }
}

/// Account state for a single address.
///
/// Tracks balance, nonce for replay protection, and optional validator data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct Account {
    /// Account address (ed25519 public key).
    pub address: Address,
    /// Available balance in MBO (not staked).
    pub balance: u128,
    /// Nonce for transaction replay protection.
    pub nonce: u64,
    /// Validator data if this account is a validator.
    pub validator_data: Option<ValidatorData>,
}

impl Account {
    /// Creates a new account with zero balance and nonce.
    #[must_use]
    pub const fn new(address: Address) -> Self {
        Self {
            address,
            balance: 0,
            nonce: 0,
            validator_data: None,
        }
    }

    /// Creates a new account with an initial balance.
    #[must_use]
    pub const fn with_balance(address: Address, balance: u128) -> Self {
        Self {
            address,
            balance,
            nonce: 0,
            validator_data: None,
        }
    }

    /// Returns the total balance including staked amount.
    #[must_use]
    pub fn total_balance(&self) -> u128 {
        let staked = self
            .validator_data
            .as_ref()
            .map_or(0, |v| v.stake);
        self.balance.saturating_add(staked)
    }

    /// Validates and increments the nonce for a new transaction.
    ///
    /// # Errors
    ///
    /// Returns `AccountError::InvalidNonce` if the provided nonce doesn't match.
    pub fn validate_and_increment_nonce(&mut self, tx_nonce: u64) -> Result<(), AccountError> {
        if tx_nonce != self.nonce {
            return Err(AccountError::InvalidNonce {
                expected: self.nonce,
                got: tx_nonce,
            });
        }
        self.nonce = self.nonce.wrapping_add(1);
        Ok(())
    }

    /// Deducts amount from balance (used for transfers/fees).
    ///
    /// # Errors
    ///
    /// Returns `AccountError::InsufficientBalance` if balance is too low.
    pub fn debit(&mut self, amount: u128) -> Result<(), AccountError> {
        if self.balance < amount {
            return Err(AccountError::InsufficientBalance {
                have: self.balance,
                need: amount,
            });
        }
        self.balance -= amount;
        Ok(())
    }

    /// Adds amount to balance (used for receiving transfers/rewards).
    ///
    /// # Errors
    ///
    /// Returns `AccountError::BalanceOverflow` if addition would overflow.
    pub fn credit(&mut self, amount: u128) -> Result<(), AccountError> {
        self.balance = self
            .balance
            .checked_add(amount)
            .ok_or(AccountError::BalanceOverflow)?;
        Ok(())
    }

    /// Performs a transfer from this account to another.
    ///
    /// Validates nonce, debits sender, and credits receiver.
    ///
    /// # Errors
    ///
    /// Returns error if nonce is invalid, balance insufficient, or overflow occurs.
    pub fn transfer(
        &mut self,
        receiver: &mut Account,
        amount: u128,
        tx_nonce: u64,
    ) -> Result<(), AccountError> {
        self.validate_and_increment_nonce(tx_nonce)?;
        self.debit(amount)?;
        receiver.credit(amount)?;
        Ok(())
    }

    /// Stakes tokens from available balance.
    ///
    /// Creates validator data if not already present.
    ///
    /// # Errors
    ///
    /// Returns error if insufficient balance.
    pub fn stake(&mut self, amount: u128) -> Result<(), AccountError> {
        self.debit(amount)?;
        let validator = self.validator_data.get_or_insert_with(ValidatorData::new);
        validator.stake = validator
            .stake
            .checked_add(amount)
            .ok_or(AccountError::BalanceOverflow)?;
        Ok(())
    }

    /// Unstakes tokens back to available balance.
    ///
    /// # Errors
    ///
    /// Returns error if insufficient stake or overflow.
    pub fn unstake(&mut self, amount: u128) -> Result<(), AccountError> {
        let validator = self.validator_data.as_mut().ok_or(AccountError::InsufficientStake {
            have: 0,
            need: amount,
        })?;

        if validator.stake < amount {
            return Err(AccountError::InsufficientStake {
                have: validator.stake,
                need: amount,
            });
        }

        validator.stake -= amount;
        self.credit(amount)?;

        // Deactivate if no stake remaining
        if validator.stake == 0 {
            validator.is_active = false;
        }

        Ok(())
    }

    /// Activates validator status (requires non-zero stake).
    pub fn activate_validator(&mut self) -> bool {
        if let Some(ref mut v) = self.validator_data {
            if v.stake > 0 {
                v.is_active = true;
                return true;
            }
        }
        false
    }

    /// Deactivates validator status.
    pub fn deactivate_validator(&mut self) {
        if let Some(ref mut v) = self.validator_data {
            v.is_active = false;
        }
    }

    /// Returns whether this account is an active validator.
    #[must_use]
    pub fn is_active_validator(&self) -> bool {
        self.validator_data
            .as_ref()
            .map_or(false, |v| v.is_active && v.stake > 0)
    }
}

impl Default for Account {
    fn default() -> Self {
        Self::new(Address::zero())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address(seed: u8) -> Address {
        Address([seed; 32])
    }

    #[test]
    fn new_account_has_zero_balance_and_nonce() {
        let acc = Account::new(test_address(1));
        assert_eq!(acc.balance, 0);
        assert_eq!(acc.nonce, 0);
        assert!(acc.validator_data.is_none());
    }

    #[test]
    fn with_balance_sets_initial_balance() {
        let acc = Account::with_balance(test_address(1), 1000);
        assert_eq!(acc.balance, 1000);
        assert_eq!(acc.nonce, 0);
    }

    #[test]
    fn credit_increases_balance() {
        let mut acc = Account::new(test_address(1));
        acc.credit(100).unwrap();
        assert_eq!(acc.balance, 100);
        acc.credit(50).unwrap();
        assert_eq!(acc.balance, 150);
    }

    #[test]
    fn credit_overflow_fails() {
        let mut acc = Account::with_balance(test_address(1), u128::MAX);
        let result = acc.credit(1);
        assert!(matches!(result, Err(AccountError::BalanceOverflow)));
    }

    #[test]
    fn debit_decreases_balance() {
        let mut acc = Account::with_balance(test_address(1), 100);
        acc.debit(30).unwrap();
        assert_eq!(acc.balance, 70);
    }

    #[test]
    fn debit_insufficient_balance_fails() {
        let mut acc = Account::with_balance(test_address(1), 50);
        let result = acc.debit(100);
        assert!(matches!(
            result,
            Err(AccountError::InsufficientBalance { have: 50, need: 100 })
        ));
    }

    #[test]
    fn nonce_validation_and_increment() {
        let mut acc = Account::new(test_address(1));
        assert_eq!(acc.nonce, 0);

        // Valid nonce increments
        acc.validate_and_increment_nonce(0).unwrap();
        assert_eq!(acc.nonce, 1);

        acc.validate_and_increment_nonce(1).unwrap();
        assert_eq!(acc.nonce, 2);

        // Invalid nonce fails
        let result = acc.validate_and_increment_nonce(0);
        assert!(matches!(
            result,
            Err(AccountError::InvalidNonce { expected: 2, got: 0 })
        ));
    }

    #[test]
    fn transfer_moves_balance_and_increments_nonce() {
        let mut sender = Account::with_balance(test_address(1), 1000);
        let mut receiver = Account::new(test_address(2));

        sender.transfer(&mut receiver, 300, 0).unwrap();

        assert_eq!(sender.balance, 700);
        assert_eq!(sender.nonce, 1);
        assert_eq!(receiver.balance, 300);
        assert_eq!(receiver.nonce, 0); // Receiver nonce unchanged
    }

    #[test]
    fn transfer_insufficient_balance_fails() {
        let mut sender = Account::with_balance(test_address(1), 100);
        let mut receiver = Account::new(test_address(2));

        let result = sender.transfer(&mut receiver, 200, 0);
        assert!(matches!(result, Err(AccountError::InsufficientBalance { .. })));
        // State should not change on failure
        assert_eq!(sender.balance, 100);
        assert_eq!(sender.nonce, 1); // Nonce was validated before debit
    }

    #[test]
    fn transfer_invalid_nonce_fails() {
        let mut sender = Account::with_balance(test_address(1), 1000);
        let mut receiver = Account::new(test_address(2));

        let result = sender.transfer(&mut receiver, 100, 5); // Wrong nonce
        assert!(matches!(result, Err(AccountError::InvalidNonce { .. })));
        assert_eq!(sender.balance, 1000);
        assert_eq!(sender.nonce, 0);
    }

    #[test]
    fn stake_moves_balance_to_validator_data() {
        let mut acc = Account::with_balance(test_address(1), 1000);

        acc.stake(400).unwrap();

        assert_eq!(acc.balance, 600);
        assert!(acc.validator_data.is_some());
        assert_eq!(acc.validator_data.as_ref().unwrap().stake, 400);
        assert_eq!(acc.total_balance(), 1000); // Total unchanged
    }

    #[test]
    fn stake_accumulates() {
        let mut acc = Account::with_balance(test_address(1), 1000);

        acc.stake(200).unwrap();
        acc.stake(300).unwrap();

        assert_eq!(acc.balance, 500);
        assert_eq!(acc.validator_data.as_ref().unwrap().stake, 500);
    }

    #[test]
    fn stake_insufficient_balance_fails() {
        let mut acc = Account::with_balance(test_address(1), 100);

        let result = acc.stake(200);
        assert!(matches!(result, Err(AccountError::InsufficientBalance { .. })));
    }

    #[test]
    fn unstake_returns_balance() {
        let mut acc = Account::with_balance(test_address(1), 1000);
        acc.stake(500).unwrap();

        acc.unstake(200).unwrap();

        assert_eq!(acc.balance, 700);
        assert_eq!(acc.validator_data.as_ref().unwrap().stake, 300);
    }

    #[test]
    fn unstake_insufficient_stake_fails() {
        let mut acc = Account::with_balance(test_address(1), 1000);
        acc.stake(100).unwrap();

        let result = acc.unstake(200);
        assert!(matches!(result, Err(AccountError::InsufficientStake { .. })));
    }

    #[test]
    fn unstake_no_validator_data_fails() {
        let mut acc = Account::with_balance(test_address(1), 1000);

        let result = acc.unstake(100);
        assert!(matches!(result, Err(AccountError::InsufficientStake { have: 0, .. })));
    }

    #[test]
    fn validator_activation() {
        let mut acc = Account::with_balance(test_address(1), 1000);

        // Cannot activate without stake
        assert!(!acc.activate_validator());
        assert!(!acc.is_active_validator());

        // Stake and activate
        acc.stake(500).unwrap();
        assert!(acc.activate_validator());
        assert!(acc.is_active_validator());

        // Deactivate
        acc.deactivate_validator();
        assert!(!acc.is_active_validator());
    }

    #[test]
    fn unstake_all_deactivates_validator() {
        let mut acc = Account::with_balance(test_address(1), 1000);
        acc.stake(500).unwrap();
        acc.activate_validator();
        assert!(acc.is_active_validator());

        acc.unstake(500).unwrap();

        assert!(!acc.is_active_validator());
        assert_eq!(acc.validator_data.as_ref().unwrap().stake, 0);
    }

    #[test]
    fn serde_roundtrip() {
        let mut acc = Account::with_balance(test_address(42), 12345);
        acc.nonce = 7;
        acc.stake(1000).unwrap();
        acc.activate_validator();

        let json = serde_json::to_string(&acc).unwrap();
        let recovered: Account = serde_json::from_str(&json).unwrap();

        assert_eq!(recovered, acc);
    }

    #[test]
    fn scale_roundtrip() {
        let mut acc = Account::with_balance(test_address(99), 999_999);
        acc.stake(100_000).unwrap();

        let encoded = acc.encode();
        let decoded = Account::decode(&mut &encoded[..]).unwrap();

        assert_eq!(decoded, acc);
    }

    #[test]
    fn total_balance_includes_stake() {
        let mut acc = Account::with_balance(test_address(1), 1000);
        assert_eq!(acc.total_balance(), 1000);

        acc.stake(300).unwrap();
        assert_eq!(acc.balance, 700);
        assert_eq!(acc.total_balance(), 1000);

        acc.credit(500).unwrap();
        assert_eq!(acc.total_balance(), 1500);
    }
}
