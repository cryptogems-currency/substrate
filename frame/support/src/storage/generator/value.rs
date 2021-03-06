// Copyright 2019-2020 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

use codec::{FullCodec, Encode, EncodeLike, Decode};
use crate::{
	Never,
	storage::{self, unhashed, StorageAppend},
	hash::{Twox128, StorageHasher},
	traits::Len
};

/// Generator for `StorageValue` used by `decl_storage`.
///
/// By default value is stored at:
/// ```nocompile
/// Twox128(module_prefix) ++ Twox128(storage_prefix)
/// ```
pub trait StorageValue<T: FullCodec> {
	/// The type that get/take returns.
	type Query;

	/// Module prefix. Used for generating final key.
	fn module_prefix() -> &'static [u8];

	/// Storage prefix. Used for generating final key.
	fn storage_prefix() -> &'static [u8];

	/// Convert an optional value retrieved from storage to the type queried.
	fn from_optional_value_to_query(v: Option<T>) -> Self::Query;

	/// Convert a query to an optional value into storage.
	fn from_query_to_optional_value(v: Self::Query) -> Option<T>;

	/// Generate the full key used in top storage.
	fn storage_value_final_key() -> [u8; 32] {
		let mut final_key = [0u8; 32];
		final_key[0..16].copy_from_slice(&Twox128::hash(Self::module_prefix()));
		final_key[16..32].copy_from_slice(&Twox128::hash(Self::storage_prefix()));
		final_key
	}
}

impl<T: FullCodec, G: StorageValue<T>> storage::StorageValue<T> for G {
	type Query = G::Query;

	fn hashed_key() -> [u8; 32] {
		Self::storage_value_final_key()
	}

	fn exists() -> bool {
		unhashed::exists(&Self::storage_value_final_key())
	}

	fn get() -> Self::Query {
		let value = unhashed::get(&Self::storage_value_final_key());
		G::from_optional_value_to_query(value)
	}

	fn try_get() -> Result<T, ()> {
		unhashed::get(&Self::storage_value_final_key()).ok_or(())
	}

	fn translate<O: Decode, F: FnOnce(Option<O>) -> Option<T>>(f: F) -> Result<Option<T>, ()> {
		let key = Self::storage_value_final_key();

		// attempt to get the length directly.
		let maybe_old = match unhashed::get_raw(&key) {
			Some(old_data) => Some(O::decode(&mut &old_data[..]).map_err(|_| ())?),
			None => None,
		};
		let maybe_new = f(maybe_old);
		if let Some(new) = maybe_new.as_ref() {
			new.using_encoded(|d| unhashed::put_raw(&key, d));
		} else {
			unhashed::kill(&key);
		}
		Ok(maybe_new)
	}

	fn put<Arg: EncodeLike<T>>(val: Arg) {
		unhashed::put(&Self::storage_value_final_key(), &val)
	}

	fn set(maybe_val: Self::Query) {
		if let Some(val) = G::from_query_to_optional_value(maybe_val) {
			unhashed::put(&Self::storage_value_final_key(), &val)
		} else {
			unhashed::kill(&Self::storage_value_final_key())
		}
	}

	fn kill() {
		unhashed::kill(&Self::storage_value_final_key())
	}

	fn mutate<R, F: FnOnce(&mut G::Query) -> R>(f: F) -> R {
		Self::try_mutate(|v| Ok::<R, Never>(f(v))).expect("`Never` can not be constructed; qed")
	}

	fn try_mutate<R, E, F: FnOnce(&mut G::Query) -> Result<R, E>>(f: F) -> Result<R, E> {
		let mut val = G::get();

		let ret = f(&mut val);
		if ret.is_ok() {
			match G::from_query_to_optional_value(val) {
				Some(ref val) => G::put(val),
				None => G::kill(),
			}
		}
		ret
	}

	fn take() -> G::Query {
		let key = Self::storage_value_final_key();
		let value = unhashed::get(&key);
		if value.is_some() {
			unhashed::kill(&key)
		}
		G::from_optional_value_to_query(value)
	}

	fn append<Item, EncodeLikeItem>(item: EncodeLikeItem)
	where
		Item: Encode,
		EncodeLikeItem: EncodeLike<Item>,
		T: StorageAppend<Item>,
	{
		let key = Self::storage_value_final_key();
		sp_io::storage::append(&key, item.encode());
	}

	fn decode_len() -> Result<usize, &'static str> where T: codec::DecodeLength, T: Len {
		let key = Self::storage_value_final_key();

		// attempt to get the length directly.
		if let Some(k) = unhashed::get_raw(&key) {
			<T as codec::DecodeLength>::len(&k).map_err(|e| e.what())
		} else {
			let len = G::from_query_to_optional_value(G::from_optional_value_to_query(None))
				.map(|v| v.len())
				.unwrap_or(0);

			Ok(len)
		}
	}
}
