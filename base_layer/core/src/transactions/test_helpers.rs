// Copyright 2019. The Tari Project
//
// Redistribution and use in source and binary forms, with or without modification, are permitted provided that the
// following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following
// disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the
// following disclaimer in the documentation and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote
// products derived from this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
// WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use std::sync::Arc;

use rand::rngs::OsRng;
use tari_common::configuration::Network;
use tari_common_types::types::{Commitment, PrivateKey, PublicKey, Signature};
use tari_crypto::keys::{PublicKey as PK, SecretKey};
use tari_key_manager::key_manager_service::KeyManagerInterface;
use tari_script::{inputs, script, ExecutionStack, TariScript};

use super::transaction_components::{TransactionInputVersion, TransactionOutputVersion};
use crate::{
    borsh::SerializedSize,
    consensus::ConsensusManager,
    core_key_manager::{BaseLayerKeyManagerInterface, CoreKeyManagerBranch, TariKeyId, TxoType},
    covenants::Covenant,
    test_helpers::{create_test_core_key_manager_with_memory_db, TestKeyManager},
    transactions::{
        crypto_factories::CryptoFactories,
        fee::Fee,
        tari_amount::MicroTari,
        transaction_components::{
            KernelBuilder,
            KernelFeatures,
            KeyManagerOutput,
            KeyManagerOutputBuilder,
            OutputFeatures,
            RangeProofType,
            Transaction,
            TransactionKernel,
            TransactionKernelVersion,
            TransactionOutput,
        },
        transaction_protocol::{TransactionMetadata, TransactionProtocolError},
        weight::TransactionWeight,
        SenderTransactionProtocol,
    },
};

pub async fn create_test_input(amount: MicroTari, maturity: u64, key_manager: &TestKeyManager) -> KeyManagerOutput {
    let params = TestParams::new(key_manager).await;
    params
        .create_input(
            UtxoTestParams {
                value: amount,
                features: OutputFeatures {
                    maturity,
                    ..Default::default()
                },
                ..Default::default()
            },
            key_manager,
        )
        .await
}

#[derive(Clone)]
pub struct TestParams {
    pub spend_key: TariKeyId,
    pub kernel_nonce: TariKeyId,
    pub change_spend_key: TariKeyId,
    pub script_private_key: TariKeyId,
    pub sender_offset_private_key: TariKeyId,
    pub ephemeral_public_nonce: TariKeyId,
    pub transaction_weight: TransactionWeight,
}

impl TestParams {
    pub async fn new(key_manager: &TestKeyManager) -> TestParams {
        let (spend_key, _) = key_manager
            .get_next_key_id(CoreKeyManagerBranch::CommitmentMask.get_branch_key())
            .await
            .unwrap();
        let (change_spend_key, _) = key_manager
            .get_next_key_id(CoreKeyManagerBranch::CommitmentMask.get_branch_key())
            .await
            .unwrap();
        let (script_private_key, _) = key_manager
            .get_next_key_id(CoreKeyManagerBranch::ScriptKey.get_branch_key())
            .await
            .unwrap();
        let (sender_offset_private_key, _) = key_manager
            .get_next_key_id(CoreKeyManagerBranch::Nonce.get_branch_key())
            .await
            .unwrap();
        let (kernel_nonce, _) = key_manager
            .get_next_key_id(CoreKeyManagerBranch::Nonce.get_branch_key())
            .await
            .unwrap();
        let (ephemeral_public_nonce, _) = key_manager
            .get_next_key_id(CoreKeyManagerBranch::Nonce.get_branch_key())
            .await
            .unwrap();

        Self {
            spend_key,
            change_spend_key,
            script_private_key,
            sender_offset_private_key,
            kernel_nonce,
            ephemeral_public_nonce,
            transaction_weight: TransactionWeight::v1(),
        }
    }

    pub fn fee(&self) -> Fee {
        Fee::new(self.transaction_weight)
    }

