//! Minimal account model for Mbongo Chain (Phase 1).

use parity_scale_codec::{Decode, Encode};

use crate::Address;

/// Error type for account operations.
#[derive(Debug, PartialEq, Eq)]
pub enum AccountError {
    /// Balance is too low to cover the requested debit.
    InsufficientBalance,
    /// Credit would overflow the maximum `u128` balance.
    BalanceOverflow,
    /// Provided nonce does not match the account's current nonce.
    InvalidNonce,
}

impl std::fmt::Display for AccountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientBalance => write!(f, "insufficient balance"),
            Self::BalanceOverflow => write!(f, "balance overflow"),
            Self::InvalidNonce => write!(f, "invalid nonce"),
        }
    }
}

impl std::error::Error for AccountError {}

/// On-chain account with an address, balance, and nonce.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Account {
    /// The account's unique address (ed25519 public key).
    pub address: Address,
    /// Current balance in the smallest unit.
    pub balance: u128,
    /// Monotonically increasing nonce for replay protection.
    pub nonce: u64,
}

impl Account {
    /// Creates a new account with zero balance and zero nonce.
    #[must_use]
    pub fn new(address: Address) -> Self {
        Self {
            address,
            balance: 0,
            nonce: 0,
        }
    }

    /// Credits `amount` to this account's balance.
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::BalanceOverflow`] if the addition overflows `u128`.
    pub fn credit(&mut self, amount: u128) -> Result<(), AccountError> {
        self.balance = self.balance.checked_add(amount).ok_or(AccountError::BalanceOverflow)?;
        Ok(())
    }

    /// Debits `amount` from this account's balance.
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::InsufficientBalance`] if balance is less than `amount`.
    pub fn debit(&mut self, amount: u128) -> Result<(), AccountError> {
        if self.balance < amount {
            return Err(AccountError::InsufficientBalance);
        }
        self.balance -= amount;
        Ok(())
    }

    /// Validates the provided nonce matches the current nonce and increments it.
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::InvalidNonce`] if `provided` does not equal `self.nonce`.
    pub fn validate_and_increment_nonce(&mut self, provided: u64) -> Result<(), AccountError> {
        if provided != self.nonce {
            return Err(AccountError::InvalidNonce);
        }
        self.nonce += 1;
        Ok(())
    }

    /// Atomically transfers `amount` from one account to another.
    ///
    /// If the credit to the receiver fails, the debit from the sender is reverted
    /// so that neither account is left in an inconsistent state.
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::InsufficientBalance`] if the sender cannot cover `amount`.
    /// Returns [`AccountError::BalanceOverflow`] if the receiver's balance would overflow.
    pub fn transfer(
        from: &mut Account,
        to: &mut Account,
        amount: u128,
    ) -> Result<(), AccountError> {
        from.debit(amount)?;
        if let Err(e) = to.credit(amount) {
            from.balance += amount;
            return Err(e);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address(byte: u8) -> Address {
        Address([byte; 32])
    }

    #[test]
    fn account_creation_defaults() {
        let addr = test_address(1);
        let account = Account::new(addr);
        assert_eq!(account.address, addr);
        assert_eq!(account.balance, 0);
        assert_eq!(account.nonce, 0);
    }

    #[test]
    fn credit_increases_balance() {
        let mut account = Account::new(test_address(1));
        account.credit(500).unwrap();
        assert_eq!(account.balance, 500);
        account.credit(300).unwrap();
        assert_eq!(account.balance, 800);
    }

    #[test]
    fn credit_overflow_fails() {
        let mut account = Account::new(test_address(1));
        account.balance = u128::MAX;
        let result = account.credit(1);
        assert_eq!(result, Err(AccountError::BalanceOverflow));
        assert_eq!(account.balance, u128::MAX);
    }

    #[test]
    fn debit_reduces_balance() {
        let mut account = Account::new(test_address(1));
        account.balance = 1000;
        account.debit(400).unwrap();
        assert_eq!(account.balance, 600);
    }

    #[test]
    fn debit_insufficient_balance_fails() {
        let mut account = Account::new(test_address(1));
        account.balance = 100;
        let result = account.debit(200);
        assert_eq!(result, Err(AccountError::InsufficientBalance));
        assert_eq!(account.balance, 100);
    }

    #[test]
    fn nonce_validation_success() {
        let mut account = Account::new(test_address(1));
        assert_eq!(account.nonce, 0);
        account.validate_and_increment_nonce(0).unwrap();
        assert_eq!(account.nonce, 1);
        account.validate_and_increment_nonce(1).unwrap();
        assert_eq!(account.nonce, 2);
    }

    #[test]
    fn nonce_validation_failure() {
        let mut account = Account::new(test_address(1));
        let result = account.validate_and_increment_nonce(5);
        assert_eq!(result, Err(AccountError::InvalidNonce));
        assert_eq!(account.nonce, 0);
    }

    #[test]
    fn transfer_success() {
        let mut from = Account::new(test_address(1));
        let mut to = Account::new(test_address(2));
        from.balance = 1000;
        Account::transfer(&mut from, &mut to, 400).unwrap();
        assert_eq!(from.balance, 600);
        assert_eq!(to.balance, 400);
    }

    #[test]
    fn transfer_insufficient_balance_fails() {
        let mut from = Account::new(test_address(1));
        let mut to = Account::new(test_address(2));
        from.balance = 100;
        let result = Account::transfer(&mut from, &mut to, 200);
        assert_eq!(result, Err(AccountError::InsufficientBalance));
        assert_eq!(from.balance, 100);
        assert_eq!(to.balance, 0);
    }

    #[test]
    fn transfer_atomic_on_credit_failure() {
        let mut from = Account::new(test_address(1));
        let mut to = Account::new(test_address(2));
        from.balance = 500;
        to.balance = u128::MAX;
        let result = Account::transfer(&mut from, &mut to, 100);
        assert_eq!(result, Err(AccountError::BalanceOverflow));
        assert_eq!(from.balance, 500);
        assert_eq!(to.balance, u128::MAX);
    }
}
