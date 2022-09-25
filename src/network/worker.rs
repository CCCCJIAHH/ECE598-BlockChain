use super::message::Message;
use super::peer;
use super::server::Handle as ServerHandle;
use crate::types::hash::{H256, Hashable};

use log::{debug, warn, error};

use std::thread;

#[cfg(any(test, test_utilities))]
use super::peer::TestReceiver as PeerTestReceiver;
#[cfg(any(test, test_utilities))]
use super::server::TestReceiver as ServerTestReceiver;
use std::sync::{Arc, Mutex};
use crate::blockchain::Blockchain;
use crate::types::block::Block;
use crate::types::transaction_mempool::TransactionMempool;
use crate::types::transaction::{self, SignedTransaction};
use crate::types::ledger_state::{self, BlockState, verify_tx, State, verify_block, update_state};


#[derive(Clone)]
pub struct Worker {
    msg_chan: smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
    block_buffer: Vec<Block>,
    transaction_mempool: Arc<Mutex<TransactionMempool>>,
    block_state: Arc<Mutex<BlockState>>,
}


impl Worker {
    pub fn new(
        num_worker: usize,
        msg_src: smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
        server: &ServerHandle,
        blockchain: &Arc<Mutex<Blockchain>>,
        transaction_mempool: &Arc<Mutex<TransactionMempool>>,
        block_state: &Arc<Mutex<BlockState>>,
    ) -> Self {
        Self {
            msg_chan: msg_src,
            num_worker,
            server: server.clone(),
            blockchain: Arc::clone(blockchain),
            block_buffer: vec![],
            transaction_mempool: Arc::clone(transaction_mempool),
            block_state: Arc::clone(block_state),
        }
    }

    pub fn start(self) {
        let num_worker = self.num_worker;
        for i in 0..num_worker {
            let cloned = self.clone();
            thread::spawn(move || {
                cloned.worker_loop();
                warn!("Worker thread {} exited", i);
            });
        }
    }

    fn worker_loop(&self) {
        loop {
            let result = smol::block_on(self.msg_chan.recv());
            if let Err(e) = result {
                error!("network worker terminated {}", e);
                break;
            }
            let msg = result.unwrap();
            let (msg, mut peer) = msg;
            let msg: Message = bincode::deserialize(&msg).unwrap();
            let mut blockchain = self.blockchain.lock().unwrap();
            let mut mempool = self.transaction_mempool.lock().unwrap();
            let mut block_state = self.block_state.lock().unwrap();

            match msg {
                Message::Ping(nonce) => {
                    debug!("Ping: {}", nonce);
                    peer.write(Message::Pong(nonce.to_string()));
                }
                Message::Pong(nonce) => {
                    debug!("Pong: {}", nonce);
                }
                //For NewBlockHashes, if the hashes are not already in blockchain,
                //you need to ask for them by sending GetBlocks.
                Message::NewBlockHashes(hash_vec) => {
                    println!("get new block hash: {:?}", hash_vec);
                    let mut hash_to_add = vec![];
                    for hash in hash_vec {
                        let mut exist = false;
                        for (h, _) in blockchain.chain_map.iter() {
                            if *h == hash {
                                exist = true;
                            }
                        }
                        for (h, _) in blockchain.buffer_map.iter() {
                            if *h == hash {
                                exist = true;
                            }
                        }
                        if !exist { hash_to_add.push(hash); }
                    }
                    if !hash_to_add.is_empty() {
                        println!("reply to get blocks, {:?}", hash_to_add);
                        peer.write(Message::GetBlocks(hash_to_add));
                    }
                }
                // For GetBlocks, if the hashes are in blockchain,
                // you can get these blocks and reply them by Blocks message.
                Message::GetBlocks(hash_vec) => {
                    let mut block_to_add = vec![];
                    for hash in hash_vec {
                        for (h, block) in blockchain.chain_map.iter() {
                            if *h == hash {
                                block_to_add.push(block.clone());
                            }
                        }
                        for (h, block) in blockchain.buffer_map.iter() {
                            if *h == hash {
                                block_to_add.push(block.clone());
                            }
                        }
                    }
                    if !block_to_add.is_empty() {
                        println!("reply with blocks, {:?}", block_to_add);
                        peer.write(Message::Blocks(block_to_add));
                    }
                }
                //For Blocks, insert the blocks into blockchain if not already in it.
                //Also if the blocks are new to this node, you need to make a broadcast of a NewBlockHashes message.
                //NewBlockHashes message should contain hashes of blocks newly received.
                Message::Blocks(block_vec) => {
                    println!("receive new block: {:?}", block_vec);
                    let mut block_buffer = vec![];
                    let mut new_blocks = vec![];
                    for block in block_vec {
                        let mut valid = true;
                        // check transaction validity
                        match block_state.block_state_map.get(&block.get_parent()) {
                            None => { valid = false }
                            Some(state) => { valid = verify_block(&block, state) }
                        }
                        if valid {
                            // insert into blockchain
                            if !blockchain.buffer_map.contains_key(&block.hash()) &&
                                !blockchain.chain_map.contains_key(&block.hash()) {
                                blockchain.insert(&block);
                                new_blocks.push(block.hash());
                                if !blockchain.chain_map.contains_key(&block.get_parent()) {
                                    block_buffer.push(block.get_parent());
                                }
                            }
                            // update mempool
                            for tx in &block.content.data {
                                let tx_hash = tx.hash();
                                if mempool.tx_wait_map.contains_key(&tx_hash) {
                                    mempool.tx_wait_map.remove(&tx_hash);
                                }
                                mempool.tx_map.insert(tx_hash, tx.clone());
                            }
                            // update state
                            update_state(&block, &mut block_state);
                        }
                    }
                    if !block_buffer.is_empty() {
                        self.server.broadcast(Message::GetBlocks(block_buffer));
                    }
                    if !new_blocks.is_empty() {
                        self.server.broadcast(Message::NewBlockHashes(new_blocks));
                    }
                }
                // similar to NewBlockHashes
                Message::NewTransactionHashes(tx_hash_vec) => {
                    println!("get new tx hash");
                    let mut tx_hash_to_add = vec![];
                    for tx_hash in tx_hash_vec {
                        if !mempool.tx_wait_map.contains_key(&tx_hash) && !mempool.tx_map.contains_key(&tx_hash) {
                            tx_hash_to_add.push(tx_hash)
                        }
                    }
                    if !tx_hash_to_add.is_empty() {
                        println!("ask for tx");
                        peer.write(Message::GetTransactions(tx_hash_to_add));
                    }
                }
                // similar to GetBlocks
                Message::GetTransactions(tx_hash_vec) => {
                    println!("get new tx hash? : {:?}", tx_hash_vec);
                    let mut tx_to_add = vec![];
                    for tx_hash in tx_hash_vec {
                        match mempool.tx_map.get(&tx_hash) {
                            None => {}
                            Some(tx) => {
                                tx_to_add.push(tx.clone());
                            }
                        }
                        match mempool.tx_wait_map.get(&tx_hash) {
                            None => {}
                            Some(tx) => {
                                tx_to_add.push(tx.clone());
                            }
                        }
                    }
                    if !tx_to_add.is_empty() {
                        println!("reply tx vector");
                        peer.write(Message::Transactions(tx_to_add));
                    }
                }
                // similar to Blocks
                Message::Transactions(tx_vec) => {
                    println!("receive tx vec");
                    let mut new_tx_vec = vec![];
                    for tx in tx_vec {
                        if transaction::verify(&tx.transaction, &tx.public_key, &tx.signature) {
                            println!("tx verify success");
                            let tx_hash = tx.hash();
                            // new tx arrive
                            if !mempool.tx_wait_map.contains_key(&tx_hash) && !mempool.tx_map.contains_key(&tx_hash) {
                                println!("new hash insert into mem pool");
                                mempool.tx_wait_map.insert(tx_hash, tx);
                                mempool.tx_queue.push_back(tx_hash);
                                new_tx_vec.push(tx_hash);
                            }
                        } else {
                            println!("tx verify failed");
                        }
                    }
                    if !new_tx_vec.is_empty() {
                        self.server.broadcast(Message::NewTransactionHashes(new_tx_vec));
                    }
                }
            }
        }
    }
}

