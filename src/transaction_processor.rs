use crate::{
    data_types::{Account, DepositOrWithdraw, TransactionEvent, TransactionType},
    transaction_context::TransactionContext,
};
use crossbeam::channel::Receiver;

#[derive(Debug)]
pub struct TransactionProcessor<'a> {
    context: &'a mut TransactionContext,
    receiver: Receiver<TransactionEvent>,
}

impl<'a> TransactionProcessor<'a> {
    /// Processes Events until the sources are exhausted.
    /// Returns a Iterator over the processed accounts.
    pub fn exhaust_sources(
        transaction_receiver: Receiver<TransactionEvent>,
    ) -> impl Iterator<Item = (u16, Account)> {
        let mut context = TransactionContext::default();
        TransactionProcessor::new(&mut context, transaction_receiver).run();

        context.into_iter_accounts()
    }

    fn new(context: &'a mut TransactionContext, receiver: Receiver<TransactionEvent>) -> Self {
        TransactionProcessor { context, receiver }
    }

    fn run(mut self) {
        while let Ok(mut event) = self.receiver.recv() {
            // precautionary call to make sure the interface is honored
            event.amount.make_absolute();

            self.update_accounts(event);
        }
    }

    /// This function executes the core logic of transaction handling.
    fn update_accounts(&mut self, event: TransactionEvent) {
        match event.ty {
            TransactionType::Deposit => self.context.handle_transaction(
                event.client_id,
                event.tx,
                event.amount,
                DepositOrWithdraw::Deposit,
            ),
            TransactionType::Withdrawal => self.context.handle_transaction(
                event.client_id,
                event.tx,
                event.amount,
                DepositOrWithdraw::Withdraw,
            ),
            TransactionType::Dispute => self.context.handle_dispute(event.client_id, event.tx),
            TransactionType::Resolve => self.context.handle_resolve(event.client_id, event.tx),
            TransactionType::Chargeback => {
                self.context.handle_chargeback(event.client_id, event.tx)
            }
        }
    }
}
