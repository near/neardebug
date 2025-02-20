pub mod errors;
pub mod logic;
pub mod profile;

use std::collections::BTreeMap;
use std::str::FromStr as _;
use std::sync::{Arc, Mutex, MutexGuard};

use js_sys::{ArrayBuffer, Uint8Array};
use logic::types::PromiseIndex;
pub use logic::with_ext_cost_counter;
use logic::{
    gas_counter, ExecutionResultState, External, GasCounter, MemSlice, VMContext, VMLogicError,
    ValuePtr,
};
use near_parameters::vm::Config;
pub use near_primitives_core::code::ContractCode;
use near_primitives_core::hash::CryptoHash;
use near_primitives_core::types::{
    AccountId, Balance, BlockHeight, EpochHeight, Gas, ProtocolVersion, StorageUsage,
};
use near_primitives_core::version::ProtocolFeature;
pub use profile::ProfileDataV3;
use serde::Serialize as _;
use serde_with::serde_as;
use std::result::Result as SResult;
use wasm_bindgen::prelude::*;

fn js_serializer() -> serde_wasm_bindgen::Serializer {
    serde_wasm_bindgen::Serializer::new()
        .serialize_missing_as_null(true)
        .serialize_large_number_types_as_bigints(true)
        .serialize_bytes_as_arrays(false)
}

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Default)]
#[serde(transparent)]
struct StoreMap {
    #[serde_as(as = "BTreeMap<serde_with::base64::Base64, serde_with::base64::Base64>")]
    map: BTreeMap<Vec<u8>, Vec<u8>>,
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct Store(Arc<Mutex<StoreMap>>);

#[wasm_bindgen]
impl Store {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self(Default::default())
    }

    pub fn from_json(array: Uint8Array) -> Result<Self> {
        let bytes = array.to_vec();
        Ok(Self(Arc::new(Mutex::new(serde_json::from_slice(&bytes)?))))
    }

    fn guard(&self) -> MutexGuard<StoreMap> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn size(&self) -> usize {
        let mut sum = 0;
        for (k, v) in &self.guard().map {
            sum += k.len();
            sum += v.len();
        }
        sum
    }

    pub fn to_json(&self) -> Result<Uint8Array> {
        let json = serde_json::to_vec(&*self.guard())?;
        let value = Uint8Array::new_with_length(json.len() as u32);
        value.copy_from(&json);
        Ok(value)
    }

    pub fn to_value(&self) -> Result<JsValue> {
        self.guard().serialize(&js_serializer()).map_err(Into::into)
    }

    pub fn set(&self, key: &[u8], value: &[u8]) {
        self.guard().map.insert(key.to_vec(), value.to_vec());
    }

    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.guard().map.get(key).cloned()
    }

    pub fn remove(&self, key: &[u8]) {
        self.guard().map.remove(key);
    }

    pub fn remove_subtree(&self, prefix: &[u8]) {
        self.guard().map.retain(|key, _| !key.starts_with(prefix));
    }

    pub fn has_key(&self, key: &[u8]) -> bool {
        self.guard().map.contains_key(key)
    }
}

struct Receipt {
    receiver: AccountId,
}

#[wasm_bindgen]
pub struct DebugExternal {
    store: Store,
    protocol_version: ProtocolVersion,
    action_hash: CryptoHash,
    prev_block_hash: CryptoHash,
    last_block_hash: CryptoHash,
    block_height: BlockHeight,
    data_count: u64,
    validators: BTreeMap<AccountId, Balance>,
    receipts: Vec<Receipt>,
}

#[wasm_bindgen]
impl DebugExternal {
    #[wasm_bindgen(constructor)]
    pub fn new(store: &Store, context: &Context, protocol_version: ProtocolVersion) -> Self {
        Self {
            store: store.clone(),
            action_hash: CryptoHash::default(),
            block_height: context.0.block_height,
            prev_block_hash: CryptoHash::default(),
            last_block_hash: CryptoHash::default(),
            data_count: 0,
            validators: Default::default(),
            protocol_version,
            receipts: Vec::new(),
        }
    }

    fn append_action(&mut self, receipt_index: logic::types::ReceiptIndex) {
        self.receipts
            .get(receipt_index as usize)
            .expect("receipt index should have been returned from runtime");
    }
}

impl External for DebugExternal {
    fn storage_set(&mut self, key: &[u8], value: &[u8]) -> SResult<(), VMLogicError> {
        self.store.set(key, value);
        Ok(())
    }

