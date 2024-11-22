# toy transaction engine

This is a small CLI application that is able to process transactions from a
given .csv file. Accounts are calculated and dumped to stdout.

# quick-start

(assumed is you have installed a rust toolchain)
* checkout repository `git clone https://github.com/svenrademakers/toy-transaction-engine.git`
* run `cargo run -- <path/to/csv>`

## csv format

following format is accepted as input:

```csv
type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 1, 2, 2
withdrawal, 1, 4, 1.5
dispute, 1, 2,
```

# Design

The choice is made to make the implementation of this application very
conservative as we are dealing with money, meaning, i tried to implement
extra checks in favor of execution speed.

i've settled on the following architecture before i started coding:

![quick design drawing](https://github.com/svenrademakers/toy-transaction-engine/blob/master/design.jpg)

Given the very limited time-frame, only a subset of this design is implemented.
But scaling was taken into account when designing this application.

* sources

source provide a stream of transaction data (`TransactionEvent`) which will be
written to the queue of the `TransactionProcessor`. In the implementation only the "CSV reader source" is implemented.
The queue is currently a SPSC implementation (given we have only one producer). It makes use of a ringbuffer with
seqlocks, one of the fastest inter thread implementations for queues.
If we were to have more consumers, similar implementation exists that support
multiple consumers (crossbeam).

* Transaction Processor

Is responsible for distributing the work of processing transaction data. The
idea is that workers can be scaled up depending on available cores or incoming
data. To keep it simple, the current implementation only has one worker.

* Shared Context

Is a store which stores submitted transactions and account data. This store
needs to have fast random access, and should be easy to extend. We need to
do a lot of lookups of transactions and updates of accounts.

* Stdout printer

Once the sources are exhausted, a printer prints the final accounts to stdout.

## Data Model

As mentioned before we will have a lot of random access.
* we need to lookup if an
given transaction already exist.
* on a disputes and chargeback's we need to look for the associated transactions.
* on deposits and
withdrawals we need to lookup and edit the associated accounts.

The account data and transaction data itself is relatively small.

Considering the previous points, using HashMap's would be the best fit given
insertion and random access takes a constant time. However they will produce
greater cache misses due to its scattered memory access pattern.
BTreeMaps would be a better for memory locality, but I will settle on
HashMaps. If i have time left i would like to measure if more cache friendly
containers are a better fit for this application.

