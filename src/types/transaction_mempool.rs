use std::collections::HashMap;
use crate::types::hash::H256;
use crate::types::transaction::SignedTransaction;
use std::collections::VecDeque;

pub struct TransactionMempool {
    pub tx_queue: VecDeque<H256>,
    pub tx_wait_map: HashMap<H256, SignedTransaction>,
    pub tx_map: HashMap<H256, SignedTransaction>,
}

impl TransactionMempool {
    pub fn new() -> Self {
        TransactionMempool {
            tx_queue: VecDeque::new(),
            tx_wait_map: HashMap::new(),
            tx_map: HashMap::new(),
        }
    }
}