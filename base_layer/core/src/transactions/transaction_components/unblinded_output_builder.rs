//  Copyright 2021. The Tari Project
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

use derivative::Derivative;
use tari_common_types::types::{ComAndPubSignature, PrivateKey, PublicKey};
use tari_crypto::commitment::HomomorphicCommitmentFactory;
use tari_script::{ExecutionStack, TariScript};

use crate::{
    covenants::Covenant,
    transactions::{
        tari_amount::MicroTari,
        transaction_components::{EncryptedData, OutputFeatures, TransactionError},
        transaction_protocol::RecoveryData,
        CryptoFactories,
    },
};

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct UnblindedOutputBuilder {
    value: MicroTari,
    #[derivative(Debug = "ignore")]
    spending_key: PrivateKey,
    features: OutputFeatures,
    script: Option<TariScript>,
    covenant: Covenant,
    input_data: Option<ExecutionStack>,
    #[derivative(Debug = "ignore")]
    script_private_key: Option<PrivateKey>,
    sender_offset_public_key: Option<PublicKey>,
    metadata_signature: Option<ComAndPubSignature>,
    metadata_signed_by_receiver: bool,
    metadata_signed_by_sender: bool,
    encrypted_data: EncryptedData,
    recovery_data: Option<RecoveryData>,
    minimum_value_promise: MicroTari,
}

impl UnblindedOutputBuilder {
    pub fn new(value: MicroTari, spending_key: PrivateKey) -> Self {
        Self {
            value,
            spending_key,
            features: OutputFeatures::default(),
            script: None,
            covenant: Covenant::default(),
            input_data: None,
            script_private_key: None,
            sender_offset_public_key: None,
            metadata_signature: None,
            metadata_signed_by_receiver: false,
            metadata_signed_by_sender: false,
            encrypted_data: EncryptedData::default(),
            recovery_data: None,
            minimum_value_promise: MicroTari::zero(),
        }
    }

    pub fn with_sender_offset_public_key(&mut self, sender_offset_public_key: PublicKey) {
        self.sender_offset_public_key = Some(sender_offset_public_key);
    }

    pub fn with_features(mut self, features: OutputFeatures) -> Self {
        self.features = features;
        self
    }

    pub fn with_script(mut self, script: TariScript) -> Self {
        self.script = Some(script);
        self
    }

    pub fn with_input_data(mut self, input_data: ExecutionStack) -> Self {
        self.input_data = Some(input_data);
        self
    }

    pub fn with_recovery_and_encrypted_data(mut self, recovery_data: RecoveryData) -> Result<Self, TransactionError> {
        self.recovery_data = Some(recovery_data.clone());
        let commitment = CryptoFactories::default()
            .commitment
            .commit_value(&self.spending_key, self.value.as_u64());
        self.encrypted_data = EncryptedData::encrypt_data(
            &recovery_data.encryption_key,
            &commitment,
            self.value,
            &self.spending_key,
        )
        .map_err(|e| TransactionError::EncryptionError(format!("{}", e)))?;
        Ok(self)
    }

    pub fn with_script_private_key(mut self, script_private_key: PrivateKey) -> Self {
        self.script_private_key = Some(script_private_key);
        self
    }

    pub fn value(&self) -> MicroTari {
        self.value
    }

    pub fn features(&self) -> &OutputFeatures {
        &self.features
    }

    pub fn script(&self) -> Option<&TariScript> {
        self.script.as_ref()
    }

    pub fn covenant(&self) -> &Covenant {
        &self.covenant
    }
}
