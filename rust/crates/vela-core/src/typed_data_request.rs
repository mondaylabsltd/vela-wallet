//! One reading of a typed-data signing request.
//!
//! The audit of 2026-10-01 ("what you see is not what you sign",
//! `docs/security/2026-10-01-eip712-wysiwys-audit.md`) found every reader of
//! an `eth_signTypedData*` request picking its own element: the sheet the
//! first string, the passkey `params[1] ?? params[0]`, the SafeTx guard the
//! first document that parsed, the approval guard `params[1]` even for the
//! legacy order. Two documents in one request — `[benign, malicious]` — showed
//! one and signed the other.
//!
//! Here the request is read ONCE, strictly:
//!
//! - only the four methods: `eth_signTypedData` / `_v1` carry
//!   `[typedData, address]`, `_v3` / `_v4` carry `[address, typedData]`
//!   (EIP-712's JSON-RPC, MetaMask's legacy order);
//! - exactly two params, a well-formed address in its slot;
//! - ONE document — a JSON string or an object — with `types`, a non-empty
//!   `primaryType`, `domain` and `message`, which hashes.
//!
//! Anything else is refused before a sheet opens. What passes is carried on as
//! [`CanonicalTypedData::params`] — the two params rebuilt around the one
//! document — so every later reader (preview, guards, passkey, signer page)
//! holds the same bytes, and [`CanonicalTypedData::digest`] is what is signed.

use serde_json::Value;

/// The typed-data methods a wallet signs, and nothing that merely resembles
/// one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypedDataMethod {
    /// `eth_signTypedData`, `eth_signTypedData_v1`: `[typedData, address]`.
    Legacy,
    /// `eth_signTypedData_v3`: `[address, typedData]`.
    V3,
    /// `eth_signTypedData_v4`: `[address, typedData]`.
    V4,
}

impl TypedDataMethod {
    /// The method, by its exact name — `eth_signTypedData_v2` or any other
    /// spelling is not one.
    #[must_use]
    pub fn of(method: &str) -> Option<Self> {
        match method {
            "eth_signTypedData" | "eth_signTypedData_v1" => Some(Self::Legacy),
            "eth_signTypedData_v3" => Some(Self::V3),
            "eth_signTypedData_v4" => Some(Self::V4),
            _ => None,
        }
    }

    /// Where the document sits; the account is in the other slot.
    #[must_use]
    pub const fn document_index(self) -> usize {
        match self {
            Self::Legacy => 0,
            Self::V3 | Self::V4 => 1,
        }
    }

    #[must_use]
    pub const fn account_index(self) -> usize {
        1 - self.document_index()
    }
}

/// A name that routes like typed data — `method.contains("signTypedData")`,
/// how every reader used to decide. Such a request that is not one of the four
/// methods is refused, never guessed at.
#[must_use]
pub fn looks_like_typed_data(method: &str) -> bool {
    method.contains("signTypedData")
}

/// Why a typed-data request is refused (`-32602`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedDataRequestError {
    /// A method that resembles typed data but is not one of the four.
    UnsupportedMethod,
    /// Not exactly `[x, y]`.
    NotTwoParams,
    /// The account slot is not a well-formed address.
    BadAccount,
    /// The document slot is not one EIP-712 document that hashes.
    BadDocument(String),
}

impl TypedDataRequestError {
    /// The fixed English message the dApp is answered with.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::UnsupportedMethod => "Unsupported typed-data method".to_owned(),
            Self::NotTwoParams => {
                "Typed-data params must be exactly [address, typedData] (v3/v4) or [typedData, address] (legacy)"
                    .to_owned()
            }
            Self::BadAccount => "Typed-data params name no valid account".to_owned(),
            Self::BadDocument(why) => format!("Invalid typed data: {why}"),
        }
    }
}

/// The request, read once.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalTypedData {
    pub method: TypedDataMethod,
    /// The account the request names, as written.
    pub account: String,
    /// The one document.
    pub document: Value,
    /// Its one serialization — what the sheet decodes and the digest covers.
    pub document_json: String,
}

impl CanonicalTypedData {
    /// The EIP-712 digest of [`Self::document_json`] — what the passkey signs
    /// (before the Safe's `SafeMessage` wrap).
    ///
    /// # Errors
    /// [`TypedDataRequestError::BadDocument`] when the document does not hash.
    pub fn digest(&self) -> Result<Vec<u8>, TypedDataRequestError> {
        crate::eip712::hash_typed_data(&self.document_json)
            .map_err(|error| TypedDataRequestError::BadDocument(error.to_string()))
    }

    /// The two params, in the method's own order, around the one document.
    #[must_use]
    pub fn params(&self) -> Value {
        let account = Value::String(self.account.clone());
        let document = Value::String(self.document_json.clone());
        match self.method {
            TypedDataMethod::Legacy => Value::Array(vec![document, account]),
            TypedDataMethod::V3 | TypedDataMethod::V4 => Value::Array(vec![account, document]),
        }
    }

    #[must_use]
    pub fn params_json(&self) -> String {
        self.params().to_string()
    }
}

fn is_address(value: &str) -> bool {
    value.len() == 42
        && value.starts_with("0x")
        && value[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// Read `params` for `method`, strictly. See the module docs for the rule.
/// This is the shape; [`CanonicalTypedData::digest`] is what is signed, and a
/// request whose document does not hash is refused at arrival
/// ([`signable`]).
///
/// # Errors
/// [`TypedDataRequestError`] — the request is refused with `-32602`.
pub fn canonical(
    method: &str,
    params: &Value,
) -> Result<CanonicalTypedData, TypedDataRequestError> {
    let method = TypedDataMethod::of(method).ok_or(TypedDataRequestError::UnsupportedMethod)?;
    let list = params
        .as_array()
        .filter(|list| list.len() == 2)
        .ok_or(TypedDataRequestError::NotTwoParams)?;
    let account = list[method.account_index()]
        .as_str()
        .filter(|account| is_address(account))
        .ok_or(TypedDataRequestError::BadAccount)?
        .to_owned();
    let document = match &list[method.document_index()] {
        Value::String(text) => serde_json::from_str::<Value>(text)
            .map_err(|_| TypedDataRequestError::BadDocument("not JSON".to_owned()))?,
        object @ Value::Object(_) => object.clone(),
        _ => {
            return Err(TypedDataRequestError::BadDocument(
                "not a typed-data document".to_owned(),
            ))
        }
    };
    let fields = document
        .as_object()
        .ok_or_else(|| TypedDataRequestError::BadDocument("not an object".to_owned()))?;
    let object = |key: &str| fields.get(key).is_some_and(Value::is_object);
    let primary_type = fields
        .get("primaryType")
        .and_then(Value::as_str)
        .is_some_and(|name| !name.is_empty());
    if !(object("types") && primary_type && object("domain") && object("message")) {
        return Err(TypedDataRequestError::BadDocument(
            "missing types, primaryType, domain or message".to_owned(),
        ));
    }
    let document_json = document.to_string();
    Ok(CanonicalTypedData {
        method,
        account,
        document,
        document_json,
    })
}

/// [`canonical`] and a document that hashes — what a request must be to reach
/// a sheet at all.
///
/// # Errors
/// As [`canonical`], or [`TypedDataRequestError::BadDocument`] when the
/// document does not hash.
pub fn signable(method: &str, params: &Value) -> Result<CanonicalTypedData, TypedDataRequestError> {
    let read = canonical(method, params)?;
    read.digest()?;
    Ok(read)
}

/// [`canonical`] over a params JSON string.
///
/// # Errors
/// As [`canonical`]; params that are not JSON are [`TypedDataRequestError::NotTwoParams`].
pub fn canonical_json(
    method: &str,
    params_json: &str,
) -> Result<CanonicalTypedData, TypedDataRequestError> {
    let params: Value =
        serde_json::from_str(params_json).map_err(|_| TypedDataRequestError::NotTwoParams)?;
    canonical(method, &params)
}

/// [`signable`] over a params JSON string.
///
/// # Errors
/// As [`signable`].
pub fn signable_json(
    method: &str,
    params_json: &str,
) -> Result<CanonicalTypedData, TypedDataRequestError> {
    let params: Value =
        serde_json::from_str(params_json).map_err(|_| TypedDataRequestError::NotTwoParams)?;
    signable(method, &params)
}

/// The document a sheet decodes — the same bytes the digest covers — or
/// `None` when the request is not a valid typed-data request (it is refused).
#[must_use]
pub fn document_json_of(method: &str, params_json: &str) -> Option<String> {
    canonical_json(method, params_json)
        .ok()
        .map(|canonical| canonical.document_json)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    const ACCOUNT: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

    fn mail() -> Value {
        json!({
            "types": {"EIP712Domain": [{"name": "name", "type": "string"}],
                      "Mail": [{"name": "contents", "type": "string"}]},
            "primaryType": "Mail", "domain": {"name": "Ether Mail"},
            "message": {"contents": "Hello"}
        })
    }

    #[test]
    fn the_four_methods_and_their_order() {
        for (method, params) in [
            ("eth_signTypedData_v4", json!([ACCOUNT, mail()])),
            ("eth_signTypedData_v3", json!([ACCOUNT, mail().to_string()])),
            ("eth_signTypedData", json!([mail(), ACCOUNT])),
            ("eth_signTypedData_v1", json!([mail().to_string(), ACCOUNT])),
        ] {
            let read = canonical(method, &params).expect(method);
            assert_eq!(read.account, ACCOUNT);
            assert_eq!(read.document, mail());
            assert_eq!(
                read.digest().unwrap(),
                crate::eip712::hash_typed_data(&mail().to_string()).unwrap()
            );
            // Rebuilt in the method's own order, and stable.
            assert_eq!(canonical(method, &read.params()).unwrap(), read);
        }
    }

    #[test]
    fn two_documents_are_refused_in_either_order() {
        assert_eq!(
            canonical("eth_signTypedData_v4", &json!([mail(), mail()])),
            Err(TypedDataRequestError::BadAccount)
        );
        assert_eq!(
            canonical("eth_signTypedData", &json!([mail(), mail()])),
            Err(TypedDataRequestError::BadAccount)
        );
        assert_eq!(
            canonical("eth_signTypedData_v4", &json!([ACCOUNT, mail(), mail()])),
            Err(TypedDataRequestError::NotTwoParams)
        );
    }

    #[test]
    fn the_wrong_order_one_param_and_other_names_are_refused() {
        assert_eq!(
            canonical("eth_signTypedData_v4", &json!([mail(), ACCOUNT])),
            Err(TypedDataRequestError::BadAccount)
        );
        assert_eq!(
            canonical("eth_signTypedData", &json!([ACCOUNT, mail()])),
            Err(TypedDataRequestError::BadAccount)
        );
        assert_eq!(
            canonical("eth_signTypedData_v4", &json!([mail()])),
            Err(TypedDataRequestError::NotTwoParams)
        );
        assert_eq!(
            canonical("eth_signTypedData_v2", &json!([ACCOUNT, mail()])),
            Err(TypedDataRequestError::UnsupportedMethod)
        );
    }

    #[test]
    fn a_document_must_be_eip712() {
        for bad in [
            json!("not json"),
            json!(42),
            json!([mail()]),
            json!({"primaryType": "Mail", "domain": {}, "message": {}}),
            json!({"types": {}, "primaryType": "", "domain": {}, "message": {}}),
        ] {
            assert!(matches!(
                canonical("eth_signTypedData_v4", &json!([ACCOUNT, bad])),
                Err(TypedDataRequestError::BadDocument(_))
            ));
        }
    }
}