    pub async fn create_output(
        &self,
        params: UtxoTestParams,
        key_manager: &TestKeyManager,
    ) -> Result<KeyManagerOutput, String> {
        let version = match params.output_version {
            Some(v) => v,
            None => TransactionOutputVersion::get_current_version(),
        };
        let script_public_key = key_manager
            .get_public_key_at_key_id(&self.script_private_key)
            .await
            .unwrap();
        let input_data = params.input_data.unwrap_or_else(|| inputs!(script_public_key));
        let sender_offset_public_key = key_manager
            .get_public_key_at_key_id(&self.sender_offset_private_key)
            .await
            .unwrap();

        let output = KeyManagerOutputBuilder::new(params.value, self.spend_key.clone())
            .with_features(params.features)
            .with_script(params.script.clone())
            .encrypt_data_for_recovery(key_manager, None)
            .await
            .unwrap()
            .with_input_data(input_data)
            .with_covenant(params.covenant)
            .with_version(version)
            .with_sender_offset_public_key(sender_offset_public_key)
            .with_script_private_key(self.script_private_key.clone())
            .with_minimum_value_promise(params.minimum_value_promise)
            .sign_as_sender_and_receiver_using_key_id(key_manager, &self.sender_offset_private_key)
            .await
            .unwrap()
            .try_build()
            .unwrap();

        Ok(output)
    }

    /// Create a random transaction input for the given amount and maturity period. The input's unblinded
    /// parameters are returned.
    pub async fn create_input(&self, params: UtxoTestParams, key_manager: &TestKeyManager) -> KeyManagerOutput {
        self.create_output(params, key_manager).await.unwrap()
    }

    pub fn get_size_for_default_features_and_scripts(&self, num_outputs: usize) -> usize {
        let output_features = OutputFeatures { ..Default::default() };
        self.fee().weighting().round_up_features_and_scripts_size(
            script![Nop].get_serialized_size() + output_features.get_serialized_size(),
        ) * num_outputs
    }
}

#[derive(Clone)]
pub struct UtxoTestParams {
    pub value: MicroTari,
    pub script: TariScript,
    pub features: OutputFeatures,
    pub input_data: Option<ExecutionStack>,
    pub covenant: Covenant,
    pub output_version: Option<TransactionOutputVersion>,
    pub minimum_value_promise: MicroTari,
}

impl UtxoTestParams {
    pub fn with_value(value: MicroTari) -> Self {
        Self {
            value,
            ..Default::default()
        }
    }
}

impl Default for UtxoTestParams {
    fn default() -> Self {
        Self {
            value: 10.into(),
            script: script![Nop],
            features: OutputFeatures::default(),
            input_data: None,
            covenant: Covenant::default(),
            output_version: None,
            minimum_value_promise: MicroTari::zero(),
        }
    }
}

/// A convenience struct for a set of public-private keys and a public-private nonce
pub struct TestKeySet {
    pub k: PrivateKey,
    pub pk: PublicKey,
    pub r: PrivateKey,
    pub pr: PublicKey,
}

/// Generate a new random key set. The key set includes
/// * a public-private keypair (k, pk)
/// * a public-private nonce keypair (r, pr)
pub fn generate_keys() -> TestKeySet {
    let _rng = rand::thread_rng();
    let (k, pk) = PublicKey::random_keypair(&mut OsRng);
    let (r, pr) = PublicKey::random_keypair(&mut OsRng);
    TestKeySet { k, pk, r, pr }
}

/// Generate a random transaction signature, returning the public key (excess) and the signature.
pub fn create_random_signature(fee: MicroTari, lock_height: u64, features: KernelFeatures) -> (PublicKey, Signature) {
    let (k, p) = PublicKey::random_keypair(&mut OsRng);
    (p, create_signature(k, fee, lock_height, features))
}

