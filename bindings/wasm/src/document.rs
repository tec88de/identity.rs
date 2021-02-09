// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use identity::core::decode_b58;
use identity::core::encode_b58;
use identity::core::FromJson;
use identity::core::SerdeInto;
use identity::crypto::merkle_key::MerkleKey;
use identity::crypto::merkle_key::MerkleTag;
use identity::crypto::merkle_key::Sha256;
use identity::crypto::merkle_tree::Hash;
use identity::crypto::JcsEd25519Signature2020 as Ed25519;
use identity::crypto::KeyType;
use identity::crypto::PublicKey;
use identity::crypto::SecretKey;
use identity::did::verifiable;
use identity::did::verifiable::Public;
use identity::did::verifiable::Secret;
use identity::did::Document as Document_;
use identity::did::MethodIdent;
use identity::did::MethodScope;
use identity::did::MethodWrap;
use identity::did::Service;
use identity::iota::DocumentDiff;
use identity::iota::IotaDocument;
use identity::iota::Properties;
use wasm_bindgen::prelude::*;

use crate::crypto::Digest;
use crate::crypto::KeyCollection;
use crate::crypto::KeyPair;
use crate::did::DID;
use crate::method::Method;
use crate::utils::err;

#[wasm_bindgen(inspectable)]
pub struct NewDocument {
  key: KeyPair,
  doc: Document,
}

#[wasm_bindgen]
impl NewDocument {
  #[wasm_bindgen(getter)]
  pub fn key(&self) -> KeyPair {
    self.key.clone()
  }

  #[wasm_bindgen(getter)]
  pub fn doc(&self) -> Document {
    self.doc.clone()
  }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Params {
  None,
  Object {
    network: Option<String>,
    shard: Option<String>,
    tag: Option<String>,
  },
}

impl Params {
  fn network(&self) -> Option<&str> {
    match self {
      Self::None => None,
      Self::Object { network, .. } => network.as_deref(),
    }
  }

  fn shard(&self) -> Option<&str> {
    match self {
      Self::None => None,
      Self::Object { shard, .. } => shard.as_deref(),
    }
  }

  fn tag(&self) -> Option<&str> {
    match self {
      Self::None => None,
      Self::Object { tag, .. } => tag.as_deref(),
    }
  }
}

// =============================================================================
// =============================================================================

#[wasm_bindgen(inspectable)]
#[derive(Clone, Debug, PartialEq)]
pub struct Document(pub(crate) IotaDocument);

#[wasm_bindgen]
impl Document {
  #[wasm_bindgen(constructor)]
  pub fn new(type_: &str, params: &JsValue) -> Result<NewDocument, JsValue> {
    let params: Params = params.into_serde().map_err(err)?;
    let key: KeyPair = KeyPair::new(type_)?;
    let did: DID = DID::create(key.0.public().as_ref(), params.network(), params.shard())?;
    let auth: Method = Method::create(&key, did, params.tag())?;
    let doc: Document = Self::from_auth(auth)?;

    Ok(NewDocument { doc, key })
  }

  /// Creates a new `DID` from an authentication `Method` object.
  #[wasm_bindgen(js_name = fromAuth)]
  pub fn from_auth(auth: Method) -> Result<Document, JsValue> {
    Document_::builder(Properties::new())
      .id(auth.0.controller().clone())
      .authentication(auth.0)
      .build()
      .map(Document_::into_verifiable)
      .map(Into::into)
      .map(Self)
      .map_err(err)
  }

  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.0.id().to_string()
  }

  #[wasm_bindgen(getter)]
  pub fn proof(&self) -> Result<JsValue, JsValue> {
    self
      .0
      .proof()
      .map(|proof| JsValue::from_serde(proof))
      .transpose()
      .map_err(err)
      .map(|option| option.unwrap_or(JsValue::NULL))
  }

  #[wasm_bindgen]
  pub fn sign(&mut self, key: &KeyPair) -> Result<JsValue, JsValue> {
    self.0.sign(key.0.secret()).map_err(err).map(|_| JsValue::NULL)
  }

  /// Verify the signature with the authentication_key
  #[wasm_bindgen]
  pub fn verify(&self) -> bool {
    self.0.verify().is_ok()
  }

