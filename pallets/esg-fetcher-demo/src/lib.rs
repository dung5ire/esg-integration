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
	use sp_runtime::offchain::{http, Duration,storage::{MutateStorageError, StorageRetrievalError, StorageValueRef}};
	use sp_std::{prelude::*, str};

	use serde::Deserialize;
	enum TransactionType {
		Signed,
		None,
	}

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
	// const HTTP_REMOTE_REQUEST: &str = "https://testnet.5ire.network/oracle";
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
		// Configuration parameters

		/// A grace period after we send transaction.
		///
		/// To avoid sending too many transactions, we only attempt to send one
		/// every `GRACE_PERIOD` blocks. We use Local Storage to coordinate
		/// sending between distinct runs of this offchain worker.
		#[pallet::constant]
		type GracePeriod: Get<Self::BlockNumber>;


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
		fn offchain_worker(block_number: T::BlockNumber) {
			let should_send = Self::choose_transaction_type(block_number);
			let res = match should_send {
				TransactionType::Signed => Self::fetch_esg_score(),
				TransactionType::None => Ok(()),
			};
			if let Err(e) = res {
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

		fn choose_transaction_type(block_number: T::BlockNumber) -> TransactionType {
			/// A friendlier name for the error that is going to be returned in case we are in the grace
			/// period.
			const RECENTLY_SENT: () = ();

			// Start off by creating a reference to Local Storage value.
			// Since the local storage is common for all offchain workers, it's a good practice
			// to prepend your entry with the module name.
			let val = StorageValueRef::persistent(b"example_ocw::last_send");
			// The Local Storage is persisted and shared between runs of the offchain workers,
			// and offchain workers may run concurrently. We can use the `mutate` function, to
			// write a storage entry in an atomic fashion. Under the hood it uses `compare_and_set`
			// low-level method of local storage API, which means that only one worker
			// will be able to "acquire a lock" and send a transaction if multiple workers
			// happen to be executed concurrently.
			let res = val.mutate(|last_send: Result<Option<T::BlockNumber>, StorageRetrievalError>| {
				match last_send {
					// If we already have a value in storage and the block number is recent enough
					// we avoid sending another transaction at this time.
					Ok(Some(block)) if block_number < block + T::GracePeriod::get() =>
						Err(RECENTLY_SENT),
					// In every other case we attempt to acquire the lock and send a transaction.
					_ => Ok(block_number),
				}
			});

			// The result of `mutate` call will give us a nested `Result` type.
			// The first one matches the return of the closure passed to `mutate`, i.e.
			// if we return `Err` from the closure, we get an `Err` here.
			// In case we return `Ok`, here we will have another (inner) `Result` that indicates
			// if the value has been set to the storage correctly - i.e. if it wasn't
			// written to in the meantime.
			match res {
				// The value has been set correctly, which means we can safely send a transaction now.
				Ok(_) => {
					// Depending if the block is even or odd we will send a `Signed` or `Unsigned`
					// transaction.
					// Note that this logic doesn't really guarantee that the transactions will be sent
					// in an alternating fashion (i.e. fairly distributed). Depending on the execution
					// order and lock acquisition, we may end up for instance sending two `Signed`
					// transactions in a row. If a strict order is desired, it's better to use
					// the storage entry for that. (for instance store both block number and a flag
					// indicating the type of next transaction to send).
					TransactionType::Signed
				},
				// We are in the grace period, we should not send a transaction this time.
				Err(MutateStorageError::ValueFunctionFailed(RECENTLY_SENT)) => TransactionType::None,
				// We wanted to send a transaction, but failed to write the block number (acquire a
				// lock). This indicates that another offchain worker that was running concurrently
				// most likely executed the same logic and succeeded at writing to storage.
				// Thus we don't really want to send the transaction, knowing that the other run
				// already did.
				Err(MutateStorageError::ConcurrentModification(_)) => TransactionType::None,
			}
		}
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