/// Generate a random transaction signature, returning the public key (excess) and the signature.
pub fn create_signature(k: PrivateKey, fee: MicroTari, lock_height: u64, features: KernelFeatures) -> Signature {
    let r = PrivateKey::random(&mut OsRng);
    let tx_meta = TransactionMetadata::new_with_features(fee, lock_height, features);
    let e = TransactionKernel::build_kernel_challenge_from_tx_meta(
        &TransactionKernelVersion::get_current_version(),
        &PublicKey::from_secret_key(&r),
        &PublicKey::from_secret_key(&k),
        &tx_meta,
    );
    Signature::sign_raw(&k, r, &e).unwrap()
}

/// Generate a random transaction signature given a key, returning the public key (excess) and the signature.
pub async fn create_random_signature_from_secret_key(
    key_manager: &TestKeyManager,
    secret_key_id: TariKeyId,
    fee: MicroTari,
    lock_height: u64,
    kernel_features: KernelFeatures,
    txo_type: TxoType,
) -> (PublicKey, Signature) {
    let tx_meta = TransactionMetadata::new_with_features(fee, lock_height, kernel_features);
    let (nonce_id, total_nonce) = key_manager
        .get_next_key_id(CoreKeyManagerBranch::Nonce.get_branch_key())
        .await
        .unwrap();
    let total_excess = key_manager.get_public_key_at_key_id(&secret_key_id).await.unwrap();
    let kernel_version = TransactionKernelVersion::get_current_version();
    let kernel_message = TransactionKernel::build_kernel_signature_message(
        &kernel_version,
        tx_meta.fee,
        tx_meta.lock_height,
        &tx_meta.kernel_features,
        &tx_meta.burn_commitment,
    );
    let kernel_signature = key_manager
        .get_txo_kernel_signature(
            &secret_key_id,
            &nonce_id,
            &total_nonce,
            &total_excess,
            &kernel_version,
            &kernel_message,
            &kernel_features,
            txo_type,
        )
        .await
        .unwrap();
    (total_excess, kernel_signature)
}

pub fn create_consensus_manager() -> ConsensusManager {
    ConsensusManager::builder(Network::LocalNet).build()
}

pub async fn create_key_manager_coinbase(
    test_params: &TestParams,
    height: u64,
    extra: Option<Vec<u8>>,
) -> KeyManagerOutput {
    let rules = create_consensus_manager();
    let key_manager = create_test_core_key_manager_with_memory_db();
    let constants = rules.consensus_constants(height);
    test_params
        .create_output(
            UtxoTestParams {
                value: rules.get_block_reward_at(height),
                features: OutputFeatures::create_coinbase(height + constants.coinbase_lock_height(), extra),
                ..Default::default()
            },
            &key_manager,
        )
        .await
        .unwrap()
}

pub async fn create_key_manager_output_with_data(
    script: TariScript,
    output_features: OutputFeatures,
    test_params: &TestParams,
    value: MicroTari,
    key_manager: &TestKeyManager,
) -> Result<KeyManagerOutput, String> {
    test_params
        .create_output(
            UtxoTestParams {
                value,
                script,
                features: output_features,
                ..Default::default()
            },
            key_manager,
        )
        .await
}

/// The tx macro is a convenience wrapper around the [create_tx] function, making the arguments optional and explicit
/// via keywords.
#[macro_export]
macro_rules! tx {
  ($amount:expr, fee: $fee:expr, lock: $lock:expr, inputs: $n_in:expr, maturity: $mat:expr, outputs: $n_out:expr, features: $features:expr, $key_manager:expr) => {{
      use $crate::transactions::test_helpers::create_tx;
      create_tx($amount, $fee, $lock, $n_in, $mat, $n_out, $features, $key_manager).await
  }};
  ($amount:expr, fee: $fee:expr, lock: $lock:expr, inputs: $n_in:expr, maturity: $mat:expr, outputs: $n_out:expr, $key_manager:expr) => {{
    tx!($amount, fee: $fee, lock: $lock, inputs: $n_in, maturity: $mat, outputs: $n_out, features: Default::default(), $key_manager)
  }};

  ($amount:expr, fee: $fee:expr, lock: $lock:expr, inputs: $n_in:expr, outputs: $n_out:expr, $key_manager:expr) => {
    tx!($amount, fee: $fee, lock: $lock, inputs: $n_in, maturity: 0, outputs: $n_out, $key_manager)
  };

  ($amount:expr, fee: $fee:expr, inputs: $n_in:expr, outputs: $n_out:expr, features: $features:expr, $key_manager:expr) => {
    tx!($amount, fee: $fee, lock: 0, inputs: $n_in, maturity: 0, outputs: $n_out, features: $features, $key_manager)
  };

  ($amount:expr, fee: $fee:expr, inputs: $n_in:expr, outputs: $n_out:expr, $key_manager:expr) => {
    tx!($amount, fee: $fee, lock: 0, inputs: $n_in, maturity: 0, outputs: $n_out, $key_manager)
  };

  ($amount:expr, fee: $fee:expr, $key_manager:expr) => {
    tx!($amount, fee: $fee, lock: 0, inputs: 1, maturity: 0, outputs: 2, $key_manager)
  }
}