#[cfg(any(test, test_utilities))]
struct TestMsgSender {
    s: smol::channel::Sender<(Vec<u8>, peer::Handle)>
}

#[cfg(any(test, test_utilities))]
impl TestMsgSender {
    fn new() -> (TestMsgSender, smol::channel::Receiver<(Vec<u8>, peer::Handle)>) {
        let (s, r) = smol::channel::unbounded();
        (TestMsgSender { s }, r)
    }

    fn send(&self, msg: Message) -> PeerTestReceiver {
        let bytes = bincode::serialize(&msg).unwrap();
        let (handle, r) = peer::Handle::test_handle();
        smol::block_on(self.s.send((bytes, handle))).unwrap();
        r
    }
}

#[cfg(any(test, test_utilities))]
/// returns two structs used by tests, and an ordered vector of hashes of all blocks in the blockchain
fn generate_test_worker_and_start() -> (TestMsgSender, ServerTestReceiver, Vec<H256>) {
    let (server, server_receiver) = ServerHandle::new_for_test();
    let (test_msg_sender, msg_chan) = TestMsgSender::new();
    let blockchain = Blockchain::new();
    let blockchain = Arc::new(Mutex::new(blockchain));
    let worker = Worker::new(1, msg_chan, &server, &blockchain);
    worker.start();
    let x = blockchain.lock().unwrap().all_blocks_in_longest_chain();
    (test_msg_sender, server_receiver, x)
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod test {
    use ntest::timeout;
    use crate::types::block::generate_random_block;
    use crate::types::hash::Hashable;

    use super::super::message::Message;
    use super::generate_test_worker_and_start;

    #[test]
    #[timeout(60000)]
    fn reply_new_block_hashes() {
        let (test_msg_sender, _server_receiver, v) = generate_test_worker_and_start();
        let random_block = generate_random_block(v.last().unwrap());
        let mut peer_receiver = test_msg_sender.send(Message::NewBlockHashes(vec![random_block.hash()]));
        let reply = peer_receiver.recv();
        if let Message::GetBlocks(v) = reply {
            assert_eq!(v, vec![random_block.hash()]);
        } else {
            panic!();
        }
    }

    #[test]
    #[timeout(60000)]
    fn reply_get_blocks() {
        let (test_msg_sender, _server_receiver, v) = generate_test_worker_and_start();
        let h = v.last().unwrap().clone();
        let mut peer_receiver = test_msg_sender.send(Message::GetBlocks(vec![h.clone()]));
        let reply = peer_receiver.recv();
        if let Message::Blocks(v) = reply {
            assert_eq!(1, v.len());
            assert_eq!(h, v[0].hash())
        } else {
            panic!();
        }
    }

    #[test]
    #[timeout(60000)]
    fn reply_blocks() {
        let (test_msg_sender, server_receiver, v) = generate_test_worker_and_start();
        let random_block = generate_random_block(v.last().unwrap());
        let mut _peer_receiver = test_msg_sender.send(Message::Blocks(vec![random_block.clone()]));
        let reply = server_receiver.recv().unwrap();
        if let Message::NewBlockHashes(v) = reply {
            assert_eq!(v, vec![random_block.hash()]);
        } else {
            panic!();
        }
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST