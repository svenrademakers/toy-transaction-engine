use crate::data_types::{
    Account, DepositOrWithdraw, Price, TransactionError, TransactionFlags, TransactionType,
};
use std::collections::{hash_map::Entry, HashMap};
use tracing::debug;

#[derive(Debug)]
pub struct TransactionContext {
    transactions: HashMap<u32, (Price, TransactionFlags)>,
    accounts: HashMap<u16, Account>,
}

impl TransactionContext {
    pub fn new() -> Self {
        TransactionContext {
            transactions: HashMap::with_capacity(1024 * 1024),
            accounts: HashMap::with_capacity(1024),
        }
    }

    pub fn into_iter_accounts(self) -> impl Iterator<Item = (u16, Account)> {
        self.accounts.into_iter()
    }

    pub fn handle_transaction(
        &mut self,
        client_id: u16,
        tx: u32,
        amount: Price,
        deposit_withdraw: DepositOrWithdraw,
    ) {
        let Entry::Vacant(entry) = self.transactions.entry(tx) else {
            debug!(error = ?TransactionError::Duplicate, tx);
            return;
        };

        entry.insert((amount, TransactionFlags::None));

        let account = self.accounts.entry(client_id).or_default();
        let result = if deposit_withdraw == DepositOrWithdraw::Deposit {
            account.deposit(amount)
        } else {
            account.withdraw(amount)
        };

        if let Err(e) = result {
            // there is a nightly .insert_entry which we could use to remove
            // again the emplaced item. To keep stay in rust stable, lookup and
            // remove instead.
            self.transactions.remove(&tx);
            debug!(error = ?e, client_id, tx, %amount);
        }
    }

    pub fn handle_dispute(&mut self, client_id: u16, tx: u32) {
        let Entry::Occupied(mut entry) = self.transactions.entry(tx) else {
            debug!(error = ?TransactionError::NotFound, typ= ?TransactionType::Dispute, tx);
            return;
        };

        entry.get_mut().1 = TransactionFlags::Disputed;

        let Entry::Occupied(mut account) = self.accounts.entry(client_id) else {
            debug!(error = ?TransactionError::InvalidDispute, client_id);
            return;
        };

        account.get_mut().held.try_add(entry.get().0);
    }

    pub fn handle_resolve(&mut self, client_id: u16, tx: u32) {
        let Entry::Occupied(mut entry) = self.transactions.entry(tx) else {
            debug!(error = ?TransactionError::NotFound, typ= ?TransactionType::Resolve, tx);
            return;
        };

        if TransactionFlags::Disputed != entry.get_mut().1 {
            debug!(error = ?TransactionError::InvalidDispute, client_id);
            return;
        }

        let Entry::Occupied(mut account) = self.accounts.entry(client_id) else {
            debug!(error = ?TransactionError::InvalidDispute, client_id);
            return;
        };

        account.get_mut().held.try_sub(entry.get().0);
        entry.get_mut().1 = TransactionFlags::Resolved;
    }

