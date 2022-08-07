#![cfg_attr(not(feature = "std"), no_std)]
pub use pallet::*;

use frame_support::{dispatch::DispatchResult, log, pallet_prelude::*};
use frame_system::{
	offchain::{AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer},
	pallet_prelude::*,
};
// #[cfg(test)]
// mod tests;
// #[cfg(feature = "runtime-benchmarks")]
// mod benchmarking;

#[frame_support::pallet]
pub mod pallet {

	use super::*;
	use scale_info::TypeInfo;
	use sp_core::crypto::KeyTypeId;
	use sp_io;
	use sp_runtime::offchain::{http, Duration};
	use sp_std::{prelude::*, str};

	use serde::Deserialize;

	pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"esg!");

	use codec::alloc::string::String;
	use scale_info::prelude::format;
	use sp_core::sr25519::Signature as Sr25519Signature;
	use sp_runtime::{
		app_crypto::{app_crypto, sr25519},
		traits::Verify,
		MultiSignature, MultiSigner,
	};
	app_crypto!(sr25519, KEY_TYPE);

	pub struct TestAuthId;

	// implemented for runtime
	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
		type RuntimeAppPublic = Public;
		type GenericPublic = sp_core::sr25519::Public;
		type GenericSignature = sp_core::sr25519::Signature;
	}

	impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature>
		for TestAuthId
	{
		type RuntimeAppPublic = Public;
		type GenericPublic = sp_core::sr25519::Public;
		type GenericSignature = sp_core::sr25519::Signature;
	}

	// We are fetching information from the esg oracle endpoint, currently ran on localhost:8080/score
	const HTTP_REMOTE_REQUEST: &str = "https://testnet.5ire.network/oracle";
	const FETCH_TIMEOUT_PERIOD: u64 = 3000; // in milli-seconds

	/// EsgInfo is the payload that we expect to receive from the oracle.
	#[derive(Deserialize, Encode, Decode, Default,TypeInfo)]
	pub struct EsgInfo {
		//company name
		name: Vec<u8>,
		// sustainable score
		sustainability: u32,
	}

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: CreateSignedTransaction<Call<Self>> + frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Because we sign transactions we need the authority ID
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;
		/// The overarching dispatch call type.
		type Call: From<Call<Self>>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The score gotten from the ESG Oracle on a validator.
		/// [score, score_owner]
		Score{
			who: T::AccountId,
			name: Vec<u8>,
			score: u32,
		},
		SetUrl(Vec<u8>),
	}


	#[pallet::storage]
	#[pallet::getter(fn score)]
	pub(super) type Scores<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, EsgInfo, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn sustainability)]
	pub(super) type SustainScores<T: Config> =
		StorageMap<_, Blake2_128Concat,Vec<u8>, u32, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn url)]
	pub(super) type Url<T: Config> =
		StorageValue<_, Vec<u8>>;
	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
		/// The offchain worker could not fetch the ESG score from the ESG oracle
		OffchainFetchESGScoreError,
		/// The offchain worker could not sign the transaction with the esg score
		OffchainSignedTxError,
		/// The offchain worker could not successfully execute the http request
		HttpFetchingError,
		/// The offchain worker culd not find a local account for signing
		NoLocalAcctForSigning,
		/// Could not find a score for the given account id in the Scores map
		NoAccountScore,
	}

	// the offchain worker entrypoint
	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(_block_number: T::BlockNumber) {
			let score = Self::fetch_esg_score();
			if let Err(e) = score {
				log::error!("offchain_worker error: {:?}", e);
			}
		}
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// TODO decide if we want to make this an unsigned tx with signed payload
		#[pallet::weight(10_000)]
		pub fn emit_score(origin: OriginFor<T>,name:Vec<u8>, score: u32) -> DispatchResult {
			let who = ensure_signed(origin)?;
			// This is the on-chain function
			Self::store_esg_score(name.clone(), score);
			Self::deposit_event(Event::Score{who,name, score});
			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn set_url(origin: OriginFor<T>, url: Vec<u8>) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			
			Url::<T>::put(url.clone());
			Self::deposit_event(Event::SetUrl(url));
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		// get the esg_score from the endpoint, emit the esg score in a signed transaction
		pub fn fetch_esg_score() -> Result<(), Error<T>> {
			let resp_bytes = Self::fetch_from_endpoint().map_err(|e| {
				log::error!("fetch_from_remote error: {:?}", e);
				<Error<T>>::HttpFetchingError
			})?;

			log::info!("check 1");
			let resp_str =
				str::from_utf8(&resp_bytes).map_err(|_| <Error<T>>::HttpFetchingError)?;

			log::info!("check 2");
			log::info!("value:{}",resp_str);
			// let mut resp = resp_str.clone();
			// let mut initial = "".to_owned();
			// let mut res: Vec<usize> = Vec::new();
			// while resp.find('"').is_some() {
			// 	log::info!("go to while");
			// 	let index = resp.find('"').unwrap();

			// 	res.push(index + initial.len());

			// 	let (begin, last) = resp.split_at(index+1);

			// 	log::info!("begin:{}", begin);
			// 	log::info!("last:{}", last);
			// 	resp = last;
			// 	initial = format!("{}{}", initial, begin);

			// 	log::info!("resp:{}", resp);
			// }

			// log::info!("res:{:?}", res);
			// let name = &resp_str[res[1]..res[2]];
			// log::info!("name:{}",name);

			let score: EsgInfo =
				serde_json::from_str(resp_str).map_err(|_| <Error<T>>::HttpFetchingError)?;
			log::info!("check 3");
			Self::offchain_signed_score(score.name, score.sustainability)?;
			Ok(())
		}

		pub fn offchain_signed_score(name: Vec<u8>, score: u32) -> Result<(), Error<T>> {
			let signer = Signer::<T, T::AuthorityId>::any_account();
			// `result` is in the type of `Option<(Account<T>, Result<(), ()>)>`. It is:
			//   - `None`: no account is available for sending transaction
			//   - `Some((account, Ok(())))`: transaction is successfully sent
			//   - `Some((account, Err(())))`: error occured when sending the transaction
			let result = signer.send_signed_transaction(|_acct|{
				// This is the on-chain function
				let name = name.clone();
				log::info!("offchain signed name:{:?}, score :{}", name.clone(), score);
				Call::emit_score {name, score }
			});

			// Display error if the signed tx fails.
			if let Some((acc, res)) = result {
				if res.is_err() {
					log::error!("failure: offchain_signed_tx: tx sent: {:?}", acc.id);
					return Err(<Error<T>>::OffchainSignedTxError)
				}
				// Transaction is sent successfully
				// Now we can add it to the scores map since it's successfully submitted
				// If the key exists, update it, else create a new entry
				//Self::store_esg_score(name, score);
				Ok(())
			} else {
				// The case result == `None`: no account is available for sending
				log::error!("No local account available");
				Err(<Error<T>>::NoLocalAcctForSigning)
			}
		}

		fn fetch_from_endpoint() -> Result<Vec<u8>, Error<T>> {
			// Initiate an external HTTP GET request.
			let url = Url::<T>::get().unwrap_or_default();
			let url_str = str::from_utf8(&url).unwrap_or_default();
			let request = http::Request::get(url_str);

			// Keeping the offchain worker execution time reasonable, so limiting the call to be within 3s.
			let timeout =
				sp_io::offchain::timestamp().add(Duration::from_millis(FETCH_TIMEOUT_PERIOD));

			let pending = request
				.deadline(timeout) // Setting the timeout time
				.send() // Sending the request out by the host
				.map_err(|e| {
					log::error!("{:?}", e);
					<Error<T>>::HttpFetchingError
				})?;

			// By default, the http request is async from the runtime perspective. So we are asking the
			// runtime to wait here
			// The returning value here is a `Result` of `Result`, so we are unwrapping it twice by two `?`
			//   ref: https://docs.substrate.io/rustdocs/latest/sp_runtime/offchain/http/struct.PendingRequest.html#method.try_wait
			let response = pending
				.try_wait(timeout)
				.map_err(|e| {
					log::error!("{:?}", e);
					<Error<T>>::HttpFetchingError
				})?
				.map_err(|e| {
					log::error!("{:?}", e);
					<Error<T>>::HttpFetchingError
				})?;

			if response.code != 200 {
				log::error!("Unexpected http request status code: {}", response.code);
				return Err(<Error<T>>::HttpFetchingError)
			}

			// Next we fully read the response body and collect it to a vector of bytes.
			Ok(response.body().collect::<Vec<u8>>())
		}

		// add a new score to the storage under the account id
		pub fn store_esg_score(name: Vec<u8> ,score: u32) {
			log::info!("go in to storage esg score");
			log::info!("storage name:{:?}", name);
			log::info!("storage score:{}",score);
			// if Scores::<T>::contains_key(acc_id) {
			// 	Scores::<T>::mutate(acc_id, |val| val.sustainability = score);
			// } else {
			// 	let esg_info = EsgInfo {name: name, sustainability: score};
			// 	Scores::<T>::insert(acc_id, esg_info);
			// };

			if SustainScores::<T>::contains_key(&name){
				log::info!("go in to storage esg score mutate");
				SustainScores::<T>::mutate(&name, |val| *val = score);
			}
			else {
				log::info!("go in to storage esg score insert");
				SustainScores::<T>::insert(name,score);
			}
		}

		pub fn get_sustain(name: Vec<u8>) -> u32 {
			let score = SustainScores::<T>::get(name);
			score
		}

	}
}