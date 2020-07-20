use std::thread;

use exogress_tunnel::ClientId;
use pyo3::prelude::*;
use pyo3::types::IntoPyDict;
use pyo3::wrap_pyfunction;
use slog::{Drain, OwnedKVList, Record};

pub struct PyLogger {
    logger_name: String,
}

impl slog::Drain for PyLogger {
    type Ok = ();
    type Err = ();

    fn log(&self, record: &Record, _values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let locals = [("logging", py.import("logging").unwrap())].into_py_dict(py);

        let msg = record.msg().to_string();
        let level = match record.level() {
            slog::Level::Critical => "critical",
            slog::Level::Error => "error",
            slog::Level::Warning => "warning",
            slog::Level::Info => "info",
            slog::Level::Debug => "debug",
            slog::Level::Trace => "debug",
        };

        let logger = py
            .eval(
                &format!("logging.getLogger('{}')", self.logger_name),
                None,
                Some(&locals),
            )
            .map_err(|e| {
                e.print(py);
            })
            .unwrap();

        logger
            .call_method("setLevel", ("INFO",), None)
            .map_err(|e| {
                e.print(py);
            })
            .expect("FIXME");

        logger
            .call_method(level, (msg,), None)
            .map_err(|e| {
                e.print(py);
            })
            .expect("FIXME");

        Ok(())
    }
}

#[pyfunction]
fn spawn(py: Python, logger_name: &str) {
    let py_logger = PyLogger {
        logger_name: logger_name.to_string(),
    };

    let log = slog::Logger::root(py_logger.fuse(), o!());

    exogress_client_lib::spawn(log)
}

/// A Python module implemented in Rust.
#[pymodule]
fn exogress(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(spawn))?;

    Ok(())
}