/// A utility macro to help make it easy to build transactions.
///
/// The full syntax allows maximum flexibility, but most arguments are optional with sane defaults
/// ```ignore
///   txn_schema!(from: inputs, to: outputs, fee: 50*uT, lock: 1250,
///     features: OutputFeatures { maturity: 1320, ..Default::default() },
///     input_version: TransactioInputVersion::get_current_version(),
///     output_version: TransactionOutputVersion::get_current_version()
///   );
///   txn_schema!(from: inputs, to: outputs, fee: 50*uT); // Uses default features, default versions and zero lock height
///   txn_schema!(from: inputs, to: outputs); // min fee of 25µT, zero lock height, default features and default versions
///   // as above, and transaction splits the first input in roughly half, returning remainder as change
///   txn_schema!(from: inputs);
/// ```
/// The output of this macro is intended to be used in [spend_utxos].
#[macro_export]
macro_rules! txn_schema {
    (from: $input:expr, to: $outputs:expr, fee: $fee:expr, lock: $lock:expr, features: $features:expr, input_version: $input_version:expr, output_version: $output_version:expr) => {{
        $crate::transactions::test_helpers::TransactionSchema {
            from: $input.clone(),
            to: $outputs.clone(),
            to_outputs: vec![],
            fee: $fee,
            lock_height: $lock,
            features: $features.clone(),
            script: tari_script::script![Nop],
            covenant: Default::default(),
            input_data: None,
            input_version: $input_version.clone(),
            output_version: $output_version.clone()
        }
    }};

    (from: $input:expr, to: $outputs:expr, fee: $fee:expr, lock: $lock:expr, features: $features:expr) => {{
        txn_schema!(
            from: $input,
            to:$outputs,
            fee:$fee,
            lock:$lock,
            features: $features.clone(),
            input_version: None,
            output_version: None
        )
    }};

    (from: $input:expr, to: $outputs:expr, features: $features:expr) => {{
        txn_schema!(
            from: $input,
            to:$outputs,
            fee: 5.into(),
            lock: 0,
            features: $features,
            input_version: None,
            output_version: None
        )
    }};

    (from: $input:expr, to: $outputs:expr, fee: $fee:expr) => {
        txn_schema!(
            from: $input,
            to:$outputs,
            fee:$fee,
            lock:0,
            features: $crate::transactions::transaction_components::OutputFeatures::default(),
            input_version: None,
            output_version: None
        )
    };

    (from: $input:expr, to: $outputs:expr) => {
        txn_schema!(from: $input, to:$outputs, fee: 5.into())
    };

    (from: $input:expr, to: $outputs:expr, input_version: $input_version:expr, output_version: $output_version:expr) => {
        txn_schema!(
            from: $input,
            to:$outputs,
            fee: 5.into(),
            lock:0,
            features: $crate::transactions::transaction_components::OutputFeatures::default(),
            input_version: Some($input_version),
            output_version: Some($output_version)
        )
    };

    // Spend inputs to ± half the first input value, with default fee and lock height
    (from: $input:expr) => {{
        let out_val = $input[0].value / 2u64;
        txn_schema!(from: $input, to: vec![out_val])
    }};
}

