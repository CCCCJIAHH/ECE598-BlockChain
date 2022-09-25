use serde::{Serialize, Deserialize};
use crate::types::hash::{H256, Hashable};
use crate::types::transaction::SignedTransaction;
use rand::Rng;
use self::chrono::Local;
use hex_literal::hex;

extern crate chrono;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Header {
    pub parent: H256,
    pub nonce: u32,
    pub difficulty: H256,
    pub timestamp: i64,
    pub merkle_root: H256,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Content {
    pub data: Vec<SignedTransaction>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub header: Header,
    pub content: Content,
}

impl Hashable for Header {
    fn hash(&self) -> H256 {
        let encoded: Vec<u8> = bincode::serialize(&self).unwrap();
        let msg = &encoded[..];
        ring::digest::digest(&ring::digest::SHA256, msg).into()
    }
}

impl Hashable for Block {
    fn hash(&self) -> H256 {
        self.header.hash()
    }
}

impl Block {
    pub fn get_parent(&self) -> H256 {
        self.header.parent
    }

    pub fn get_difficulty(&self) -> H256 {
        self.header.difficulty
    }
}

#[cfg(any(test, test_utilities))]
pub fn generate_random_block(parent: &H256) -> Block {
    let nonce: u32 = rand::thread_rng().gen();
    let timestamp = Local::now().timestamp_millis();
    let difficulty = hex!("1013100000001000000000000100000000000000000000000000000000000000").into();
    let header = Header {
        parent: *parent,
        nonce,
        difficulty,
        timestamp,
        merkle_root: Default::default(),
    };
    let data = vec![];
    let content = Content {
        data,
    };
    let block = Block {
        header,
        content,
    };
    println!("{:?}", block);
    block
}

pub fn generate_genesis_block(parent: &H256) -> Block {
    let nonce: u32 = 0;
    let timestamp = 0;
    let difficulty = hex!("0004000000001000000000000100000000000000000000000000000000000000").into();
    let header = Header {
        parent: *parent,
        nonce,
        difficulty,
        timestamp,
        merkle_root: Default::default(),
    };
    let data = vec![];
    let content = Content {
        data,
    };
    let block = Block {
        header,
        content,
    };
    block
}