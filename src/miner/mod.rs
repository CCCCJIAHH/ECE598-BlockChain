pub mod worker;

use log::info;

use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};
use std::time;

use std::thread;

use crate::types::block::{Block, Content, Header};
use std::sync::{Arc, Mutex};
use crate::blockchain::Blockchain;
use rand::Rng;
use chrono::Local;
use crate::types::transaction::generate_random_signed_transaction;
use ring::digest::{digest, SHA256};
use crate::types::hash::{H256, Hashable};
use crate::types::merkle::MerkleTree;
use crate::types::transaction_mempool::TransactionMempool;

enum ControlSignal {
    Start(u64),
    // the number controls the lambda of interval between block generation
    Update,
    // update the block in mining, it may due to new blockchain tip or new transaction
    Exit,
}

enum OperatingState {
    Paused,
    Run(u64),
    ShutDown,
}

pub struct Context {
    /// Channel for receiving control signal
    control_chan: Receiver<ControlSignal>,
    operating_state: OperatingState,
    finished_block_chan: Sender<Block>,
    blockchain: Arc<Mutex<Blockchain>>,
    tx_mempool: Arc<Mutex<TransactionMempool>>,
    block_num: u8,
    tip: H256,
    difficulty: H256, // constant difficulty for each block
}

#[derive(Clone)]
pub struct Handle {
    /// Channel for sending signal to the miner thread
    control_chan: Sender<ControlSignal>,
}

pub fn new(blockchain: &Arc<Mutex<Blockchain>>, tx_mempool: &Arc<Mutex<TransactionMempool>>) -> (Context, Handle, Receiver<Block>) {
    let (signal_chan_sender, signal_chan_receiver) = unbounded();
    let (finished_block_sender, finished_block_receiver) = unbounded();
    let tip = blockchain.lock().unwrap().tip();
    let difficulty = blockchain.lock().unwrap().chain_map.get(&tip).unwrap().get_difficulty();
    let ctx = Context {
        control_chan: signal_chan_receiver,
        operating_state: OperatingState::Paused,
        finished_block_chan: finished_block_sender,
        blockchain: Arc::clone(blockchain),
        tx_mempool: Arc::clone(tx_mempool),
        block_num: 0,
        tip,
        difficulty,
    };

    let handle = Handle {
        control_chan: signal_chan_sender,
    };

    (ctx, handle, finished_block_receiver)
}

#[cfg(any(test, test_utilities))]
fn test_new() -> (Context, Handle, Receiver<Block>) {
    let blockchain = Blockchain::new();
    let blockchain = Arc::new(Mutex::new(blockchain));
    let mempool = TransactionMempool::new();
    let mempool = Arc::new(Mutex::new(TransactionMempool));
    new(&blockchain, &mempool)
}

impl Handle {
    pub fn exit(&self) {
        self.control_chan.send(ControlSignal::Exit).unwrap();
    }

    pub fn start(&self, lambda: u64) {
        self.control_chan
            .send(ControlSignal::Start(lambda))
            .unwrap();
    }

    pub fn update(&self) {
        self.control_chan.send(ControlSignal::Update).unwrap();
    }
}

impl Context {
    pub fn start(mut self) {
        thread::Builder::new()
            .name("miner".to_string())
            .spawn(move || {
                self.miner_loop();
            })
            .unwrap();
        info!("Miner initialized into paused mode");
    }

    fn miner_loop(&mut self) {
        // main mining loop
        loop {
            // check and react to control signals
            match self.operating_state {
                OperatingState::Paused => {
                    let signal = self.control_chan.recv().unwrap();
                    match signal {
                        ControlSignal::Exit => {
                            info!("Miner shutting down");
                            self.operating_state = OperatingState::ShutDown;
                        }
                        ControlSignal::Start(i) => {
                            info!("Miner starting in continuous mode with lambda {}", i);
                            self.operating_state = OperatingState::Run(i);
                        }
                        ControlSignal::Update => {
                            // in paused state, don't need to update
                        }
                    };
                    continue;
                }
                OperatingState::ShutDown => {
                    return;
                }
                _ => match self.control_chan.try_recv() {
                    Ok(signal) => {
                        match signal {
                            ControlSignal::Exit => {
                                info!("Miner shutting down");
                                self.operating_state = OperatingState::ShutDown;
                            }
                            ControlSignal::Start(i) => {
                                info!("Miner starting in continuous mode with lambda {}", i);
                                self.operating_state = OperatingState::Run(i);
                            }
                            ControlSignal::Update => {
                                self.tip = self.blockchain.lock().unwrap().tip();
                            }
                        };
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => panic!("Miner control channel detached"),
                },
            }
            if let OperatingState::ShutDown = self.operating_state {
                return;
            }

            // TODO for student: actual mining, create a block
            // TODO for student: if block mining finished, you can have something like: self.finished_block_chan.send(block.clone()).expect("Send finished block error");
            // gather a block's fields
            let blockchain = self.blockchain.lock().unwrap();
            let parent = self.tip;
            let timestamp = Local::now().timestamp_millis();
            // let difficulty = blockchain.chain_map.get(&parent).unwrap().get_difficulty();
            let difficulty = self.difficulty;
            let nonce = rand::thread_rng().gen();
            // block content
            let mut tx_vec = vec![];
            let mut tx_remove = vec![];
            let mut tx_hash_vec = vec![];
            let mut mempool = self.tx_mempool.lock().unwrap();
           // println!("mempool tx queue length : {}", mempool.tx_queue.len());
            for i in 0..mempool.tx_queue.len() {
                let tx_hash = *mempool.tx_queue.get(i).unwrap();
                // println!("miner loop pop hash from tx queue: {}", tx_hash);
                let transaction = mempool.tx_wait_map.get(&tx_hash).unwrap().clone();
                tx_remove.push(transaction.clone());
                tx_hash_vec.push(tx_hash);
                tx_vec.push(transaction.clone());
                if tx_hash_vec.len() >= 15 { break; }
            }
            // generate merkle root using transaction
            let tx_hash_arr: &[H256] = &tx_hash_vec[..];
            let merkle_tree = MerkleTree::new(&tx_hash_arr);
            let merkle_root = merkle_tree.root();
            let content = Content { data: tx_vec };

            // generate new block
            let header = Header {
                parent,
                nonce,
                difficulty,
                timestamp,
                merkle_root,
            };
            let block = Block {
                header,
                content,
            };

            // check proof of work
            if block.hash() <= difficulty {
                println!("block with hash:{} generated\n", block.hash());
                self.block_num = self.block_num + 1;
                self.tip = block.hash();
                // println!("Number of blocks mined until now:{}", self.block_num);
                // println!("The number of blocks in the chain: {}", blockchain.chain_map.len());
                for i in 0..tx_hash_vec.len() {
                    let tx_hash = mempool.tx_queue.pop_front().unwrap();
                    let tx = tx_remove.get(i).unwrap();
                    mempool.tx_map.insert(tx_hash, tx.clone());
                    mempool.tx_wait_map.remove(&tx_hash);
                }
                self.finished_block_chan.send(block.clone()).expect("Send finished block error");
            }

            if let OperatingState::Run(i) = self.operating_state {
                if i != 0 {
                    let interval = time::Duration::from_micros(i as u64);
                    thread::sleep(interval);
                }
            }
        }
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod test {
    use ntest::timeout;
    use crate::types::hash::Hashable;

    #[test]
    #[timeout(60000)]
    fn miner_three_block() {
        let (miner_ctx, miner_handle, finished_block_chan) = super::test_new();
        miner_ctx.start();
        miner_handle.start(0);
        let mut block_prev = finished_block_chan.recv().unwrap();
        for i in 0..10 {
            println!("test iteration {}", i);
            let block_next = finished_block_chan.recv().unwrap();
            // println!("prev block {:?}", block_prev);
            // println!("prev block {:?}", block_next);
            assert_eq!(block_prev.hash(), block_next.get_parent());
            block_prev = block_next;
        }
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST