//  Copyright 2023, The Tari Project
//
//  Redistribution and use in source and binary forms, with or without modification, are permitted provided that the
//  following conditions are met:
//
//  1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following
//  disclaimer.
//
//  2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the
//  following disclaimer in the documentation and/or other materials provided with the distribution.
//
//  3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote
//  products derived from this software without specific prior written permission.
//
//  THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
//  INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
//  DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
//  SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//  SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
//  WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
//  USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use std::sync::Arc;

use tari_common_types::types::{
    ComAndPubSignature,
    Commitment,
    PrivateKey,
    PublicKey,
    RangeProof,
    RangeProofService,
    Signature,
};
use tari_comms::types::CommsDHKE;
use tari_key_manager::{
    cipher_seed::CipherSeed,
    key_manager_service::{
        storage::database::{KeyManagerBackend, KeyManagerDatabase},
        AddResult,
        KeyId,
        KeyManagerInterface,
        KeyManagerServiceError,
        NextKeyResult,
    },
};
use tokio::sync::RwLock;

use crate::{
    core_key_manager::{BaseLayerKeyManagerInterface, CoreKeyManagerBranch, CoreKeyManagerInner},
    transactions::{
        tari_amount::MicroTari,
        transaction_components::{
            EncryptedData,
            RangeProofType,
            TransactionError,
            TransactionInputVersion,
            TransactionKernelVersion,
            TransactionOutputVersion,
        },
        CryptoFactories,
    },
};

/// The key manager provides a hierarchical key derivation function (KDF) that derives uniformly random secret keys from
/// a single seed key for arbitrary branches, using an implementation of `KeyManagerBackend` to store the current index
/// for each branch.
///
/// This handle can be cloned cheaply and safely shared across multiple threads.
#[derive(Clone)]
pub struct CoreKeyManagerHandle<TBackend> {
    core_key_manager_inner: Arc<RwLock<CoreKeyManagerInner<TBackend>>>,
}

impl<TBackend> CoreKeyManagerHandle<TBackend>
where TBackend: KeyManagerBackend<PublicKey> + 'static
{
    /// Creates a new key manager.
    /// * `master_seed` is the primary seed that will be used to derive all unique branch keys with their indexes
    /// * `db` implements `KeyManagerBackend` and is used for persistent storage of branches and indices.
    pub fn new(
        master_seed: CipherSeed,
        db: KeyManagerDatabase<TBackend, PublicKey>,
        crypto_factories: CryptoFactories,
    ) -> Result<Self, KeyManagerServiceError> {
        Ok(CoreKeyManagerHandle {
            core_key_manager_inner: Arc::new(RwLock::new(CoreKeyManagerInner::new(
                master_seed,
                db,
                crypto_factories,
            )?)),
        })
    }
}

#[async_trait::async_trait]
impl<TBackend> KeyManagerInterface<PublicKey> for CoreKeyManagerHandle<TBackend>
where TBackend: KeyManagerBackend<PublicKey> + 'static
{
    async fn add_new_branch<T: Into<String> + Send>(&self, branch: T) -> Result<AddResult, KeyManagerServiceError> {
        (*self.core_key_manager_inner)
            .write()
            .await
            .add_key_manager_branch(&branch.into())
    }

    async fn get_next_key<T: Into<String> + Send>(
        &self,
        branch: T,
    ) -> Result<NextKeyResult<PublicKey>, KeyManagerServiceError> {
        // unimplemented!(
        //     "Oops! `get_next_key` - we do not share private keys outside `core_key_manager`. ({})",
        //     branch.into(),
        // )
        // TODO: Remove this call - only here for legacy tests
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_next_key(&branch.into())
            .await
    }

    async fn get_next_key_id<T: Into<String> + Send>(
        &self,
        branch: T,
    ) -> Result<KeyId<PublicKey>, KeyManagerServiceError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_next_key_id(&branch.into())
            .await
    }

    async fn get_static_key_id<T: Into<String> + Send>(
        &self,
        branch: T,
    ) -> Result<KeyId<PublicKey>, KeyManagerServiceError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_static_key_id(&branch.into())
            .await
    }

    async fn get_key_at_index<T: Into<String> + Send>(
        &self,
        branch: T,
        index: u64,
    ) -> Result<PrivateKey, KeyManagerServiceError> {
        // unimplemented!(
        //     "Oops! `get_key_at_index` - we do not share private keys outside `core_key_manager`. ({}, {})",
        //     branch.into(),
        //     index
        // )
        // TODO: Remove this call - only here for legacy tests
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_key_at_index(&branch.into(), index)
            .await
    }

    async fn get_public_key_at_key_id(&self, key_id: &KeyId<PublicKey>) -> Result<PublicKey, KeyManagerServiceError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_public_key_at_key_id(key_id)
            .await
    }

    async fn find_key_index<T: Into<String> + Send>(
        &self,
        branch: T,
        key: &PublicKey,
    ) -> Result<u64, KeyManagerServiceError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .find_key_index(&branch.into(), key)
            .await
    }

    async fn update_current_key_index_if_higher<T: Into<String> + Send>(
        &self,
        branch: T,
        index: u64,
    ) -> Result<(), KeyManagerServiceError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .update_current_key_index_if_higher(&branch.into(), index)
            .await
    }

    async fn import_key(&self, private_key: PrivateKey) -> Result<KeyId<PublicKey>, KeyManagerServiceError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .import_key(private_key)
            .await
    }
}

