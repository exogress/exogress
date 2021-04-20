pub fn load_native_certs_safe(tls_config: &mut rustls::ClientConfig) {
    let roots = match rustls_native_certs::load_native_certs() {
        Ok(store) | Err((Some(store), _)) => store.roots,
        Err((None, _)) => Vec::new(),
    };
    for root in roots {
        tls_config.root_store.roots.push(root);
    }
}