  /// Creates a Merkle Key Collection public key value for the given key
  /// collection instance.
  ///
  /// The public key value will be encoded using Base58 encoding.
  #[wasm_bindgen(js_name = encodeMerkleKey)]
  pub fn encode_merkle_key(digest: &str, keys: &KeyCollection) -> Result<JsValue, JsValue> {
    match (keys.0.type_(), digest.into()) {
      (KeyType::Ed25519, Digest::Sha256) => {
        let root: Hash<Sha256> = keys.0.merkle_root();
        let data: Vec<u8> = MerkleKey::encode_key::<Ed25519, Sha256>(&Ed25519, &root);

        Ok(encode_b58(&data).into())
      }
    }
  }

  /// Creates a Merkle Key Collection signature for the given `data` with the
  /// DID Document verification method identified by `method`.
  ///
  /// A key collection (`keys`) is required and the keypair at `index` is
  /// used for signing.
  #[wasm_bindgen(js_name = signMerkleKey)]
  pub fn sign_merkle_key(
    &self,
    data: &JsValue,
    keys: &KeyCollection,
    method: &str,
    index: usize,
  ) -> Result<JsValue, JsValue> {
    let mut data: verifiable::Properties = data.into_serde().map_err(err)?;

    let digest: MerkleTag = {
      let method: MethodWrap<'_> = self.0.try_resolve(method).map_err(err)?;
      let public: Vec<u8> = method.key_data().try_decode().map_err(err)?;

      MerkleKey::extract_tags(&public).map_err(err)?.1
    };

    let secret: &SecretKey = match keys.0.secret(index) {
      Some(secret) => secret,
      None => return Err("Invalid Secret Key Index".into()),
    };

    match digest {
      MerkleTag::SHA256 => match keys.0.merkle_proof::<Sha256>(index) {
        Some(proof) => {
          self
            .0
            .sign_that(&mut data, method, Secret::with_merkle_proof(secret.as_ref(), &proof))
            .map_err(err)?;
        }
        None => return Err("Invalid Public Key Proof".into()),
      },
      _ => return Err("Invalid Merkle Key Digest".into()),
    }

    JsValue::from_serde(&data).map_err(err)
  }

  /// Verifies the authenticity of `data` using the given Merkle Key Collection
  /// target public key.
  ///
  /// The target public key is expected to be a Base58-encoded string.
  #[wasm_bindgen(js_name = verifyMerkleKey)]
  pub fn verify_merkle_key(&self, data: &JsValue, target: String) -> Result<JsValue, JsValue> {
    let data: verifiable::Properties = data.into_serde().map_err(err)?;

    let public: PublicKey = decode_b58(&target).map_err(err).map(Into::into)?;
    let public: Public<'_> = Public::with_merkle_target(public.as_ref());

    self.0.verify_that(&data, public).map_err(err)?;

    Ok(JsValue::TRUE)
  }

  /// Generate the difference between two DID Documents and sign it
  #[wasm_bindgen]
  pub fn diff(&self, other: &Document, key: &KeyPair, prev_msg: String) -> Result<JsValue, JsValue> {
    let doc: IotaDocument = other.0.clone();
    let diff: DocumentDiff = self.0.diff(&doc, key.0.secret(), prev_msg.into()).map_err(err)?;

    JsValue::from_serde(&diff).map_err(err)
  }

  /// Verify the signature in a diff with the authentication_key
  #[wasm_bindgen(js_name = verifyDiff)]
  pub fn verify_diff(&self, diff: String) -> bool {
    match DocumentDiff::from_json(&diff) {
      Ok(diff) => self.0.verify_data(&diff).is_ok(),
      Err(_) => false,
    }
  }

  #[wasm_bindgen(js_name = updateService)]
  pub fn update_service(&mut self, json: &JsValue) -> Result<bool, JsValue> {
    let service: Service = json.into_serde().map_err(err)?;
    Self::mutate(self, |doc| doc.service_mut().update(service.into()))
  }

  #[wasm_bindgen(js_name = clearServices)]
  pub fn clear_services(&mut self) -> Result<(), JsValue> {
    Self::mutate(self, |doc| doc.service_mut().clear())
  }

  #[wasm_bindgen(js_name = updateAuthentication)]
  pub fn update_authentication(&mut self, method: &Method) -> Result<bool, JsValue> {
    Self::mutate(self, |doc| {
      doc.authentication_mut().update(method.0.clone().into_ref().into())
    })
  }

  #[wasm_bindgen(js_name = clearAuthentication)]
  pub fn clear_authentication(&mut self) -> Result<(), JsValue> {
    Self::mutate(self, |doc| doc.authentication_mut().clear())
  }

  #[wasm_bindgen(js_name = updateAssertionMethod)]
  pub fn update_assertion_method(&mut self, method: &Method) -> Result<bool, JsValue> {
    Self::mutate(self, |doc| {
      doc.assertion_method_mut().update(method.0.clone().into_ref().into())
    })
  }