    fn storage_get<'a>(
        &'a self,
        key: &[u8],
        _: near_parameters::vm::StorageGetMode,
    ) -> SResult<Option<Box<dyn logic::ValuePtr + 'a>>, VMLogicError> {
        pub struct MockedValuePtr {
            value: Vec<u8>,
        }

        impl MockedValuePtr {
            pub fn new<T>(value: T) -> Self
            where
                T: AsRef<[u8]>,
            {
                MockedValuePtr {
                    value: value.as_ref().to_vec(),
                }
            }
        }

        impl ValuePtr for MockedValuePtr {
            fn len(&self) -> u32 {
                self.value.len() as u32
            }

            fn deref(&self) -> SResult<Vec<u8>, VMLogicError> {
                Ok(self.value.clone())
            }
        }

        let v = self.store.get(key);
        Ok(v.map(|v| Box::new(MockedValuePtr::new(&v)) as Box<_>))
    }

    fn storage_remove(&mut self, key: &[u8]) -> SResult<(), VMLogicError> {
        self.store.remove(key);
        Ok(())
    }

    fn storage_remove_subtree(&mut self, prefix: &[u8]) -> SResult<(), VMLogicError> {
        self.store.remove_subtree(prefix);
        Ok(())
    }

    fn storage_has_key(
        &mut self,
        key: &[u8],
        _: near_parameters::vm::StorageGetMode,
    ) -> SResult<bool, VMLogicError> {
        Ok(self.store.has_key(key))
    }

    fn generate_data_id(&mut self) -> CryptoHash {
        /// FIXME: get this outta near_primitives?
        fn create_hash_upgradable(
            protocol_version: near_primitives_core::types::ProtocolVersion,
            base: &CryptoHash,
            extra_hash_old: &CryptoHash,
            extra_hash: &CryptoHash,
            block_height: near_primitives_core::types::BlockHeight,
            salt: u64,
        ) -> CryptoHash {
            const BYTES_LEN: usize =
                size_of::<CryptoHash>() + size_of::<CryptoHash>() + size_of::<u64>();
            let mut bytes: Vec<u8> = Vec::with_capacity(BYTES_LEN);
            bytes.extend_from_slice(base.as_ref());
            if ProtocolFeature::BlockHeightForReceiptId.enabled(protocol_version) {
                bytes.extend_from_slice(block_height.to_le_bytes().as_ref())
            } else if protocol_version >= 42 {
                bytes.extend_from_slice(extra_hash.as_ref())
            } else {
                bytes.extend_from_slice(extra_hash_old.as_ref())
            };
            bytes.extend(salt.to_le_bytes());
            CryptoHash::hash_bytes(&bytes)
        }
        let hash = create_hash_upgradable(
            self.protocol_version,
            &self.action_hash,
            &self.prev_block_hash,
            &self.last_block_hash,
            self.block_height,
            self.data_count,
        );
        self.data_count += 1;
        hash
    }

    fn get_trie_nodes_count(&self) -> logic::TrieNodesCount {
        logic::TrieNodesCount {
            db_reads: 0,
            mem_reads: 0,
        }
    }

    fn get_recorded_storage_size(&self) -> usize {
        0
    }

    fn validator_stake(&self, account_id: &AccountId) -> SResult<Option<Balance>, VMLogicError> {
        Ok(self.validators.get(account_id).cloned())
    }

    fn validator_total_stake(&self) -> SResult<Balance, VMLogicError> {
        Ok(self.validators.values().sum())
    }

    fn create_action_receipt(
        &mut self,
        _receipt_indices: Vec<logic::types::ReceiptIndex>,
        receiver_id: AccountId,
    ) -> SResult<logic::types::ReceiptIndex, logic::VMLogicError> {
        let index = self.receipts.len();
        self.receipts.push(Receipt {
            receiver: receiver_id,
        });
        Ok(index as u64)
    }

    fn create_promise_yield_receipt(
        &mut self,
        receiver_id: AccountId,
    ) -> SResult<(logic::types::ReceiptIndex, CryptoHash), logic::VMLogicError> {
        let index = self.receipts.len();
        let data_id = self.generate_data_id();
        self.receipts.push(Receipt {
            receiver: receiver_id,
        });
        Ok((index as u64, data_id))
    }

    fn submit_promise_resume_data(
        &mut self,
        data_id: CryptoHash,
        data: Vec<u8>,
    ) -> SResult<bool, logic::VMLogicError> {
        todo!()
    }

    fn append_action_create_account(
        &mut self,
        receipt_index: logic::types::ReceiptIndex,
    ) -> SResult<(), logic::VMLogicError> {
        self.append_action(receipt_index);
        Ok(())
    }

    fn append_action_deploy_contract(
        &mut self,
        receipt_index: logic::types::ReceiptIndex,
        _code: Vec<u8>,
    ) -> SResult<(), logic::VMLogicError> {
        self.append_action(receipt_index);
        Ok(())
    }

    fn append_action_function_call_weight(
        &mut self,
        receipt_index: logic::types::ReceiptIndex,
        _method_name: Vec<u8>,
        _args: Vec<u8>,
        _attached_deposit: Balance,
        _prepaid_gas: Gas,
        _gas_weight: near_primitives_core::types::GasWeight,
    ) -> SResult<(), logic::VMLogicError> {
        self.append_action(receipt_index);
        Ok(())
    }

    fn append_action_transfer(
        &mut self,
        receipt_index: logic::types::ReceiptIndex,
        _deposit: Balance,
    ) -> SResult<(), logic::VMLogicError> {
        self.append_action(receipt_index);
        Ok(())
    }

    fn append_action_stake(
        &mut self,
        receipt_index: logic::types::ReceiptIndex,
        _stake: Balance,
        _public_key: near_crypto::PublicKey,
    ) {
        self.append_action(receipt_index);
    }

    fn append_action_add_key_with_full_access(
        &mut self,
        receipt_index: logic::types::ReceiptIndex,
        _public_key: near_crypto::PublicKey,
        _nonce: near_primitives_core::types::Nonce,
    ) {
        self.append_action(receipt_index);
    }

    fn append_action_add_key_with_function_call(
        &mut self,
        receipt_index: logic::types::ReceiptIndex,
        _public_key: near_crypto::PublicKey,
        _nonce: near_primitives_core::types::Nonce,
        _allowance: Option<Balance>,
        _receiver_id: AccountId,
        _method_names: Vec<Vec<u8>>,
    ) -> SResult<(), logic::VMLogicError> {
        self.append_action(receipt_index);
        Ok(())
    }

    fn append_action_delete_key(
        &mut self,
        receipt_index: logic::types::ReceiptIndex,
        _public_key: near_crypto::PublicKey,
    ) {
        self.append_action(receipt_index);
    }

    fn append_action_delete_account(
        &mut self,
        receipt_index: logic::types::ReceiptIndex,
        _beneficiary_id: AccountId,
    ) -> SResult<(), logic::VMLogicError> {
        self.append_action(receipt_index);
        Ok(())
    }

    fn get_receipt_receiver(&self, receipt_index: logic::types::ReceiptIndex) -> &AccountId {
        &self
            .receipts
            .get(receipt_index as usize)
            .expect("receipt index should have been returned by the runtime")
            .receiver
    }
}