    pub fn handle_chargeback(&mut self, client_id: u16, tx: u32) {
        let Entry::Occupied(mut entry) = self.transactions.entry(tx) else {
            debug!(error = ?TransactionError::NotFound, typ= ?TransactionType::Resolve, tx);
            return;
        };

        if TransactionFlags::Disputed != entry.get_mut().1 {
            debug!(error = ?TransactionError::InvalidDispute, client_id);
            return;
        }

        let Entry::Occupied(mut account) = self.accounts.entry(client_id) else {
            debug!(error = ?TransactionError::InvalidDispute, client_id);
            return;
        };

        let mut_acc = account.get_mut();
        mut_acc.held.try_sub(entry.get().0);
        mut_acc.total.try_sub(entry.get().0);
        mut_acc.locked = true;

        entry.get_mut().1 = TransactionFlags::Chargeback;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_types::{DepositOrWithdraw, Price, TransactionFlags, PRICE_SCALAR};

    fn price(value: i64) -> Price {
        Price(value * PRICE_SCALAR)
    }

    #[test]
    fn test_deposit() {
        let mut context = TransactionContext::new();
        context.handle_transaction(1, 1, price(100), DepositOrWithdraw::Deposit);

        assert_eq!(context.transactions.len(), 1);
        assert_eq!(
            context.transactions.get(&1),
            Some(&(price(100), TransactionFlags::None))
        );

        let account = context.accounts.get(&1).unwrap();
        assert_eq!(account.total, price(100));
        assert_eq!(account.held, price(0));
        assert_eq!(account.available(), price(100));
    }

    #[test]
    fn test_withdraw() {
        let mut context = TransactionContext::new();
        context.handle_transaction(1, 1, price(100), DepositOrWithdraw::Deposit);
        context.handle_transaction(1, 2, price(50), DepositOrWithdraw::Withdraw);

        assert_eq!(context.transactions.len(), 2);

        let account = context.accounts.get(&1).unwrap();
        assert_eq!(account.total, price(50));
        assert_eq!(account.held, price(0));
        assert_eq!(account.available(), price(50));
    }

    #[test]
    fn test_dispute() {
        let mut context = TransactionContext::new();
        context.handle_transaction(1, 1, price(100), DepositOrWithdraw::Deposit);
        context.handle_dispute(1, 1);

        assert_eq!(
            context.transactions.get(&1),
            Some(&(price(100), TransactionFlags::Disputed))
        );

        let account = context.accounts.get(&1).unwrap();
        assert_eq!(account.total, price(100));
        assert_eq!(account.held, price(100));
        assert_eq!(account.available(), price(0));
    }

    #[test]
    fn test_resolve() {
        let mut context = TransactionContext::new();
        context.handle_transaction(1, 1, price(100), DepositOrWithdraw::Deposit);
        context.handle_dispute(1, 1);
        context.handle_resolve(1, 1);

        assert_eq!(
            context.transactions.get(&1),
            Some(&(price(100), TransactionFlags::Resolved))
        );

        let account = context.accounts.get(&1).unwrap();
        assert_eq!(account.total, price(100));
        assert_eq!(account.held, price(0));
        assert_eq!(account.available(), price(100));
    }

    #[test]
    fn test_chargeback() {
        let mut context = TransactionContext::new();
        context.handle_transaction(1, 1, price(100), DepositOrWithdraw::Deposit);
        context.handle_dispute(1, 1);
        context.handle_chargeback(1, 1);

        assert_eq!(
            context.transactions.get(&1),
            Some(&(price(100), TransactionFlags::Chargeback))
        );

        let account = context.accounts.get(&1).unwrap();
        assert_eq!(account.total, price(0));
        assert_eq!(account.held, price(0));
        assert_eq!(account.available(), price(0));
        assert!(account.locked);
    }

    #[test]
    fn test_duplicate_transaction() {
        let mut context = TransactionContext::new();
        context.handle_transaction(1, 1, price(100), DepositOrWithdraw::Deposit);
        context.handle_transaction(1, 1, price(200), DepositOrWithdraw::Deposit);

        assert_eq!(context.transactions.len(), 1);

        let account = context.accounts.get(&1).unwrap();
        assert_eq!(account.total, price(100));
        assert_eq!(account.available(), price(100));
    }

    #[test]
    fn test_invalid_dispute() {
        let mut context = TransactionContext::new();
        context.handle_dispute(1, 1);

        assert_eq!(context.transactions.len(), 0);
        assert_eq!(context.accounts.len(), 0);
    }

    #[test]
    fn test_invalid_resolve() {
        let mut context = TransactionContext::new();
        context.handle_resolve(1, 1);

        assert_eq!(context.transactions.len(), 0);
        assert_eq!(context.accounts.len(), 0);
    }

    #[test]
    fn test_invalid_chargeback() {
        let mut context = TransactionContext::new();
        context.handle_chargeback(1, 1);

        assert_eq!(context.transactions.len(), 0);
        assert_eq!(context.accounts.len(), 0);
    }
}
