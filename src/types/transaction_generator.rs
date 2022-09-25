use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};
use crate::network::server::Handle as ServerHandle;
use std::sync::{Arc, Mutex};
use crate::types::transaction_mempool::TransactionMempool;
use std::{thread, time};
use crate::types::hash::{H256, Hashable};
use crate::types::transaction::{generate_random_signed_transaction, UTXOInput, UTXOOutput, Transaction, sign, SignedTransaction};
use crate::network::message::Message;
use log::info;
use crate::blockchain::Blockchain;
use crate::types::ledger_state::{State, BlockState};
use crate::types::{hash, key_pair};
use rand::Rng;
use ring::signature::KeyPair;
use crate::types::address::generate_random_address;

enum ControlSignal {
    Start(u64),
    // the number controls the theta of interval between transaction generation
    Exit,
}

enum OperatingState {
    Paused,
    Run(u64),
    ShutDown,
}

pub struct Context {
    control_chan: Receiver<ControlSignal>,
    operating_state: OperatingState,
    server: ServerHandle,
    mempool: Arc<Mutex<TransactionMempool>>,
    blockchain: Arc<Mutex<Blockchain>>,
    block_state: Arc<Mutex<BlockState>>,
}

#[derive(Clone)]
pub struct Handle {
    /// Channel for sending signal to the generator thread
    control_chan: Sender<ControlSignal>,
}

pub fn new(server: &ServerHandle, mempool: &Arc<Mutex<TransactionMempool>>, blockchain: &Arc<Mutex<Blockchain>>, block_state: &Arc<Mutex<BlockState>>) -> (Context, Handle) {
    let (signal_chan_sender, signal_chan_receiver) = unbounded();

    let ctx = Context {
        control_chan: signal_chan_receiver,
        operating_state: OperatingState::Paused,
        server: server.clone(),
        mempool: Arc::clone(mempool),
        blockchain: Arc::clone(blockchain),
        block_state: Arc::clone(block_state),
    };

    let handle = Handle {
        control_chan: signal_chan_sender,
    };

    (ctx, handle)
}

impl Handle {
    pub fn exit(&self) {
        self.control_chan.send(ControlSignal::Exit).unwrap();
    }

    pub fn start(&self, theta: u64) {
        self.control_chan
            .send(ControlSignal::Start(theta))
            .unwrap();
    }
}

impl Context {
    pub fn start(mut self) {
        thread::Builder::new()
            .name("transaction_generator".to_string())
            .spawn(move || {
                self.tx_gen_loop();
            })
            .unwrap();
        info!("Generator initialized into paused mode");
    }

    fn tx_gen_loop(&mut self) {
        loop {
            // check and react to control signals
            match self.operating_state {
                OperatingState::Paused => {
                    let signal = self.control_chan.recv().unwrap();
                    match signal {
                        ControlSignal::Exit => {
                            info!("Generator shutting down");
                            self.operating_state = OperatingState::ShutDown;
                        }
                        ControlSignal::Start(i) => {
                            info!("Generator starting in continuous mode with lambda {}", i);
                            self.operating_state = OperatingState::Run(i);
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
                                info!("Generator shutting down");
                                self.operating_state = OperatingState::ShutDown;
                            }
                            ControlSignal::Start(i) => {
                                info!("Generator starting in continuous mode with theta {}", i);
                                self.operating_state = OperatingState::Run(i);
                            }
                        };
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => panic!("Generator control channel detached"),
                },
            }
            if let OperatingState::ShutDown = self.operating_state {
                return;
            }

            //todo create transaction that can pass tx_check
            let mut signed_tx = generate_random_signed_transaction();
            let blockchain = self.blockchain.lock().unwrap();
            let block_state = self.block_state.lock().unwrap();
            let tip_hash = blockchain.tip;
            if !block_state.block_state_map.contains_key(&tip_hash) {
                println!("tip hash not in state map: {}", tip_hash);
                return;
            }
            // tip的state
            let state = block_state.block_state_map.get(&tip_hash).unwrap();
            for input in state.state_map.keys () {
                // output的地址和金额
                let output = &state.state_map[input];
                let address = output.recipient_address;
                let mut value = output.value;
                let input = vec![input.clone()];
                let mut output = vec![];
                // 随机分配金额给随机地址
                while value > 0 {
                    let mut rng = rand::thread_rng();
                    let val = rng.gen_range(0..value + 1);
                    let address=generate_random_address();
                    //println!("address: {} ", address);
                    output.push(UTXOOutput { recipient_address: address, value: val });
                    value -= val;
                }
                // 生成一个transaction，包括input和output vector
                let transaction = Transaction { input, output };
                let key = key_pair::random();
                let signature = sign(&transaction, &key);
                signed_tx = SignedTransaction {
                    transaction,
                    signature: signature.as_ref().to_vec(),
                    public_key: key.public_key().as_ref().to_vec(),
                };
                break;
            }
            // 把transaction存入到mempool中
            let mut tx_hash_vec = vec![];
            tx_hash_vec.push(signed_tx.hash());
            let mut mempool = self.mempool.lock().unwrap();
            let tx_hash = signed_tx.hash();
            println!("Generate transaction hash {}", tx_hash);
            mempool.tx_wait_map.insert(tx_hash, signed_tx);
            mempool.tx_queue.push_back(tx_hash);
            self.server.broadcast(Message::NewTransactionHashes(tx_hash_vec));

            if let OperatingState::Run(i) = self.operating_state {
                if i != 0 {
                    let interval = time::Duration::from_millis(i as u64);
                    thread::sleep(interval);
                }
            }
        }
    }
}