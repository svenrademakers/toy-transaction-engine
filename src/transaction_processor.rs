use crate::{
    data_types::{Account, TransactionEvent, TransactionFlags, TransactionType},
    transaction_context::TransactionContext,
};
use rtrb::Consumer;

#[derive(Debug)]
pub struct TransactionProcessor<'a> {
    context: &'a mut TransactionContext,
    consumer: Consumer<TransactionEvent>,
}

impl<'a> TransactionProcessor<'a> {
    /// Processes Events until the sources are exhausted.
    /// Returns a Iterator over the processed accounts.
    pub fn exhaust_sources(
        consumer: Consumer<TransactionEvent>,
    ) -> impl Iterator<Item = (u16, Account)> {
        let mut context = TransactionContext::new();

        // here multiple workers could be started, in this case the context needs to be converted
        // so it can thread-safe handle interior mutability.
        TransactionProcessor::new(&mut context, consumer).run();

        context.into_iter_accounts()
    }

    fn new(context: &'a mut TransactionContext, consumer: Consumer<TransactionEvent>) -> Self {
        TransactionProcessor { context, consumer }
    }

    fn run(mut self) {
        loop {
            if let Ok(mut event) = self.consumer.pop() {
                // precautionary call to make sure the interface is honored
                event.amount.make_absolute();
                self.update_accounts(event);
            } else if self.consumer.is_abandoned() {
                // we are done
                break;
            }
        }
    }

    fn update_accounts(&mut self, event: TransactionEvent) {
        match event.ty {
            TransactionType::Deposit => {
                self.context
                    .handle_transaction(&event, Account::deposit, true)
            }
            TransactionType::Withdrawal => {
                self.context
                    .handle_transaction(&event, Account::withdraw, false)
            }
            TransactionType::Dispute => self.context.handle_dispute(
                &event,
                (TransactionFlags::None, TransactionFlags::Disputed),
                Account::dispute,
            ),
            TransactionType::Resolve => self.context.handle_dispute(
                &event,
                (TransactionFlags::Disputed, TransactionFlags::Resolved),
                Account::resolve,
            ),
            TransactionType::Chargeback => self.context.handle_dispute(
                &event,
                (TransactionFlags::Disputed, TransactionFlags::Chargeback),
                Account::chargeback,
            ),
        }
    }
}