/// A convenience struct that holds plaintext versions of transactions
#[derive(Clone, Debug)]
pub struct TransactionSchema {
    pub from: Vec<KeyManagerOutput>,
    pub to: Vec<MicroTari>,
    pub to_outputs: Vec<KeyManagerOutput>,
    pub fee: MicroTari,
    pub lock_height: u64,
    pub features: OutputFeatures,
    pub script: TariScript,
    pub input_data: Option<ExecutionStack>,
    pub covenant: Covenant,
    pub input_version: Option<TransactionInputVersion>,
    pub output_version: Option<TransactionOutputVersion>,
}

/// Create an unconfirmed transaction for testing with a valid fee, unique access_sig, random inputs and outputs, the
/// transaction is only partially constructed
pub async fn create_tx(
    amount: MicroTari,
    fee_per_gram: MicroTari,
    lock_height: u64,
    input_count: usize,
    input_maturity: u64,
    output_count: usize,
    output_features: OutputFeatures,
    key_manager: &TestKeyManager,
) -> (Transaction, Vec<KeyManagerOutput>, Vec<KeyManagerOutput>) {
    let (inputs, outputs) = create_key_manager_txos(
        amount,
        input_count,
        input_maturity,
        output_count,
        fee_per_gram,
        &output_features,
        &script![Nop],
        &Default::default(),
        key_manager,
    )
    .await;
    let tx = create_transaction_with(lock_height, fee_per_gram, inputs.clone(), outputs.clone(), key_manager).await;
    (tx, inputs, outputs.into_iter().map(|(utxo, _)| utxo).collect())
}

pub async fn create_key_manager_txos(
    amount: MicroTari,
    input_count: usize,
    input_maturity: u64,
    output_count: usize,
    fee_per_gram: MicroTari,
    output_features: &OutputFeatures,
    output_script: &TariScript,
    output_covenant: &Covenant,
    key_manager: &TestKeyManager,
) -> (Vec<KeyManagerOutput>, Vec<(KeyManagerOutput, TariKeyId)>) {
    let weighting = TransactionWeight::latest();
    // This is a best guess to not underestimate metadata size
    let output_features_and_scripts_size = weighting.round_up_features_and_scripts_size(
        output_features.get_serialized_size() +
            output_script.get_serialized_size() +
            output_covenant.get_serialized_size(),
    ) * output_count;
    let estimated_fee = Fee::new(weighting).calculate(
        fee_per_gram,
        1,
        input_count,
        output_count,
        output_features_and_scripts_size,
    );
    let amount_per_output = (amount - estimated_fee) / output_count as u64;
    let amount_for_last_output = (amount - estimated_fee) - amount_per_output * (output_count as u64 - 1);

    let mut outputs = Vec::new();
    for i in 0..output_count {
        let output_amount = if i < output_count - 1 {
            amount_per_output
        } else {
            amount_for_last_output
        };
        let test_params = TestParams::new(key_manager).await;
        let script_offset_pvt_key = test_params.sender_offset_private_key.clone();

        let output = test_params
            .create_output(
                UtxoTestParams {
                    value: output_amount,
                    covenant: output_covenant.clone(),
                    script: output_script.clone(),
                    features: output_features.clone(),
                    ..Default::default()
                },
                key_manager,
            )
            .await
            .unwrap();
        outputs.push((output, script_offset_pvt_key));
    }

    let amount_per_input = amount / input_count as u64;
    let mut inputs = Vec::new();
    for i in 0..input_count {
        let mut params = UtxoTestParams {
            features: OutputFeatures {
                maturity: input_maturity,
                ..OutputFeatures::default()
            },
            ..Default::default()
        };
        if i == input_count - 1 {
            params.value = amount - amount_per_input * (input_count as u64 - 1);
        } else {
            params.value = amount_per_input;
        }

        let key_manager_output = TestParams::new(key_manager)
            .await
            .create_input(params, key_manager)
            .await;
        inputs.push(key_manager_output);
    }

    (inputs, outputs)
}
/// Create an unconfirmed transaction for testing with a valid fee, unique excess_sig, random inputs and outputs, the
/// transaction is only partially constructed
pub async fn create_transaction_with(
    lock_height: u64,
    fee_per_gram: MicroTari,
    inputs: Vec<KeyManagerOutput>,
    outputs: Vec<(KeyManagerOutput, TariKeyId)>,
    key_manager: &TestKeyManager,
) -> Transaction {
    let stx_protocol = create_sender_transaction_protocol_with(lock_height, fee_per_gram, inputs, outputs, key_manager)
        .await
        .unwrap();
    stx_protocol.take_transaction().unwrap()
}

