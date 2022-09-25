use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};
use log::{debug, info};
use crate::types::block::Block;
use crate::network::server::Handle as ServerHandle;
use std::thread;
use std::sync::{Arc, Mutex};
use crate::blockchain::Blockchain;
use crate::network::message::Message;
use crate::types::hash::{H256, Hashable};
use crate::types::ledger_state::{update_state, BlockState};

#[derive(Clone)]
pub struct Worker {
    server: ServerHandle,
    finished_block_chan: Receiver<Block>,
    blockchain: Arc<Mutex<Blockchain>>,
    block_state: Arc<Mutex<BlockState>>,
}

impl Worker {
    pub fn new(
        server: &ServerHandle,
        finished_block_chan: Receiver<Block>,
        blockchain: &Arc<Mutex<Blockchain>>,
        block_state: &Arc<Mutex<BlockState>>,
    ) -> Self {
        Self {
            server: server.clone(),
            finished_block_chan,
            blockchain: Arc::clone(blockchain),
            block_state: Arc::clone(block_state),
        }
    }

    pub fn start(self) {
        thread::Builder::new()
            .name("miner-worker".to_string())
            .spawn(move || {
                self.worker_loop();
            })
            .unwrap();
        info!("Miner initialized into paused mode");
    }

    fn worker_loop(&self) {
        loop {
            let block = self.finished_block_chan.recv().expect("Receive finished block error");
            // TODO for student: insert this finished block to blockchain, and broadcast this block hash
            // 插入block，并更新block_state
            let mut blockchain = self.blockchain.lock().unwrap();
            let mut block_state = self.block_state.lock().unwrap();
            blockchain.insert(&block);
            update_state(&block, &mut block_state);
            let mut block_hash: Vec<H256> = vec![];
            block_hash.push(block.hash());
            self.server.broadcast(Message::NewBlockHashes(block_hash));
            println!("New block, broadcast: {}", block.hash());
        }
    }
}
