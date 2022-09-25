use std::collections::HashMap;
use crate::types::transaction::{UTXOInput, UTXOOutput};
use crate::types::block::Block;
use crate::types::transaction::{self, SignedTransaction};
use crate::types::address::{self, Address};
use crate::types::hash::{H256, Hashable};
use crate::types::transaction_mempool::TransactionMempool;
use hex_literal::hex;
use crate::types::block;

// 某个block里面保存的state
#[derive(Debug, Default, Clone)]
pub struct State {
    pub state_map: HashMap<UTXOInput,UTXOOutput>,
}

// 整个区块链的state map，block的hash对应一个state
pub struct BlockState {
    pub block_state_map: HashMap<H256, State>,
}

impl BlockState {
    pub fn new() -> Self {
        let mut block_state_map = HashMap::new();
        // 初始state
        let state = ico();
        let genesis_block: Block = block::generate_genesis_block(&[0; 32].into());
        let genesis_hash: H256 = genesis_block.hash();
        // 插入到链的state_map中
        block_state_map.insert(genesis_hash, state);
        BlockState { block_state_map }
    }
}

// 验证transaction的合法性
pub fn verify_tx(tx: &SignedTransaction, state: &State) -> bool {
    // 验证公钥和签名
    if !transaction::verify(&tx.transaction, &tx.public_key, &tx.signature) {
        return false;
    }
    // owner地址
    let owner_address = Address::from_public_key_bytes(&tx.public_key);
    let mut total_input = 0;
    for input in &tx.transaction.input {
        // 检查double spend
        if !state.state_map.contains_key(&input) {
            println!("tx is double spend as input is not there in State!");
            return false;
        }
        let output = &state.state_map[&input];
        // 检查address，当前input的地址需要和前一个交易的output相同
        if output.recipient_address != owner_address {
            println!("owner of tx input doesn't match to previous tx output");
            println!("input addreess {:?}", owner_address);
            println!("output address {:?}", output.recipient_address);
            return false;
        }
        total_input += output.value;
    }
    let mut total_output = 0;
    for output in &tx.transaction.output {
        total_output += output.value;
    }
    // output不能大于input
    if total_input < total_output {
        println!("Input sum didn't match to output sum for tx");
        return false;
    }
    true
}

// 验证block中的transaction的合法性
pub fn verify_block(block: &Block, state: &State) -> bool {
    for tx in &block.content.data {
        if !verify_tx(tx, state) {
            return false;
        }
    }
    true
}

// 更新state
pub fn update_state(block: &Block, block_state: &mut BlockState) {
    // 拿到前一个block的state
    println!("parent{}",block.get_parent());
    let parent_state = &block_state.block_state_map[&block.get_parent()];
    let mut cur_block_state = parent_state.clone();
    for tx in &block.content.data {
        // 删除input
        for input in &tx.transaction.input {
            cur_block_state.state_map.remove(input);

        }
        // 生成新的input，存入当前的state_map中
        for (i, output) in (&tx.transaction.output).iter().enumerate() {
            let input = UTXOInput { tx_hash: tx.transaction.hash(), index: i as u8 };
            cur_block_state.state_map.insert(input, output.clone());
            //println!("insert new state");
        }
    }
    // 更新整个链的state_map
    block_state.block_state_map.insert(block.hash(), cur_block_state);
}

// 生成初始state，向初始地址中存入金额，并插入到state_map中
pub fn ico() -> State {
    let public_key: Vec<u8> = b"AAAAC3NzaC1lZDI1NTE5AAAAICYqyx/qrxvVPB2lPvV3ZmTH+uYwB6wL1hkBlGaYPmGu".to_vec();
    let address = Address::from_public_key_bytes(&public_key);
    let initial_tx_hash: H256 = hex!("6b787718210e0b3b608814e04e61fde06d0df794319a12162f287412df3ec920").into();
    let val: u32 = 500;
    let mut initial_state: State = State { state_map: HashMap::new() };
    let input = UTXOInput { tx_hash: initial_tx_hash, index: 0 };
    let output = UTXOOutput { recipient_address: address, value: val };
    initial_state.state_map.insert(input, output);
    initial_state
}