pub async fn create_sender_transaction_protocol_with(
    lock_height: u64,
    fee_per_gram: MicroTari,
    inputs: Vec<KeyManagerOutput>,
    outputs: Vec<(KeyManagerOutput, TariKeyId)>,
    key_manager: &TestKeyManager,
) -> Result<SenderTransactionProtocol, TransactionProtocolError> {
    let rules = ConsensusManager::builder(Network::LocalNet).build();
    let constants = rules.consensus_constants(0).clone();
    let mut stx_builder = SenderTransactionProtocol::builder(constants, key_manager.clone());
    let script = script!(Nop);
    let (change_script_key_id, _) = key_manager
        .get_next_key_id(CoreKeyManagerBranch::ScriptKey.get_branch_key())
        .await
        .unwrap();
    let (change_secret_key_id, change_public_key) = key_manager
        .get_next_key_id(CoreKeyManagerBranch::CommitmentMask.get_branch_key())
        .await
        .unwrap();
    let (change_sender_offset_key_id, _) = key_manager
        .get_next_key_id(CoreKeyManagerBranch::Nonce.get_branch_key())
        .await
        .unwrap();
    let change_covenant = Covenant::default();

    let change_input_data = inputs!(change_public_key);
    stx_builder
        .with_lock_height(lock_height)
        .with_fee_per_gram(fee_per_gram)
        .with_kernel_features(KernelFeatures::empty())
        .with_change_data(
            script,
            change_input_data,
            change_script_key_id,
            change_secret_key_id,
            change_sender_offset_key_id,
            change_covenant,
        );
    for input in inputs {
        stx_builder.with_input(input).await.unwrap();
    }

    for (output, script_offset_key_id) in outputs {
        stx_builder.with_output(output, script_offset_key_id).await.unwrap();
    }

    let mut stx_protocol = stx_builder.build().await.unwrap();
    stx_protocol.finalize(key_manager).await?;

    Ok(stx_protocol)
}

/// Spend the provided UTXOs to the given amounts. Change will be created with any outstanding amount.
/// You only need to provide the unblinded outputs to spend. This function will calculate the commitment for you.
/// This is obviously less efficient, but is offered as a convenience.
/// The output features will be applied to every output
pub async fn spend_utxos(
    schema: TransactionSchema,
    key_manager: &TestKeyManager,
) -> (Transaction, Vec<KeyManagerOutput>) {
    let (mut stx_protocol, outputs) = create_stx_protocol(schema, key_manager).await;
    stx_protocol.finalize(key_manager).await.unwrap();
    let txn = stx_protocol.get_transaction().unwrap().clone();
    (txn, outputs)
}