#[wasm_bindgen]
pub struct Context(VMContext);

#[wasm_bindgen]
impl Context {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let alice = AccountId::from_str("alice.near").unwrap();
        let bob = AccountId::from_str("bob.near").unwrap();
        Self(VMContext {
            current_account_id: alice.clone(),
            signer_account_id: bob.clone(),
            signer_account_pk: CryptoHash::default().as_bytes().to_vec(),
            predecessor_account_id: bob,
            attached_deposit: 0,
            input: vec![],
            promise_results: Default::default(),
            block_height: 0,
            block_timestamp: 0,
            epoch_height: 0,
            account_balance: 100000000000000000000000000,
            account_locked_balance: 500000000000000000000000,
            prepaid_gas: 300000000000000,
            random_seed: CryptoHash::default().as_bytes().to_vec(),
            view_config: None,
            output_data_receivers: vec![],
            storage_usage: 1,
        })
    }

    pub fn gas(mut self, gas: &str) -> Result<Self> {
        self.0.prepaid_gas = gas.parse()?;
        Ok(self)
    }

    pub fn input_str(mut self, value: &str) -> Self {
        self.0.input = Vec::from(value.as_bytes());
        self
    }

    pub fn attached_deposit(mut self, deposit: &str) -> Result<Self> {
        self.0.attached_deposit = deposit.parse()?;
        Ok(self)
    }

    pub fn balance(mut self, balance: &str) -> Result<Self> {
        self.0.account_balance = balance.parse()?;
        Ok(self)
    }

    pub fn locked_balance(mut self, balance: &str) -> Result<Self> {
        self.0.account_locked_balance = balance.parse()?;
        Ok(self)
    }

    pub fn current_account(mut self, account: &str) -> Result<Self> {
        self.0.current_account_id = account.parse()?;
        Ok(self)
    }

    pub fn signer_account(mut self, account: &str) -> Result<Self> {
        self.0.signer_account_id = account.parse()?;
        Ok(self)
    }

    pub fn signer_account_pk(mut self, key: &str) -> Result<Self> {
        self.0.signer_account_pk = CryptoHash::from_str(key)
            .map_err(|e| JsError::new(&e.to_string()))?
            .as_bytes()
            .to_vec();
        Ok(self)
    }

    pub fn predecessor_account(mut self, account: &str) -> Result<Self> {
        self.0.predecessor_account_id = account.parse()?;
        Ok(self)
    }

    pub fn block_height(mut self, height: &str) -> Result<Self> {
        self.0.block_height = height.parse()?;
        Ok(self)
    }

    pub fn block_timestamp(mut self, timestamp: &str) -> Result<Self> {
        self.0.block_timestamp = timestamp.parse()?;
        Ok(self)
    }

    pub fn epoch_height(mut self, height: &str) -> Result<Self> {
        self.0.epoch_height = height.parse()?;
        Ok(self)
    }

    pub fn random_seed(mut self, key: &str) -> Result<Self> {
        self.0.random_seed = CryptoHash::from_str(key)
            .map_err(|e| JsError::new(&e.to_string()))?
            .as_bytes()
            .to_vec();
        Ok(self)
    }
}

