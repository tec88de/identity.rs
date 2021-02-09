// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use identity::core::decode_b58;
use identity::core::encode_b58;
use identity::crypto::merkle_key::Sha256;
use identity::crypto::merkle_tree::Proof;
use identity::crypto::KeyCollection as KeyCollection_;
use identity::crypto::KeyType;
use identity::crypto::PublicKey;
use identity::crypto::SecretKey;
use wasm_bindgen::prelude::*;

use crate::crypto::Digest;
use crate::crypto::KeyPair;
use crate::utils::err;

#[derive(Deserialize, Serialize)]
struct JsonData {
  #[serde(rename = "type")]
  type_: KeyType,
  keys: Vec<KeyData>,
}

#[derive(Deserialize, Serialize)]
struct KeyData {
  public: String,
  secret: String,
}

// =============================================================================
// =============================================================================

#[wasm_bindgen(inspectable)]
#[derive(Clone, Debug)]
pub struct KeyCollection(pub(crate) KeyCollection_);

#[wasm_bindgen]
impl KeyCollection {
  /// Creates a new `KeyCollection` with `ed25519` keys.
  #[wasm_bindgen(constructor)]
  pub fn new(value: &str, count: usize) -> Result<KeyCollection, JsValue> {
    let type_: KeyType = KeyPair::parse_key_type(value)?;
    let keys: KeyCollection_ = KeyCollection_::new(type_, count).map_err(err)?;

    Ok(Self(keys))
  }

  /// Returns the number of keys in the collection.
  #[wasm_bindgen(getter)]
  pub fn length(&self) -> usize {
    self.0.len()
  }

  /// Returns `true` if the collection contains no keys.
  #[wasm_bindgen(js_name = isEmpty)]
  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  /// Returns the keypair at the specified `index`.
  #[wasm_bindgen]
  pub fn keypair(&self, index: usize) -> Option<KeyPair> {
    self.0.keypair(index).map(KeyPair)
  }

  /// Returns the public key at the specified `index` as a base58-encoded string.
  #[wasm_bindgen]
  pub fn public(&self, index: usize) -> JsValue {
    match self.0.public(index) {
      Some(key) => encode_b58(key).into(),
      None => JsValue::NULL,
    }
  }

  /// Returns the secret key at the specified `index` as a base58-encoded string.
  #[wasm_bindgen]
  pub fn secret(&self, index: usize) -> JsValue {
    match self.0.secret(index) {
      Some(key) => encode_b58(key).into(),
      None => JsValue::NULL,
    }
  }

  #[wasm_bindgen(js_name = merkleRoot)]
  pub fn merkle_root(&self, digest: &JsValue) -> Result<JsValue, JsValue> {
    match Digest::from_value(digest)? {
      Digest::Sha256 => Ok(encode_b58(self.0.merkle_root::<Sha256>().as_slice()).into()),
    }
  }

  #[wasm_bindgen(js_name = merkleProof)]
  pub fn merkle_proof(&self, digest: &JsValue, index: usize) -> Result<JsValue, JsValue> {
    match Digest::from_value(digest)? {
      Digest::Sha256 => {
        let proof: Proof<Sha256> = match self.0.merkle_proof(index) {
          Some(proof) => proof,
          None => return Ok(JsValue::NULL),
        };

        Ok(encode_b58(&proof.encode()).into())
      }
    }
  }

  /// Serializes a `KeyCollection` object as a JSON object.
  #[wasm_bindgen(js_name = toJSON)]
  pub fn to_json(&self) -> Result<JsValue, JsValue> {
    let public: _ = self.0.iter_public();
    let secret: _ = self.0.iter_secret();

    let keys: Vec<KeyData> = public
      .zip(secret)
      .map(|(public, secret)| KeyData {
        public: encode_b58(public),
        secret: encode_b58(secret),
      })
      .collect();

    let data: JsonData = JsonData {
      keys,
      type_: self.0.type_(),
    };

    JsValue::from_serde(&data).map_err(err)
  }

  /// Deserializes a `KeyCollection` object from a JSON object.
  #[wasm_bindgen(js_name = fromJSON)]
  pub fn from_json(json: &JsValue) -> Result<KeyCollection, JsValue> {
    let data: JsonData = json.into_serde().map_err(err)?;

    let iter: _ = data.keys.iter().flat_map(|data| {
      let pk: PublicKey = decode_b58(&data.public).ok()?.into();
      let sk: SecretKey = decode_b58(&data.secret).ok()?.into();

      Some((pk, sk))
    });

    KeyCollection_::from_iterator(data.type_, iter).map_err(err).map(Self)
  }
}