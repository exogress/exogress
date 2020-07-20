use std::os::raw::{c_char, c_int};

#[repr(C)]
pub struct Config {
    pub num_threads: c_int,
    pub access_token: *mut c_char,
    pub endpoint_id: *mut c_char,
    pub config_file: *mut c_char,
}

#[no_mangle]
pub extern "C" fn spawn(_config: &Config) -> bool {
    return true;
}