  #[wasm_bindgen(js_name = clearAssertionMethod)]
  pub fn clear_assertion_method(&mut self) -> Result<(), JsValue> {
    Self::mutate(self, |doc| doc.assertion_method_mut().clear())
  }

  #[wasm_bindgen(js_name = updateVerificationMethod)]
  pub fn update_verification_method(&mut self, method: &Method) -> Result<bool, JsValue> {
    Self::mutate(self, |doc| {
      doc.verification_method_mut().update(method.0.clone().into())
    })
  }

  #[wasm_bindgen(js_name = clearVerificationMethod)]
  pub fn clear_verification_method(&mut self) -> Result<(), JsValue> {
    Self::mutate(self, |doc| doc.verification_method_mut().clear())
  }

  #[wasm_bindgen(js_name = updateCapabilityDelegation)]
  pub fn update_capability_delegation(&mut self, method: &Method) -> Result<bool, JsValue> {
    Self::mutate(self, |doc| {
      doc
        .capability_delegation_mut()
        .update(method.0.clone().into_ref().into())
    })
  }

  #[wasm_bindgen(js_name = clearCapabilityDelegation)]
  pub fn clear_capability_delegation(&mut self) -> Result<(), JsValue> {
    Self::mutate(self, |doc| doc.capability_delegation_mut().clear())
  }

  #[wasm_bindgen(js_name = updateCapabilityInvocation)]
  pub fn update_capability_invocation(&mut self, method: &Method) -> Result<bool, JsValue> {
    Self::mutate(self, |doc| {
      doc
        .capability_invocation_mut()
        .update(method.0.clone().into_ref().into())
    })
  }

  #[wasm_bindgen(js_name = clearCapabilityInvocation)]
  pub fn clear_capability_invocation(&mut self) -> Result<(), JsValue> {
    Self::mutate(self, |doc| doc.capability_invocation_mut().clear())
  }

  #[wasm_bindgen(js_name = updateKeyAgreement)]
  pub fn update_key_agreement(&mut self, method: &Method) -> Result<bool, JsValue> {
    Self::mutate(self, |doc| {
      doc.key_agreement_mut().update(method.0.clone().into_ref().into())
    })
  }

  #[wasm_bindgen(js_name = clearKeyAgreement)]
  pub fn clear_key_agreement(&mut self) -> Result<(), JsValue> {
    Self::mutate(self, |doc| doc.key_agreement_mut().clear())
  }

  #[wasm_bindgen(js_name = resolveKey)]
  pub fn resolve_key(&mut self, ident: JsValue, scope: Option<String>) -> Result<Method, JsValue> {
    let borrow: String;

    let ident: MethodIdent = if let Some(number) = ident.as_f64() {
      number.to_string().parse().map_err(err).map(MethodIdent::Index)?
    } else if let Some(ident) = ident.as_string() {
      borrow = ident;
      MethodIdent::Ident(&borrow)
    } else {
      return Err("Invalid Verification Method Identifier".into());
    };

    let scope: MethodScope = scope
      .map(|scope| scope.parse::<MethodScope>())
      .transpose()
      .map_err(err)?
      .unwrap_or(MethodScope::Authentication);

    self
      .0
      .resolve((ident, scope))
      .map(|wrap| wrap.into_method().clone())
      .map(Method)
      .ok_or_else(|| "Verification Method Not Found".into())
  }

  /// Serializes a `Document` object as a JSON object.
  #[wasm_bindgen(js_name = toJSON)]
  pub fn to_json(&self) -> Result<JsValue, JsValue> {
    JsValue::from_serde(&self.0).map_err(err)
  }

  /// Deserializes a `Document` object from a JSON object.
  #[wasm_bindgen(js_name = fromJSON)]
  pub fn from_json(json: &JsValue) -> Result<Document, JsValue> {
    json.into_serde().map_err(err).map(Self)
  }

  // Bypass IotaDocument Deref limitations and allow modifications to the
  // core DID Document type.
  //
  // Uses `serde` for conversions and re-validates the document after mutation.
  fn mutate<T>(this: &mut Self, f: impl FnOnce(&mut Document_) -> T) -> Result<T, JsValue> {
    let mut document: Document_ = this.0.serde_into().map_err(err)?;
    let output: T = f(&mut document);

    this.0 = IotaDocument::try_from_document(document).map_err(err)?;

    Ok(output)
  }
}
