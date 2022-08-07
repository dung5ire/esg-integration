#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::vec::Vec;

sp_api::decl_runtime_apis! {
	pub trait EsgFetcherApi
	{
		fn query_info(name: Vec<u8>) -> u32;
	}
}