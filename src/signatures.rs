//! Offline authentication of Rust channel manifests under a pinned release key.
use crate::{Error, ErrorKind, Result};
use pgp::{
    composed::{Deserializable, DetachedSignature, SignedPublicKey},
    packet::{Signature, SignatureType},
    types::{KeyDetails, Tag},
};
use serde::{Deserialize, Serialize};

/// Pinned primary fingerprint, independently checked against official Rust sources.
pub const RUST_RELEASE_FINGERPRINT: &str = "108F66205EAEB0AAA8DD5E1C85AB96E6FA1BE5FE";
/// Compiled-in ASCII-armored release certificate. No keyserver is consulted.
pub const RUST_RELEASE_KEY: &str = include_str!("signatures/rust-release-key.asc");
const ARMOR_LIMIT: usize = 64 * 1024;

/// Recorded authentication outcome; an unsigned result requires an explicit opt-out.
#[non_exhaustive]
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationResult {
    /// Original manifest bytes have a valid signature by the pinned certificate.
    Verified,
    /// No signature was supplied and unsigned historical manifests were explicitly allowed.
    AllowedUnsigned,
}

/// Authentication evidence for one original manifest, not a substitute for re-verification.
#[non_exhaustive]
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManifestVerification {
    /// Authentication result.
    pub result: VerificationResult,
    /// Whether the user explicitly permitted missing signatures for this manifest.
    pub allow_unsigned_manifest: bool,
    /// Pinned primary fingerprint, absent for unsigned manifests.
    pub primary_fingerprint: Option<String>,
    /// Fingerprint of the actual signing primary key or subkey.
    pub signer_fingerprint: Option<String>,
    /// Authenticated signature creation time as Unix seconds.
    pub signature_created_at: Option<u64>,
}
fn refusal(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    Error::new(ErrorKind::Signature, message).into()
}
fn key(armor: &[u8], fingerprint: &str) -> Result<SignedPublicKey> {
    if armor.is_empty() || armor.len() > ARMOR_LIMIT {
        return Err(refusal("release key armor is missing or oversized"));
    }
    let (mut keys, _) = SignedPublicKey::from_armor_many(armor)
        .map_err(|_| refusal("malformed release key armor"))?;
    let key = keys
        .next()
        .ok_or_else(|| refusal("missing release key"))?
        .map_err(|_| refusal("malformed release key packets"))?;
    if keys.next().is_some() || hex::encode_upper(key.fingerprint().as_bytes()) != fingerprint {
        return Err(refusal(
            "release key primary fingerprint differs from pinned trust anchor",
        ));
    }
    Ok(key)
}
/// Validate a refreshed release certificate without changing the pinned primary identity.
/// This performs no network access and accepts no alternate trust anchor.
pub fn validate_key(armor: &[u8]) -> Result<()> {
    key(armor, RUST_RELEASE_FINGERPRINT).map(|_| ())
}
fn active(sig: &Signature, at: u64) -> bool {
    sig.created().is_some_and(|t| {
        let created = u64::from(t.as_secs());
        created <= at
            && sig
                .signature_expiration_time()
                .is_none_or(|d| d.as_secs() == 0 || at < created + u64::from(d.as_secs()))
    })
}
fn latest<'a>(signatures: impl Iterator<Item = &'a Signature>, at: u64) -> Result<&'a Signature> {
    signatures
        .filter(|s| active(s, at))
        .max_by_key(|s| s.created())
        .ok_or_else(|| refusal("no valid key binding at manifest signing time"))
}
fn lifetime(created: u64, binding: &Signature, at: u64) -> Result<()> {
    if at < created
        || binding
            .key_expiration_time()
            .is_some_and(|d| d.as_secs() != 0 && at >= created + u64::from(d.as_secs()))
    {
        return Err(refusal(
            "release key or subkey was expired or not yet valid at signing time",
        ));
    }
    Ok(())
}
/// Authenticate original manifest bytes with the compiled-in Rust release certificate.
/// An explicit opt-out permits only a missing signature, never an invalid present signature.
pub fn verify(
    manifest: &[u8],
    signature: &[u8],
    allow_unsigned_manifest: bool,
) -> Result<ManifestVerification> {
    verify_with_key(
        manifest,
        signature,
        allow_unsigned_manifest,
        RUST_RELEASE_KEY.as_bytes(),
    )
}
/// Authenticate with a refreshed copy of the same pinned primary certificate.
/// No network access or external programs are used; a different primary key is refused.
pub fn verify_with_key(
    manifest: &[u8],
    signature: &[u8],
    allow_unsigned_manifest: bool,
    armor: &[u8],
) -> Result<ManifestVerification> {
    verify_pinned(
        manifest,
        signature,
        allow_unsigned_manifest,
        armor,
        RUST_RELEASE_FINGERPRINT,
    )
}
fn verify_pinned(
    manifest: &[u8],
    signature: &[u8],
    allow_unsigned_manifest: bool,
    armor: &[u8],
    fingerprint: &str,
) -> Result<ManifestVerification> {
    if signature.is_empty() {
        if !allow_unsigned_manifest {
            return Err(refusal("missing manifest .asc; unsigned historical manifests require --allow-unsigned-manifest"));
        }
        return Ok(ManifestVerification {
            result: VerificationResult::AllowedUnsigned,
            allow_unsigned_manifest,
            primary_fingerprint: None,
            signer_fingerprint: None,
            signature_created_at: None,
        });
    }
    if signature.len() > ARMOR_LIMIT || manifest.len() > 4 * 1024 * 1024 {
        return Err(refusal(
            "manifest or armored signature exceeds verification limits",
        ));
    }
    let key = key(armor, fingerprint)?;
    let (mut signatures, _) = DetachedSignature::from_armor_many(signature)
        .map_err(|_| refusal("malformed manifest signature armor"))?;
    let sig = signatures
        .next()
        .ok_or_else(|| refusal("missing detached signature"))?
        .map_err(|_| refusal("malformed detached signature packets"))?;
    if signatures.next().is_some() || sig.signature.typ() != Some(SignatureType::Binary) {
        return Err(refusal(
            "expected exactly one binary detached manifest signature",
        ));
    }
    let at = u64::from(
        sig.signature
            .created()
            .ok_or_else(|| refusal("manifest signature has no authenticated creation time"))?
            .as_secs(),
    );
    let primary = &key.primary_key;
    if key.details.revocation_signatures.iter().any(|s| {
        s.created().is_some_and(|t| u64::from(t.as_secs()) <= at) && s.verify_key(primary).is_ok()
    }) {
        return Err(refusal("release primary key was revoked at signing time"));
    }
    let certifications = key.details.users.iter().flat_map(|u| {
        u.signatures
            .iter()
            .filter(|s| s.verify_certification(primary, Tag::UserId, &u.id).is_ok())
    });
    let direct = key
        .details
        .direct_signatures
        .iter()
        .filter(|s| s.verify_key(primary).is_ok());
    let binding = latest(certifications.chain(direct), at)?;
    lifetime(u64::from(primary.created_at().as_secs()), binding, at)?;
    let signer = if sig.verify(primary, manifest).is_ok() {
        if !binding.key_flags().sign() {
            return Err(refusal("primary key is not authorized to sign manifests"));
        }
        hex::encode_upper(primary.fingerprint().as_bytes())
    } else {
        let sub = key
            .public_subkeys
            .iter()
            .find(|s| sig.verify(&s.key, manifest).is_ok())
            .ok_or_else(|| {
                refusal("manifest signature is invalid or made by a different signer")
            })?;
        if sub.signatures.iter().any(|s| {
            s.typ() == Some(SignatureType::SubkeyRevocation)
                && s.created().is_some_and(|t| u64::from(t.as_secs()) <= at)
                && s.verify_subkey_binding(primary, &sub.key).is_ok()
        }) {
            return Err(refusal(
                "release signing subkey was revoked at signing time",
            ));
        }
        let binding = latest(
            sub.signatures.iter().filter(|s| {
                s.typ() == Some(SignatureType::SubkeyBinding)
                    && s.verify_subkey_binding(primary, &sub.key).is_ok()
            }),
            at,
        )?;
        if !binding.key_flags().sign() {
            return Err(refusal("subkey is not authorized to sign manifests"));
        }
        binding
            .embedded_signature()
            .ok_or_else(|| refusal("missing signing subkey cross-certification"))?
            .verify_primary_key_binding(&sub.key, primary)
            .map_err(|_| refusal("invalid signing subkey cross-certification"))?;
        lifetime(u64::from(sub.key.created_at().as_secs()), binding, at)?;
        hex::encode_upper(sub.key.fingerprint().as_bytes())
    };
    Ok(ManifestVerification {
        result: VerificationResult::Verified,
        allow_unsigned_manifest,
        primary_fingerprint: Some(fingerprint.into()),
        signer_fingerprint: Some(signer),
        signature_created_at: Some(at),
    })
}
