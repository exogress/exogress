use node_bindgen::derive::node_bindgen;

#[node_bindgen]
fn spawn(client_id: String, client_secret: String, account: String, project: String) {
    exogress_client_lib::spawn(client_id, client_secret, account, project);
}
