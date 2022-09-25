use crate::types::block::Block;
use crate::types::hash::{H256, Hashable};
use log::info;
use std::collections::HashMap;
use std::collections::VecDeque;
use self::chrono::Local;
use crate::types::block;

extern crate chrono;

use chrono::prelude::*;
use bincode::Error;
use clap::ErrorKind;

pub struct Blockchain {
    pub chain_map: HashMap<H256, Block>,
    pub tip: H256,
    pub length_map: HashMap<H256, i32>,
    pub buffer_map: HashMap<H256, Block>,
}

const GENESIS_PARENT: [u8; 32] = [0; 32];

impl Blockchain {
    /// Create a new blockchain, only containing the genesis block
    pub fn new() -> Self {
        let mut chain_map = HashMap::new();
        let mut length_map = HashMap::new();
        let genesis_block: Block = block::generate_genesis_block(&GENESIS_PARENT.into());
        let genesis_hash: H256 = genesis_block.hash();
        chain_map.insert(genesis_hash, genesis_block);
        length_map.insert(genesis_hash, 0);
        let buffer_map = HashMap::new();
        let chain = Blockchain {
            chain_map,
            tip: genesis_hash,
            length_map,
            buffer_map,
        };
        println!("genesis chain tip: {}", chain.tip);
        chain
    }

    /// Insert a block into blockchain
    pub fn insert(&mut self, block: &Block) {
        let hash = block.hash();
        println!("new block hash: {}", hash);
        match self.chain_map.get(&block.get_parent()) {
            // parent node not in the chain, store in the buffer map
            None => {
                if !self.buffer_map.contains_key(&hash) {
                    self.buffer_map.insert(hash, block.clone());
                }
            }

            Some(_parent_block) => {
                if hash <= _parent_block.get_difficulty() {
                    // put block in the chain and update current length
                    self.chain_map.insert(hash, block.clone());
                    let length = self.length_map[&block.get_parent()] + 1;
                    self.length_map.insert(hash, length);
                    if length > self.length_map[&self.tip] {
                        self.tip = hash;
                        println!("new length: {}, now tip: {}", length, self.tip);
                    }
                    // put all blocks in buffer map after this block if matched
                    let mut block_to_remove = Vec::new();
                    let mut hash_queue = Vec::new();
                    hash_queue.push(hash);
                    // do bfs
                    while !hash_queue.is_empty() {
                        let parent_hash = hash_queue.remove(0);
                        // find block in buffer map that match the parent hash
                        for (buffer_hash, buffer_block) in self.buffer_map.iter() {
                            if buffer_block.get_parent() == parent_hash {
                                let buffer_hash_copy = *buffer_hash;
                                // put block in remove list and push into queue
                                block_to_remove.push(buffer_hash_copy);
                                hash_queue.push(buffer_hash_copy);
                                // add to chain
                                self.chain_map.insert(buffer_hash_copy, buffer_block.clone());
                                let length = self.length_map[&buffer_block.get_parent()] + 1;
                                self.length_map.insert(buffer_hash_copy, length);
                                if length > self.length_map[&self.tip] {
                                    self.tip = hash;
                                }
                            }
                        }
                    }
                    // remove blocks from buffer map
                    for remove_block in block_to_remove {
                        self.buffer_map.remove(&remove_block);
                    }
                }
            }
        }
    }

    /// Get the last block's hash of the longest chain
    pub fn tip(&self) -> H256 {
        self.tip
    }

    /// Get all blocks' hashes of the longest chain, ordered from genesis to the tip
    pub fn all_blocks_in_longest_chain(&self) -> Vec<H256> {
        let mut parent_hash = self.tip;
        let mut res = vec![];
        let genesis_parent_hash = H256::from(GENESIS_PARENT);
        while parent_hash != genesis_parent_hash {
            res.push(parent_hash);
            parent_hash = self.chain_map.get(&parent_hash).unwrap().get_parent();
        }
        res.reverse();
        res
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::block::generate_random_block;
    use crate::types::hash::Hashable;

    #[test]
    fn insert_one() {
        let mut blockchain = Blockchain::new();
        let genesis_hash = blockchain.tip();
        let block = generate_random_block(&genesis_hash);
        blockchain.insert(&block);
        assert_eq!(blockchain.tip(), block.hash());
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST