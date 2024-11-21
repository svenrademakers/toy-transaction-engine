use crate::data_types::{
    Account, Price, TransactionError, TransactionEvent, TransactionFlags, TransactionType,
};
use std::collections::{hash_map::Entry, HashMap};
use tracing::debug;

#[derive(Debug)]
pub struct TransactionContext {
    transactions: HashMap<u32, (Price, TransactionFlags, u16)>,
    accounts: HashMap<u16, Account>,
}

impl TransactionContext {
    pub fn new() -> Self {
        TransactionContext {
            // arbitrary chosen capacity values
            transactions: HashMap::with_capacity(1024 * 1024),
            accounts: HashMap::with_capacity(1024),
        }
    }

    pub fn into_iter_accounts(self) -> impl Iterator<Item = (u16, Account)> {
        self.accounts.into_iter()
    }

    pub fn handle_transaction(
        &mut self,
        event: &TransactionEvent,
        action: impl Fn(&mut Account, Price) -> Result<(), TransactionError>,
        store_transaction: bool,
    ) {
        let Entry::Vacant(entry) = self.transactions.entry(event.tx) else {
            debug!(error = ?TransactionError::Duplicate, event.tx);
            return;
        };

        if store_transaction {
            entry.insert((event.amount, TransactionFlags::None, event.client_id));
        }

        let account = self.accounts.entry(event.client_id).or_default();
        if let Err(e) = action(account, event.amount) {
            // there is a nightly .insert_entry which we could use to remove
            // again the emplaced item. To keep stay in rust stable, lookup and
            // remove instead.
            self.transactions.remove(&event.tx);
            debug!(error = ?e, event.client_id, event.tx, %event.amount);
        }
    }

    pub fn handle_dispute(
        &mut self,
        event: &TransactionEvent,
        expected_desired: (TransactionFlags, TransactionFlags),
        dispute_action: impl Fn(&mut Account, Price),
    ) {
        let Entry::Occupied(mut entry) = self.transactions.entry(event.tx) else {
            debug!(error = ?TransactionError::NotFound, typ= ?TransactionType::Dispute, event.tx);
            return;
        };

        let mut_entry = entry.get_mut();
        if mut_entry.2 != event.client_id {
            debug!(error = ?TransactionError::ClientMismatch, event.client_id, ?mut_entry);
            return;
        }

        if mut_entry.1 != expected_desired.0 {
            debug!(error = ?TransactionError::InvalidDispute, event.client_id, ?mut_entry);
            return;
        }

        let Entry::Occupied(mut account) = self.accounts.entry(event.client_id) else {
            debug!(error = ?TransactionError::InvalidDispute, event.client_id);
            return;
        };

        entry.get_mut().1 = expected_desired.1;
        dispute_action(account.get_mut(), entry.get().0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_types::{Account, TransactionEvent, TransactionType};

    fn create_event(
        tx_type: TransactionType,
        client_id: u16,
        tx: u32,
        amount: f64,
    ) -> TransactionEvent {
        TransactionEvent {
            ty: tx_type,
            client_id,
            tx,
            amount: amount.try_into().unwrap_or_default(),
        }
    }

    #[test]
    fn test_deposit() {
        let mut context = TransactionContext::new();

        let deposit_event = create_event(TransactionType::Deposit, 1, 1, 10.0);
        context.handle_transaction(&deposit_event, Account::deposit, true);

        let account = context.accounts.get(&1).expect("Account not found");
        assert_eq!(account.available(), 10.0.try_into().unwrap());
        assert_eq!(account.total, 10.0.try_into().unwrap());
        assert_eq!(account.held, 0.0.try_into().unwrap());
    }

    #[test]
    fn test_withdrawal() {
        let mut context = TransactionContext::new();

        let deposit_event = create_event(TransactionType::Deposit, 1, 1, 10.0);
        context.handle_transaction(&deposit_event, Account::deposit, true);

        let withdrawal_event = create_event(TransactionType::Withdrawal, 1, 2, 5.0);
        context.handle_transaction(&withdrawal_event, Account::withdraw, false);

        // Check the account balance
        let account = context.accounts.get(&1).expect("Account not found");
        assert_eq!(account.available(), 5.0.try_into().unwrap());
        assert_eq!(account.total, 5.0.try_into().unwrap());
        assert_eq!(account.held, 0.0.try_into().unwrap());
    }

    #[test]
    fn test_dispute() {
        let mut context = TransactionContext::new();

        let deposit_event = create_event(TransactionType::Deposit, 1, 1, 10.0);
        context.handle_transaction(&deposit_event, Account::deposit, true);

        let dispute_event = create_event(TransactionType::Dispute, 1, 1, 0.0);
        context.handle_dispute(
            &dispute_event,
            (TransactionFlags::None, TransactionFlags::Disputed),
            Account::dispute,
        );

        // Check the account balance
        let account = context.accounts.get(&1).expect("Account not found");
        assert_eq!(account.available(), 0.0.try_into().unwrap());
        assert_eq!(account.held, 10.0.try_into().unwrap());
        assert_eq!(account.total, 10.0.try_into().unwrap());
    }

    #[test]
    fn test_resolve() {
        let mut context = TransactionContext::new();

        let deposit_event = create_event(TransactionType::Deposit, 1, 1, 10.0);
        context.handle_transaction(&deposit_event, Account::deposit, true);

        let dispute_event = create_event(TransactionType::Dispute, 1, 1, 0.0);
        context.handle_dispute(
            &dispute_event,
            (TransactionFlags::None, TransactionFlags::Disputed),
            Account::dispute,
        );

        let resolve_event = create_event(TransactionType::Resolve, 1, 1, 0.0);
        context.handle_dispute(
            &resolve_event,
            (TransactionFlags::Disputed, TransactionFlags::Resolved),
            Account::resolve,
        );

        // Check the account balance
        let account = context.accounts.get(&1).expect("Account not found");
        assert_eq!(account.available(), 10.0.try_into().unwrap());
        assert_eq!(account.held, 0.0.try_into().unwrap());
        assert_eq!(account.total, 10.0.try_into().unwrap());
    }

    #[test]
    fn test_chargeback() {
        let mut context = TransactionContext::new();

        let deposit_event = create_event(TransactionType::Deposit, 1, 1, 10.0);
        context.handle_transaction(&deposit_event, Account::deposit, true);

        let dispute_event = create_event(TransactionType::Dispute, 1, 1, 0.0);
        context.handle_dispute(
            &dispute_event,
            (TransactionFlags::None, TransactionFlags::Disputed),
            Account::dispute,
        );

        let chargeback_event = create_event(TransactionType::Chargeback, 1, 1, 0.0);
        context.handle_dispute(
            &chargeback_event,
            (TransactionFlags::Disputed, TransactionFlags::Chargeback),
            Account::chargeback,
        );

        let account = context.accounts.get(&1).expect("Account not found");
        assert_eq!(account.available(), 0.0.try_into().unwrap());
        assert_eq!(account.held, 0.0.try_into().unwrap());
        assert_eq!(account.total, 0.0.try_into().unwrap());
        assert!(account.locked);
    }
}