#[wasm_bindgen]
pub struct Logic {
    logic: logic::VMLogic,
}

type Result<T> = std::result::Result<T, JsError>;

#[wasm_bindgen]
impl Logic {
    #[wasm_bindgen(constructor)]
    pub fn new(context: Context, memory: js_sys::WebAssembly::Memory, ext: DebugExternal) -> Self {
        let config_store = near_parameters::RuntimeConfigStore::new(None);
        let config = config_store.get_config(near_primitives_core::version::PROTOCOL_VERSION);
        let max_gas_burnt = match context.0.view_config {
            Some(ref v) => v.max_gas_burnt,
            None => config.wasm_config.limit_config.max_gas_burnt,
        };
        let gas_counter = GasCounter::new(
            config.wasm_config.ext_costs.clone(),
            max_gas_burnt,
            config.wasm_config.regular_op_cost,
            context.0.prepaid_gas,
            context.0.view_config.is_some(),
        );
        let result_state =
            ExecutionResultState::new(&context.0, gas_counter, config.wasm_config.clone());
        let ext = Box::new(ext);
        Self {
            logic: logic::VMLogic::new(
                ext,
                context.0,
                config.fees.clone(),
                result_state,
                Box::new(memory),
            ),
        }
    }

    pub fn context(&self) -> Result<JsValue> {
        self.logic
            .context
            .serialize(&js_serializer())
            .map_err(Into::into)
    }

    pub fn outcome(&self) -> Result<JsValue> {
        self.logic
            .result_state
            .clone()
            .compute_outcome()
            .serialize(&js_serializer())
            .map_err(Into::into)
    }

    pub fn registers(&mut self) -> Result<JsValue> {
        let s = js_serializer();
        self.logic.registers().serialize(&s).map_err(Into::into)
    }

