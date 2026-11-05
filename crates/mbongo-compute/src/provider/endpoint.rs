//! Where a provider is, and who may vouch for it: `provider-endpoint-v1`.
//!
//! The client holds one file per provider, given out of band. It names the
//! executor, the URL, and the certificate authorities trusted for this provider
//! alone. Nothing here ties the certificate to the executor key: TLS proves a
//! host name, and the chain proves the executor.

use std::path::{Component, Path, PathBuf};

use mbongo_core::Address;
use reqwest::Url;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::CertificateDer;
use rustls::RootCertStore;
use serde::Deserialize;

/// The file's schema.
pub const ENDPOINT_SCHEMA: &str = "provider-endpoint-v1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EndpointFile {
    schema: String,
    executor: String,
    url: String,
    /// PEM, beside this file: it cannot name a bundle kept elsewhere.
    trust_anchors: PathBuf,
}

/// Why an endpoint file was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("PROVIDER_ENDPOINT_INVALID: {0}")]
pub struct EndpointError(String);

fn invalid(detail: impl Into<String>) -> EndpointError {
    EndpointError(detail.into())
}

/// A provider's endpoint, as this client was told.
#[derive(Debug, Clone)]
pub struct ProviderEndpoint {
    executor: Address,
    url: Url,
    roots: RootCertStore,
}

impl ProviderEndpoint {
    /// Loads and checks an endpoint file.
    pub fn load(path: &Path) -> Result<Self, EndpointError> {
        let bytes = std::fs::read(path).map_err(|e| invalid(format!("{:?}", e.kind())))?;
        let file: EndpointFile =
            serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
        if file.schema != ENDPOINT_SCHEMA {
            return Err(invalid("unknown schema"));
        }
        let executor = hex::decode(&file.executor)
            .ok()
            .and_then(|b| <[u8; 32]>::try_from(b).ok())
            .ok_or_else(|| invalid("executor is not 32 bytes of hex"))?;
        let url = Url::parse(&file.url).map_err(|_| invalid("url"))?;
        let bare = url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none()
            && url.username().is_empty()
            && url.password().is_none();
        if url.scheme() != "https" || url.host().is_none() || !bare {
            return Err(invalid("url must be https, with a host and nothing else"));
        }
        let beside = file.trust_anchors.components().all(|c| matches!(c, Component::Normal(_)));
        if !beside {
            return Err(invalid("trust_anchors must sit beside the endpoint file"));
        }
        let anchors_file = path.parent().unwrap_or(Path::new(".")).join(file.trust_anchors);
        let pem =
            std::fs::read(anchors_file).map_err(|e| invalid(format!("anchors: {:?}", e.kind())))?;
        let mut roots = RootCertStore::empty();
        for certificate in CertificateDer::pem_slice_iter(&pem) {
            let certificate = certificate.map_err(|_| invalid("anchors"))?;
            roots.add(certificate).map_err(|_| invalid("anchors"))?;
        }
        if roots.is_empty() {
            return Err(invalid("no trust anchors"));
        }
        Ok(Self {
            executor: Address(executor),
            url,
            roots,
        })
    }

    /// The executor this endpoint is for.
    pub fn executor(&self) -> Address {
        self.executor
    }

    /// The base URL, with no trailing slash.
    pub fn base(&self) -> String {
        self.url.as_str().trim_end_matches('/').to_owned()
    }

    /// The certificate authorities trusted for this provider, and no others.
    pub fn roots(&self) -> &RootCertStore {
        &self.roots
    }
}
