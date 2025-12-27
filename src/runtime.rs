//! Python runtime for executing transpiled Languria code
//!
//! Uses PyO3 to embed Python and maintain a persistent namespace
//! for variable storage across REPL iterations.

use crate::errors::{LanguriaError, LanguriaResult};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::sync::OnceLock;

static PYTHON_GLOBALS: OnceLock<Py<PyDict>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct EvalResult {
    pub display: String,
    pub py_type: String,
}

pub fn init_python() -> LanguriaResult<()> {
    Python::with_gil(|py| {
        let globals = PyDict::new(py);

        let math = py.import("math").map_err(|e| LanguriaError::PythonError {
            message: format!("Failed to import math: {}", e),
        })?;
        globals
            .set_item("math", math)
            .map_err(|e| LanguriaError::PythonError {
                message: format!("Failed to set math in globals: {}", e),
            })?;

        let _ = PYTHON_GLOBALS.set(globals.into());
        Ok(())
    })
}

pub fn eval_python(code: &str) -> LanguriaResult<EvalResult> {
    Python::with_gil(|py| {
        let globals = PYTHON_GLOBALS
            .get()
            .ok_or_else(|| LanguriaError::PythonError {
                message: "Python not initialized".into(),
            })?
            .as_ref(py);

        match py.eval(code, Some(globals), None) {
            Ok(result) => {
                let display = result
                    .repr()
                    .map(|r| r.to_string())
                    .unwrap_or_else(|_| "<unable to display>".into());

                let py_type = result
                    .get_type()
                    .name()
                    .map(|n| n.into())
                    .unwrap_or_else(|_| "unknown".into());

                let _ = globals.set_item("_", result);

                Ok(EvalResult { display, py_type })
            }
            Err(e) => Err(LanguriaError::PythonError {
                message: format!("{}", e),
            }),
        }
    })
}

pub fn exec_python(code: &str) -> LanguriaResult<()> {
    Python::with_gil(|py| {
        let globals = PYTHON_GLOBALS
            .get()
            .ok_or_else(|| LanguriaError::PythonError {
                message: "Python not initialized".into(),
            })?
            .as_ref(py);

        py.run(code, Some(globals), None)
            .map_err(|e| LanguriaError::PythonError {
                message: format!("{}", e),
            })
    })
}

pub fn get_variable(name: &str) -> LanguriaResult<Option<String>> {
    Python::with_gil(|py| {
        let globals = PYTHON_GLOBALS
            .get()
            .ok_or_else(|| LanguriaError::PythonError {
                message: "Python not initialized".into(),
            })?
            .as_ref(py);

        match globals.get_item(name) {
            Ok(Some(value)) => Ok(Some(
                value
                    .repr()
                    .map(|r| r.to_string())
                    .unwrap_or_else(|_| "<unable to display>".into()),
            )),
            Ok(None) => Ok(None),
            Err(e) => Err(LanguriaError::PythonError {
                message: format!("Failed to get variable {}: {}", name, e),
            }),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() {
        let _ = init_python();
    }

    #[test]
    fn test_eval_simple() {
        setup();
        let result = eval_python("1 + 2").unwrap();
        assert_eq!(result.display, "3");
        assert_eq!(result.py_type, "int");
    }

    #[test]
    fn test_eval_float() {
        setup();
        let result = eval_python("3.14 * 2").unwrap();
        assert!(result.display.starts_with("6.28"));
        assert_eq!(result.py_type, "float");
    }

    #[test]
    fn test_eval_power() {
        setup();
        let result = eval_python("2 ** 10").unwrap();
        assert_eq!(result.display, "1024");
    }

    #[test]
    fn test_underscore_variable() {
        setup();
        let _ = eval_python("42").unwrap();
        let result = eval_python("_ + 1").unwrap();
        assert_eq!(result.display, "43");
    }

    #[test]
    fn test_math_available() {
        setup();
        let result = eval_python("math.pi").unwrap();
        assert!(result.display.starts_with("3.14"));
    }
}