    pub fn fees_before_loading_executable(
        &mut self,
        method_name: &str,
        code_len: u64,
    ) -> Result<()> {
        let config = self.logic.config().clone();
        self.logic
            .gas_counter()
            .before_loading_executable(&config, method_name, code_len)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    pub fn fees_after_loading_executable(&mut self, code_len: u64) -> Result<()> {
        let config = self.logic.config().clone();
        self.logic
            .gas_counter()
            .after_loading_executable(&config, code_len)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    pub fn finite_wasm_gas(&mut self, gas: u64) -> Result<()> {
        self.logic.finite_wasm_gas(gas).map_err(Into::into)
    }

    pub fn finite_wasm_stack(&mut self, operand_size: u64, frame_size: u64) -> Result<()> {
        self.logic
            .finite_wasm_stack(operand_size, frame_size)
            .map_err(Into::into)
    }

    pub fn finite_wasm_unstack(&mut self, operand_size: u64, frame_size: u64) -> Result<()> {
        self.logic
            .finite_wasm_unstack(operand_size, frame_size)
            .map_err(Into::into)
    }

    pub fn read_register(&mut self, register_id: u64, ptr: u64) -> Result<()> {
        self.logic
            .read_register(register_id, ptr)
            .map_err(Into::into)
    }

    pub fn register_len(&mut self, register_id: u64) -> Result<u64> {
        self.logic.register_len(register_id).map_err(Into::into)
    }

    pub fn write_register(&mut self, register_id: u64, data_len: u64, data_ptr: u64) -> Result<()> {
        self.logic
            .write_register(register_id, data_len, data_ptr)
            .map_err(Into::into)
    }

    pub fn current_account_id(&mut self, register_id: u64) -> Result<()> {
        self.logic
            .current_account_id(register_id)
            .map_err(Into::into)
    }

    pub fn signer_account_id(&mut self, register_id: u64) -> Result<()> {
        self.logic
            .signer_account_id(register_id)
            .map_err(Into::into)
    }

    pub fn signer_account_pk(&mut self, register_id: u64) -> Result<()> {
        self.logic
            .signer_account_pk(register_id)
            .map_err(Into::into)
    }

    pub fn predecessor_account_id(&mut self, register_id: u64) -> Result<()> {
        self.logic
            .predecessor_account_id(register_id)
            .map_err(Into::into)
    }

    pub fn input(&mut self, register_id: u64) -> Result<()> {
        self.logic.input(register_id).map_err(Into::into)
    }

    pub fn block_index(&mut self) -> Result<u64> {
        self.logic.block_index().map_err(Into::into)
    }

    pub fn block_timestamp(&mut self) -> Result<u64> {
        self.logic.block_timestamp().map_err(Into::into)
    }

    pub fn epoch_height(&mut self) -> Result<EpochHeight> {
        self.logic.epoch_height().map_err(Into::into)
    }

    pub fn validator_stake(
        &mut self,
        account_id_len: u64,
        account_id_ptr: u64,
        stake_ptr: u64,
    ) -> Result<()> {
        self.logic
            .validator_stake(account_id_len, account_id_ptr, stake_ptr)
            .map_err(Into::into)
    }

    pub fn validator_total_stake(&mut self, stake_ptr: u64) -> Result<()> {
        self.logic
            .validator_total_stake(stake_ptr)
            .map_err(Into::into)
    }

    pub fn storage_usage(&mut self) -> Result<StorageUsage> {
        self.logic.storage_usage().map_err(Into::into)
    }

    pub fn account_balance(&mut self, balance_ptr: u64) -> Result<()> {
        self.logic.account_balance(balance_ptr).map_err(Into::into)
    }

    pub fn account_locked_balance(&mut self, balance_ptr: u64) -> Result<()> {
        self.logic
            .account_locked_balance(balance_ptr)
            .map_err(Into::into)
    }

    pub fn attached_deposit(&mut self, balance_ptr: u64) -> Result<()> {
        self.logic.attached_deposit(balance_ptr).map_err(Into::into)
    }

    pub fn prepaid_gas(&mut self) -> Result<Gas> {
        self.logic.prepaid_gas().map_err(Into::into)
    }

    pub fn used_gas(&mut self) -> Result<Gas> {
        self.logic.used_gas().map_err(Into::into)
    }

    pub fn alt_bn128_g1_multiexp(
        &mut self,
        value_len: u64,
        value_ptr: u64,
        register_id: u64,
    ) -> Result<()> {
        self.logic
            .alt_bn128_g1_multiexp(value_len, value_ptr, register_id)
            .map_err(Into::into)
    }

    pub fn alt_bn128_g1_sum(
        &mut self,
        value_len: u64,
        value_ptr: u64,
        register_id: u64,
    ) -> Result<()> {
        self.logic
            .alt_bn128_g1_sum(value_len, value_ptr, register_id)
            .map_err(Into::into)
    }

    pub fn alt_bn128_pairing_check(&mut self, value_len: u64, value_ptr: u64) -> Result<u64> {
        self.logic
            .alt_bn128_pairing_check(value_len, value_ptr)
            .map_err(Into::into)
    }

    pub fn random_seed(&mut self, register_id: u64) -> Result<()> {
        self.logic.random_seed(register_id).map_err(Into::into)
    }

    pub fn sha256(&mut self, value_len: u64, value_ptr: u64, register_id: u64) -> Result<()> {
        self.logic
            .sha256(value_len, value_ptr, register_id)
            .map_err(Into::into)
    }

    pub fn keccak256(&mut self, value_len: u64, value_ptr: u64, register_id: u64) -> Result<()> {
        self.logic
            .keccak256(value_len, value_ptr, register_id)
            .map_err(Into::into)
    }

    pub fn keccak512(&mut self, value_len: u64, value_ptr: u64, register_id: u64) -> Result<()> {
        self.logic
            .keccak512(value_len, value_ptr, register_id)
            .map_err(Into::into)
    }

    pub fn ripemd160(&mut self, value_len: u64, value_ptr: u64, register_id: u64) -> Result<()> {
        self.logic
            .ripemd160(value_len, value_ptr, register_id)
            .map_err(Into::into)
    }

    // pub fn ecrecover(
    //     &mut self,
    //     hash_len: u64,
    //     hash_ptr: u64,
    //     sig_len: u64,
    //     sig_ptr: u64,
    //     v: u64,
    //     malleability_flag: u64,
    //     register_id: u64,
    // ) -> Result<u64> {
    //     self.logic
    //         .ecrecover(
    //             hash_len,
    //             hash_ptr,
    //             sig_len,
    //             sig_ptr,
    //             v,
    //             malleability_flag,
    //             register_id,
    //         )
    //         .map_err(Into::into)
    // }

    pub fn ed25519_verify(
        &mut self,
        signature_len: u64,
        signature_ptr: u64,
        message_len: u64,
        message_ptr: u64,
        public_key_len: u64,
        public_key_ptr: u64,
    ) -> Result<u64> {
        self.logic
            .ed25519_verify(
                signature_len,
                signature_ptr,
                message_len,
                message_ptr,
                public_key_len,
                public_key_ptr,
            )
            .map_err(Into::into)
    }

    pub fn gas(&mut self, opcodes: u32) -> Result<()> {
        self.logic.gas_seen_from_wasm(opcodes).map_err(Into::into)
    }

    pub fn burn_gas(&mut self, gas: Gas) -> Result<()> {
        self.logic.gas(gas).map_err(Into::into)
    }

    pub fn promise_create(
        &mut self,
        account_id_len: u64,
        account_id_ptr: u64,
        method_name_len: u64,
        method_name_ptr: u64,
        arguments_len: u64,
        arguments_ptr: u64,
        amount_ptr: u64,
        gas: Gas,
    ) -> Result<u64> {
        self.logic
            .promise_create(
                account_id_len,
                account_id_ptr,
                method_name_len,
                method_name_ptr,
                arguments_len,
                arguments_ptr,
                amount_ptr,
                gas,
            )
            .map_err(Into::into)
    }

    pub fn promise_then(
        &mut self,
        promise_idx: u64,
        account_id_len: u64,
        account_id_ptr: u64,
        method_name_len: u64,
        method_name_ptr: u64,
        arguments_len: u64,
        arguments_ptr: u64,
        amount_ptr: u64,
        gas: u64,
    ) -> Result<u64> {
        self.logic
            .promise_then(
                promise_idx,
                account_id_len,
                account_id_ptr,
                method_name_len,
                method_name_ptr,
                arguments_len,
                arguments_ptr,
                amount_ptr,
                gas,
            )
            .map_err(Into::into)
    }

    pub fn promise_and(
        &mut self,
        promise_idx_ptr: u64,
        promise_idx_count: u64,
    ) -> Result<PromiseIndex> {
        self.logic
            .promise_and(promise_idx_ptr, promise_idx_count)
            .map_err(Into::into)
    }

    pub fn promise_batch_create(
        &mut self,
        account_id_len: u64,
        account_id_ptr: u64,
    ) -> Result<u64> {
        self.logic
            .promise_batch_create(account_id_len, account_id_ptr)
            .map_err(Into::into)
    }

    pub fn promise_batch_then(
        &mut self,
        promise_idx: u64,
        account_id_len: u64,
        account_id_ptr: u64,
    ) -> Result<u64> {
        self.logic
            .promise_batch_then(promise_idx, account_id_len, account_id_ptr)
            .map_err(Into::into)
    }

    pub fn promise_batch_action_create_account(&mut self, promise_idx: u64) -> Result<()> {
        self.logic
            .promise_batch_action_create_account(promise_idx)
            .map_err(Into::into)
    }

    pub fn promise_batch_action_deploy_contract(
        &mut self,
        promise_idx: u64,
        code_len: u64,
        code_ptr: u64,
    ) -> Result<()> {
        self.logic
            .promise_batch_action_deploy_contract(promise_idx, code_len, code_ptr)
            .map_err(Into::into)
    }

    pub fn promise_batch_action_function_call(
        &mut self,
        promise_idx: u64,
        method_name_len: u64,
        method_name_ptr: u64,
        arguments_len: u64,
        arguments_ptr: u64,
        amount_ptr: u64,
        gas: Gas,
    ) -> Result<()> {
        self.logic
            .promise_batch_action_function_call(
                promise_idx,
                method_name_len,
                method_name_ptr,
                arguments_len,
                arguments_ptr,
                amount_ptr,
                gas,
            )
            .map_err(Into::into)
    }

    pub fn promise_batch_action_function_call_weight(
        &mut self,
        promise_idx: u64,
        method_name_len: u64,
        method_name_ptr: u64,
        arguments_len: u64,
        arguments_ptr: u64,
        amount_ptr: u64,
        gas: Gas,
        gas_weight: u64,
    ) -> Result<()> {
        self.logic
            .promise_batch_action_function_call_weight(
                promise_idx,
                method_name_len,
                method_name_ptr,
                arguments_len,
                arguments_ptr,
                amount_ptr,
                gas,
                gas_weight,
            )
            .map_err(Into::into)
    }

    pub fn promise_batch_action_transfer(
        &mut self,
        promise_idx: u64,
        amount_ptr: u64,
    ) -> Result<()> {
        self.logic
            .promise_batch_action_transfer(promise_idx, amount_ptr)
            .map_err(Into::into)
    }

    pub fn promise_batch_action_stake(
        &mut self,
        promise_idx: u64,
        amount_ptr: u64,
        public_key_len: u64,
        public_key_ptr: u64,
    ) -> Result<()> {
        self.logic
            .promise_batch_action_stake(promise_idx, amount_ptr, public_key_len, public_key_ptr)
            .map_err(Into::into)
    }

    pub fn promise_batch_action_add_key_with_full_access(
        &mut self,
        promise_idx: u64,
        public_key_len: u64,
        public_key_ptr: u64,
        nonce: u64,
    ) -> Result<()> {
        self.logic
            .promise_batch_action_add_key_with_full_access(
                promise_idx,
                public_key_len,
                public_key_ptr,
                nonce,
            )
            .map_err(Into::into)
    }

    pub fn promise_batch_action_add_key_with_function_call(
        &mut self,
        promise_idx: u64,
        public_key_len: u64,
        public_key_ptr: u64,
        nonce: u64,
        allowance_ptr: u64,
        receiver_id_len: u64,
        receiver_id_ptr: u64,
        method_names_len: u64,
        method_names_ptr: u64,
    ) -> Result<()> {
        self.logic
            .promise_batch_action_add_key_with_function_call(
                promise_idx,
                public_key_len,
                public_key_ptr,
                nonce,
                allowance_ptr,
                receiver_id_len,
                receiver_id_ptr,
                method_names_len,
                method_names_ptr,
            )
            .map_err(Into::into)
    }

    pub fn promise_batch_action_delete_key(
        &mut self,
        promise_idx: u64,
        public_key_len: u64,
        public_key_ptr: u64,
    ) -> Result<()> {
        self.logic
            .promise_batch_action_delete_key(promise_idx, public_key_len, public_key_ptr)
            .map_err(Into::into)
    }

    pub fn promise_batch_action_delete_account(
        &mut self,
        promise_idx: u64,
        beneficiary_id_len: u64,
        beneficiary_id_ptr: u64,
    ) -> Result<()> {
        self.logic
            .promise_batch_action_delete_account(
                promise_idx,
                beneficiary_id_len,
                beneficiary_id_ptr,
            )
            .map_err(Into::into)
    }

    pub fn promise_yield_create(
        &mut self,
        method_name_len: u64,
        method_name_ptr: u64,
        arguments_len: u64,
        arguments_ptr: u64,
        gas: Gas,
        gas_weight: u64,
        register_id: u64,
    ) -> Result<u64> {
        self.logic
            .promise_yield_create(
                method_name_len,
                method_name_ptr,
                arguments_len,
                arguments_ptr,
                gas,
                gas_weight,
                register_id,
            )
            .map_err(Into::into)
    }

    pub fn promise_yield_resume(
        &mut self,
        data_id_len: u64,
        data_id_ptr: u64,
        payload_len: u64,
        payload_ptr: u64,
    ) -> Result<u32> {
        self.logic
            .promise_yield_resume(data_id_len, data_id_ptr, payload_len, payload_ptr)
            .map_err(Into::into)
    }

    pub fn promise_results_count(&mut self) -> Result<u64> {
        self.logic.promise_results_count().map_err(Into::into)
    }

    pub fn promise_result(&mut self, result_idx: u64, register_id: u64) -> Result<u64> {
        self.logic
            .promise_result(result_idx, register_id)
            .map_err(Into::into)
    }

    pub fn promise_return(&mut self, promise_idx: u64) -> Result<()> {
        self.logic.promise_return(promise_idx).map_err(Into::into)
    }

    pub fn value_return(&mut self, value_len: u64, value_ptr: u64) -> Result<()> {
        self.logic
            .value_return(value_len, value_ptr)
            .map_err(Into::into)
    }

    pub fn get_utf8_string_free(&mut self, len: u64, ptr: u64) -> Result<String> {
        self.logic
            .get_utf8_string_free(len, ptr)
            .map_err(Into::into)
    }

    pub fn log_utf8(&mut self, len: u64, ptr: u64) -> Result<()> {
        self.logic.log_utf8(len, ptr).map_err(Into::into)
    }

    pub fn log_utf16(&mut self, len: u64, ptr: u64) -> Result<()> {
        self.logic.log_utf16(len, ptr).map_err(Into::into)
    }

    pub fn abort(&mut self, msg_ptr: u32, filename_ptr: u32, line: u32, col: u32) -> Result<()> {
        self.logic
            .abort(msg_ptr, filename_ptr, line, col)
            .map_err(Into::into)
    }

    pub fn panic_utf8(&mut self, len: u64, ptr: u64) -> Result<()> {
        self.logic.panic_utf8(len, ptr).map_err(Into::into)
    }

    pub fn panic(&mut self) -> Result<()> {
        self.logic.panic().map_err(Into::into)
    }

    pub fn storage_write(
        &mut self,
        key_len: u64,
        key_ptr: u64,
        value_len: u64,
        value_ptr: u64,
        register_id: u64,
    ) -> Result<u64> {
        self.logic
            .storage_write(key_len, key_ptr, value_len, value_ptr, register_id)
            .map_err(Into::into)
    }

    pub fn storage_read(&mut self, key_len: u64, key_ptr: u64, register_id: u64) -> Result<u64> {
        self.logic
            .storage_read(key_len, key_ptr, register_id)
            .map_err(Into::into)
    }

    pub fn storage_remove(&mut self, key_len: u64, key_ptr: u64, register_id: u64) -> Result<u64> {
        self.logic
            .storage_remove(key_len, key_ptr, register_id)
            .map_err(Into::into)
    }

    pub fn storage_has_key(&mut self, key_len: u64, key_ptr: u64) -> Result<u64> {
        self.logic
            .storage_has_key(key_len, key_ptr)
            .map_err(Into::into)
    }

    pub fn storage_iter_prefix(&mut self, prefix_len: u64, prefix_ptr: u64) -> Result<u64> {
        self.logic
            .storage_iter_prefix(prefix_len, prefix_ptr)
            .map_err(Into::into)
    }

    pub fn storage_iter_range(
        &mut self,
        start_len: u64,
        start_ptr: u64,
        end_len: u64,
        end_ptr: u64,
    ) -> Result<u64> {
        self.logic
            .storage_iter_range(start_len, start_ptr, end_len, end_ptr)
            .map_err(Into::into)
    }

    pub fn storage_iter_next(
        &mut self,
        iterator_id: u64,
        key_register_id: u64,
        value_register_id: u64,
    ) -> Result<u64> {
        self.logic
            .storage_iter_next(iterator_id, key_register_id, value_register_id)
            .map_err(Into::into)
    }
}

impl logic::MemoryLike for js_sys::WebAssembly::Memory {
    fn fits_memory(&self, slice: logic::MemSlice) -> std::result::Result<(), ()> {
        let buffer = self.buffer().dyn_into::<ArrayBuffer>().unwrap();
        let bytes = buffer.byte_length();
        if slice.ptr.saturating_add(slice.len) >= u64::from(bytes) {
            return Err(());
        } else {
            return Ok(());
        }
    }

    fn view_memory(
        &self,
        slice: logic::MemSlice,
    ) -> std::result::Result<std::borrow::Cow<[u8]>, ()> {
        let mut out = vec![0; usize::try_from(slice.len).map_err(|_| ())?];
        self.read_memory(slice.ptr, &mut out)?;
        Ok(std::borrow::Cow::Owned(out))
    }

    fn read_memory(&self, offset: u64, buffer: &mut [u8]) -> std::result::Result<(), ()> {
        let array = js_sys::Uint8Array::new_with_byte_offset_and_length(
            &self.buffer(),
            u32::try_from(offset).map_err(|_| ())?,
            u32::try_from(buffer.len()).map_err(|_| ())?,
        );
        array.copy_to(buffer);
        Ok(())
    }

    fn write_memory(&mut self, offset: u64, buffer: &[u8]) -> std::result::Result<(), ()> {
        let array = js_sys::Uint8Array::new_with_byte_offset_and_length(
            &self.buffer(),
            u32::try_from(offset).map_err(|_| ())?,
            u32::try_from(buffer.len()).map_err(|_| ())?,
        );
        array.copy_from(buffer);
        Ok(())
    }
}