#[async_trait::async_trait]
impl<TBackend> BaseLayerKeyManagerInterface for CoreKeyManagerHandle<TBackend>
where TBackend: KeyManagerBackend<PublicKey> + 'static
{
    async fn get_commitment(
        &self,
        spend_key_id: &KeyId<PublicKey>,
        value: &PrivateKey,
    ) -> Result<Commitment, KeyManagerServiceError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_commitment(spend_key_id, value)
            .await
    }

    async fn verify_mask(
        &self,
        prover: &RangeProofService,
        commitment: &Commitment,
        spending_key_id: &KeyId<PublicKey>,
        value: u64,
    ) -> Result<bool, KeyManagerServiceError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .verify_mask(prover, commitment, spending_key_id, value)
            .await
    }

    async fn get_recovery_key_id(&self) -> Result<KeyId<PublicKey>, KeyManagerServiceError> {
        self.get_static_key_id(CoreKeyManagerBranch::DataEncryption.get_branch_key())
            .await
    }

    async fn get_next_spend_and_script_key_ids(
        &self,
    ) -> Result<(KeyId<PublicKey>, KeyId<PublicKey>), KeyManagerServiceError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_next_spend_and_script_key_ids()
            .await
    }

    async fn get_diffie_hellman_shared_secret(
        &self,
        secret_key_id: &KeyId<PublicKey>,
        public_key: &PublicKey,
    ) -> Result<CommsDHKE, TransactionError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_diffie_hellman_shared_secret(secret_key_id, public_key)
            .await
    }

    async fn get_spending_key_id(&self, public_spending_key: &PublicKey) -> Result<KeyId<PublicKey>, TransactionError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_spending_key_id(public_spending_key)
            .await
    }

    async fn construct_range_proof(
        &self,
        spend_key_id: &KeyId<PublicKey>,
        value: u64,
        min_value: u64,
    ) -> Result<RangeProof, TransactionError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .construct_range_proof(spend_key_id, value, min_value)
            .await
    }

    async fn get_script_signature(
        &self,
        script_key_id: &KeyId<PublicKey>,
        spend_key_id: &KeyId<PublicKey>,
        value: &PrivateKey,
        tx_version: &TransactionInputVersion,
        script_message: &[u8; 32],
    ) -> Result<ComAndPubSignature, TransactionError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_script_signature(script_key_id, spend_key_id, value, tx_version, script_message)
            .await
    }

    async fn get_partial_kernel_signature(
        &self,
        spend_key_id: &KeyId<PublicKey>,
        nonce_id: &KeyId<PublicKey>,
        total_nonce: &PublicKey,
        total_excess: &PublicKey,
        kernel_version: &TransactionKernelVersion,
        kernel_message: &[u8; 32],
    ) -> Result<Signature, TransactionError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_partial_kernel_signature(
                spend_key_id,
                nonce_id,
                total_nonce,
                total_excess,
                kernel_version,
                kernel_message,
            )
            .await
    }

    async fn get_partial_kernel_signature_excess(
        &self,
        spend_key_id: &KeyId<PublicKey>,
        nonce_id: &KeyId<PublicKey>,
    ) -> Result<PublicKey, TransactionError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_partial_kernel_signature_excess(spend_key_id, nonce_id)
            .await
    }

    async fn get_partial_private_kernel_offset(
        &self,
        spend_key_id: &KeyId<PublicKey>,
        nonce_id: &KeyId<PublicKey>,
    ) -> Result<PrivateKey, TransactionError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_partial_private_kernel_offset(spend_key_id, nonce_id)
            .await
    }

    async fn encrypt_data_for_recovery(
        &self,
        spend_key_id: &KeyId<PublicKey>,
        custom_recovery_key_id: &Option<KeyId<PublicKey>>,
        value: u64,
    ) -> Result<EncryptedData, TransactionError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .encrypt_data_for_recovery(spend_key_id, custom_recovery_key_id, value)
            .await
    }

    async fn try_commitment_key_recovery(
        &self,
        commitment: &Commitment,
        data: &EncryptedData,
        custom_recovery_key_id: &Option<KeyId<PublicKey>>,
    ) -> Result<(KeyId<PublicKey>, MicroTari), TransactionError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .try_commitment_key_recovery(commitment, data, custom_recovery_key_id)
            .await
    }

    async fn get_script_offset(
        &self,
        script_key_ids: &[KeyId<PublicKey>],
        sender_offset_key_ids: &[KeyId<PublicKey>],
    ) -> Result<PrivateKey, TransactionError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_script_offset(script_key_ids, sender_offset_key_ids)
            .await
    }

    async fn get_metadata_signature_ephemeral_commitment(
        &self,
        nonce_id: &KeyId<PublicKey>,
        range_proof_type: RangeProofType,
    ) -> Result<Commitment, TransactionError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_metadata_signature_ephemeral_commitment(nonce_id, range_proof_type)
            .await
    }

    async fn get_metadata_signature(
        &self,
        value_as_private_key: &PrivateKey,
        spending_key_id: &KeyId<PublicKey>,
        sender_offset_private_key: &PrivateKey,
        nonce_a: &PrivateKey,
        nonce_b: &PrivateKey,
        nonce_x: &PrivateKey,
        challenge_bytes: &[u8; 32],
    ) -> Result<ComAndPubSignature, TransactionError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_metadata_signature(
                value_as_private_key,
                spending_key_id,
                sender_offset_private_key,
                nonce_a,
                nonce_b,
                nonce_x,
                challenge_bytes,
            )
            .await
    }

    async fn get_receiver_partial_metadata_signature(
        &self,
        spend_key_id: &KeyId<PublicKey>,
        value: &PrivateKey,
        nonce_id: &KeyId<PublicKey>,
        sender_offset_public_key: &PublicKey,
        ephemeral_pubkey: &PublicKey,
        tx_version: &TransactionOutputVersion,
        metadata_signature_message: &[u8; 32],
        range_proof_type: RangeProofType,
    ) -> Result<ComAndPubSignature, TransactionError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_receiver_partial_metadata_signature(
                spend_key_id,
                value,
                nonce_id,
                sender_offset_public_key,
                ephemeral_pubkey,
                tx_version,
                metadata_signature_message,
                range_proof_type,
            )
            .await
    }

    async fn get_sender_partial_metadata_signature(
        &self,
        nonce_id: &KeyId<PublicKey>,
        sender_offset_key_id: &KeyId<PublicKey>,
        commitment: &Commitment,
        ephemeral_commitment: &Commitment,
        tx_version: &TransactionOutputVersion,
        metadata_signature_message: &[u8; 32],
    ) -> Result<ComAndPubSignature, TransactionError> {
        (*self.core_key_manager_inner)
            .read()
            .await
            .get_sender_partial_metadata_signature(
                nonce_id,
                sender_offset_key_id,
                commitment,
                ephemeral_commitment,
                tx_version,
                metadata_signature_message,
            )
            .await
    }
}
