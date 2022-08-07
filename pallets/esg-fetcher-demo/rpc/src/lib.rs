// use std::{convert::TryInto, sync::Arc};
use std::sync::Arc;
// use codec::{Codec, Decode};
use jsonrpsee::{
	core::{async_trait, Error as JsonRpseeError, RpcResult},
	proc_macros::rpc,
	types::error::{CallError, ErrorCode, ErrorObject},
};

use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
// use sp_core::Bytes;
// use sp_rpc::number::NumberOrHex;
use sp_runtime::{
	generic::BlockId,
	traits::{Block as BlockT},
};

pub use esg_fetcher_rpc_runtime_api::EsgFetcherApi as EsgFetcherRuntimeApi;

#[rpc(client, server)]
pub trait EsgFetcherApi<BlockHash> {
	#[method(name = "esg_querySustainable")]
	fn query_info(&self, name: Vec<u8>, at: Option<BlockHash>) -> RpcResult<u32>;

}

/// Provides RPC methods to query a dispatchable's class, weight and fee.
pub struct EsgFetcher<C, P> {
	/// Shared reference to the client.
	client: Arc<C>,
	_marker: std::marker::PhantomData<P>,
}

impl<C, P> EsgFetcher<C, P> {
	/// Creates a new instance of the TransactionPayment Rpc helper.
	pub fn new(client: Arc<C>) -> Self {
		Self { client, _marker: Default::default() }
	}
}

/// Error type of this RPC api.
pub enum Error {
	/// The transaction was not decodable.
	DecodeError,
	/// The call to runtime failed.
	RuntimeError,
}

impl From<Error> for i32 {
	fn from(e: Error) -> i32 {
		match e {
			Error::RuntimeError => 1,
			Error::DecodeError => 2,
		}
	}
}

#[async_trait]
impl<C, Block>
    EsgFetcherApiServer<<Block as BlockT>::Hash>
	for EsgFetcher<C, Block>
where
	Block: BlockT,
	C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
	C::Api: EsgFetcherRuntimeApi<Block>,
{
	fn query_info(
		&self,
        name: Vec<u8>,
        at: Option<Block::Hash>,
	) -> RpcResult<u32> {
		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));



		api.query_info(&at, name).map_err(|e| {
			CallError::Custom(ErrorObject::owned(
				Error::RuntimeError.into(),
				"Unable to query dispatch info.",
				Some(e.to_string()),
			))
			.into()
		})
	}

}