#[allow(clippy::too_many_lines)]
pub async fn create_stx_protocol(
    schema: TransactionSchema,
    key_manager: &TestKeyManager,
) -> (SenderTransactionProtocol, Vec<KeyManagerOutput>) {
    let constants = ConsensusManager::builder(Network::LocalNet)
        .build()
        .consensus_constants(0)
        .clone();
    let mut stx_builder = SenderTransactionProtocol::builder(constants, key_manager.clone());
    let script = script!(Nop);
    let (change_script_key_id, change_public_key) = key_manager
        .get_next_key_id(CoreKeyManagerBranch::ScriptKey.get_branch_key())
        .await
        .unwrap();
    let (change_secret_key_id, _) = key_manager
        .get_next_key_id(CoreKeyManagerBranch::CommitmentMask.get_branch_key())
        .await
        .unwrap();
    let (change_sender_offset_key_id, _) = key_manager
        .get_next_key_id(CoreKeyManagerBranch::Nonce.get_branch_key())
        .await
        .unwrap();
    let change_covenant = Covenant::default();
    let change_input_data = inputs!(change_public_key);

    stx_builder
        .with_lock_height(schema.lock_height)
        .with_fee_per_gram(schema.fee)
        .with_change_data(
            script,
            change_input_data,
            change_script_key_id,
            change_secret_key_id,
            change_sender_offset_key_id,
            change_covenant,
        );

    for tx_input in &schema.from {
        stx_builder.with_input(tx_input.clone()).await.unwrap();
    }
    let mut outputs = Vec::with_capacity(schema.to.len());
    for val in schema.to {
        let (spending_key, _) = key_manager
            .get_next_key_id(CoreKeyManagerBranch::CommitmentMask.get_branch_key())
            .await
            .unwrap();
        let (sender_offset_key_id, sender_offset_public_key) = key_manager
            .get_next_key_id(CoreKeyManagerBranch::Nonce.get_branch_key())
            .await
            .unwrap();
        let (script_key_id, _) = key_manager
            .get_next_key_id(CoreKeyManagerBranch::ScriptKey.get_branch_key())
            .await
            .unwrap();
        let script_public_key = key_manager.get_public_key_at_key_id(&script_key_id).await.unwrap();
        let input_data = match &schema.input_data {
            Some(data) => data.clone(),
            None => inputs!(script_public_key),
        };
        let version = match schema.output_version {
            Some(data) => data.clone(),
            None => TransactionOutputVersion::get_current_version(),
        };
        let output = KeyManagerOutputBuilder::new(val, spending_key)
            .with_features(schema.features.clone())
            .with_script(schema.script.clone())
            .encrypt_data_for_recovery(key_manager, None)
            .await
            .unwrap()
            .with_input_data(input_data)
            .with_covenant(schema.covenant.clone())
            .with_version(version)
            .with_sender_offset_public_key(sender_offset_public_key)
            .with_script_private_key(script_key_id.clone())
            .sign_as_sender_and_receiver_using_key_id(key_manager, &sender_offset_key_id)
            .await
            .unwrap()
            .try_build()
            .unwrap();

        outputs.push(output.clone());
        stx_builder.with_output(output, sender_offset_key_id).await.unwrap();
    }
    for mut utxo in schema.to_outputs {
        let (sender_offset_key_id, _) = key_manager
            .get_next_key_id(CoreKeyManagerBranch::Nonce.get_branch_key())
            .await
            .unwrap();
        let metadata_message = TransactionOutput::metadata_signature_message(&utxo);
        utxo.metadata_signature = key_manager
            .get_metadata_signature(
                &utxo.spending_key_id,
                &utxo.value.into(),
                &sender_offset_key_id,
                &utxo.version,
                &metadata_message,
                utxo.features.range_proof_type,
            )
            .await
            .unwrap();

        stx_builder.with_output(utxo, sender_offset_key_id).await.unwrap();
    }

    let stx_protocol = stx_builder.build().await.unwrap();
    let change_output = stx_protocol.get_change_output().unwrap().unwrap();

    outputs.push(change_output);
    (stx_protocol, outputs)
}

