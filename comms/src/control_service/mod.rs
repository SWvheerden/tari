//  Copyright 2019 The Tari Project
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

//! # Control Service
//!
//! The control service listens on the configured address for [EstablishConnection] messages
//! and decides whether to connect to the requested address.
//!
//! ```edition2018
//! # use tari_comms::{connection::*, control_service::*, dispatcher::*, connection_manager::*, peer_manager::*, types::*};
//! # use tari_comms::control_service::handlers as comms_handlers;
//! # use std::{time::Duration, sync::Arc};
//! # use tari_storage::lmdb::LMDBStore;
//! # use std::collections::HashMap;
//! # use rand::OsRng;
//!
//! let node_identity = Arc::new(NodeIdentity::random(&mut OsRng::new().unwrap(), "127.0.0.1:9000".parse().unwrap()).unwrap());
//!
//! let context = ZmqContext::new();
//! let listener_address = "127.0.0.1:9000".parse::<NetAddress>().unwrap();
//!
//! let peer_manager = Arc::new(PeerManager::<LMDBStore>::new(None).unwrap());
//!
//! let conn_manager = Arc::new(ConnectionManager::new(context.clone(), node_identity.clone(), peer_manager.clone(), PeerConnectionConfig {
//!      max_message_size: 1024,
//!      max_connect_retries: 1,
//!      socks_proxy_address: None,
//!      message_sink_address: InprocAddress::random(),
//!      host: "127.0.0.1".parse().unwrap(),
//!      peer_connection_establish_timeout: Duration::from_secs(4),
//! }));
//!
//! let service = ControlService::<u8>::with_default_config(
//!       context,
//!       node_identity,
//!     )
//!     .serve(conn_manager)
//!     .unwrap();
//!
//! service.shutdown().unwrap();
//! ```
mod error;
pub mod handlers;
mod service;
mod types;
mod worker;

pub use self::{
    error::ControlServiceError,
    service::{ControlService, ControlServiceConfig, ControlServiceHandle},
    types::{ControlServiceMessageContext, ControlServiceMessageType},
};
