use lazy_static::lazy_static;
use rustls::OwnedTrustAnchor;

lazy_static! {
    pub static ref NATIVE_CERTS: Vec<OwnedTrustAnchor> = {
        let roots = match rustls_native_certs::load_native_certs() {
            Ok(store) | Err((Some(store), _)) => store.roots,
            Err((None, _)) => Vec::new(),
        };
        roots
    };
}

pub fn load_native_certs_safe(tls_config: &mut rustls::ClientConfig) {
    for root in &*NATIVE_CERTS {
        tls_config.root_store.roots.push(root.clone());
    }
}