pub async fn create_coinbase_kernel(spending_key_id: &TariKeyId, key_manager: &TestKeyManager) -> TransactionKernel {
    let kernel_version = TransactionKernelVersion::get_current_version();
    let kernel_features = KernelFeatures::COINBASE_KERNEL;
    let kernel_message =
        TransactionKernel::build_kernel_signature_message(&kernel_version, 0.into(), 0, &kernel_features, &None);
    let (public_nonce_id, public_nonce) = key_manager
        .get_next_key_id(CoreKeyManagerBranch::Nonce.get_branch_key())
        .await
        .unwrap();
    let public_spend_key = key_manager.get_public_key_at_key_id(&spending_key_id).await.unwrap();

    let kernel_signature = key_manager
        .get_txo_kernel_signature(
            &spending_key_id,
            &public_nonce_id,
            &public_nonce,
            &public_spend_key,
            &kernel_version,
            &kernel_message,
            &kernel_features,
            TxoType::Output,
        )
        .await
        .unwrap();

    KernelBuilder::new()
        .with_features(kernel_features)
        .with_excess(&Commitment::from_public_key(&public_spend_key))
        .with_signature(kernel_signature)
        .build()
        .unwrap()
}

/// Create a transaction kernel with the given fee, using random keys to generate the signature
pub fn create_test_kernel(fee: MicroTari, lock_height: u64, features: KernelFeatures) -> TransactionKernel {
    let (excess, s) = create_random_signature(fee, lock_height, features);
    KernelBuilder::new()
        .with_fee(fee)
        .with_lock_height(lock_height)
        .with_features(features)
        .with_excess(&Commitment::from_public_key(&excess))
        .with_signature(s)
        .build()
        .unwrap()
}

/// Create a new UTXO for the specified value and return the output and spending key
pub async fn create_utxo(
    value: MicroTari,
    key_manager: &TestKeyManager,
    features: &OutputFeatures,
    script: &TariScript,
    covenant: &Covenant,
    minimum_value_promise: MicroTari,
) -> (TransactionOutput, TariKeyId, TariKeyId) {
    let (spending_key_id, _) = key_manager
        .get_next_key_id(CoreKeyManagerBranch::CommitmentMask.get_branch_key())
        .await
        .unwrap();
    let encrypted_data = key_manager
        .encrypt_data_for_recovery(&spending_key_id, None, value.into())
        .await
        .unwrap();
    let (sender_offset_key_id, _) = key_manager
        .get_next_key_id(CoreKeyManagerBranch::Nonce.get_branch_key())
        .await
        .unwrap();
    let metadata_message = TransactionOutput::metadata_signature_message_from_parts(
        &TransactionOutputVersion::get_current_version(),
        script,
        features,
        covenant,
        &encrypted_data,
        minimum_value_promise,
    );
    let metadata_sig = key_manager
        .get_metadata_signature(
            &spending_key_id,
            &value.into(),
            &sender_offset_key_id,
            &TransactionOutputVersion::get_current_version(),
            &metadata_message,
            features.range_proof_type,
        )
        .await
        .unwrap();
    let commitment = key_manager
        .get_commitment(&spending_key_id, &value.into())
        .await
        .unwrap();
    let proof = if features.range_proof_type == RangeProofType::BulletProofPlus {
        Some(
            key_manager
                .construct_range_proof(&spending_key_id, value.into(), minimum_value_promise.into())
                .await
                .unwrap(),
        )
    } else {
        None
    };

    let sender_offset_public_key = key_manager
        .get_public_key_at_key_id(&sender_offset_key_id)
        .await
        .unwrap();
    let utxo = TransactionOutput::new_current_version(
        features.clone(),
        commitment,
        proof,
        script.clone(),
        sender_offset_public_key,
        metadata_sig,
        covenant.clone(),
        encrypted_data,
        minimum_value_promise,
    );
    utxo.verify_range_proof(&CryptoFactories::default().range_proof)
        .unwrap();
    (utxo, spending_key_id, sender_offset_key_id)
}

pub async fn schema_to_transaction(
    txns: &[TransactionSchema],
    key_manager: &TestKeyManager,
) -> (Vec<Arc<Transaction>>, Vec<KeyManagerOutput>) {
    let mut txs = Vec::new();
    let mut utxos = Vec::new();
    for schema in txns {
        let (txn, mut output) = spend_utxos(schema.clone(), key_manager).await;
        txs.push(Arc::new(txn));
        utxos.append(&mut output);
    }

    (txs, utxos)
}
