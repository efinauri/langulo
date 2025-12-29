//! Python runtime for executing transpiled Langulo code
//!
//! Uses PyO3 to embed Python and maintain a persistent namespace
//! for variable storage across REPL iterations.

use crate::errors::{LanguloError, LanguloResult};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::sync::OnceLock;

static PYTHON_GLOBALS: OnceLock<Py<PyDict>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct EvalResult {
    pub display: String,
    pub py_type: String,
}

pub fn init_python() -> LanguloResult<()> {
    Python::with_gil(|py| {
        let globals = PyDict::new(py);

        let math = py.import("math").map_err(|e| LanguloError::PythonError {
            message: format!("Failed to import math: {}", e),
        })?;
        globals
            .set_item("math", math)
            .map_err(|e| LanguloError::PythonError {
                message: format!("Failed to set math in globals: {}", e),
            })?;

        let _ = PYTHON_GLOBALS.set(globals.into());
        Ok(())
    })
}

pub fn eval_python(code: &str) -> LanguloResult<EvalResult> {
    Python::with_gil(|py| {
        let globals = PYTHON_GLOBALS
            .get()
            .ok_or_else(|| LanguloError::PythonError {
                message: "Python not initialized".into(),
            })?
            .as_ref(py);

        // Split into statements and final expression
        let lines: Vec<&str> = code.lines().collect();

        if lines.is_empty() {
            return Err(LanguloError::PythonError {
                message: "Empty code".into(),
            });
        }

        // Run all lines except the last as statements
        if lines.len() > 1 {
            let statements = lines[..lines.len() - 1].join("\n");
            py.run(&statements, Some(globals), None)
                .map_err(|e| LanguloError::PythonError {
                    message: format!("Statement error: {}", e),
                })?;
        }

        // Eval the last line as an expression
        let expr = lines.last().unwrap();
        match py.eval(expr, Some(globals), None) {
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
            Err(e) => Err(LanguloError::PythonError {
                message: format!("{}", e),
            }),
        }
    })
}

pub fn exec_python(code: &str) -> LanguloResult<()> {
    Python::with_gil(|py| {
        let globals = PYTHON_GLOBALS
            .get()
            .ok_or_else(|| LanguloError::PythonError {
                message: "Python not initialized".into(),
            })?
            .as_ref(py);

        py.run(code, Some(globals), None)
            .map_err(|e| LanguloError::PythonError {
                message: format!("{}", e),
            })
    })
}

pub fn get_variable(name: &str) -> LanguloResult<Option<String>> {
    Python::with_gil(|py| {
        let globals = PYTHON_GLOBALS
            .get()
            .ok_or_else(|| LanguloError::PythonError {
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
            Err(e) => Err(LanguloError::PythonError {
                message: format!("Failed to get variable {}: {}", name, e),
            }),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use crate::transpile::transpile;

    fn setup() {
        let _ = init_python();
    }

    /// Helper: transpile Langulo source and evaluate it
    fn eval_langulo(source: &str) -> LanguloResult<EvalResult> {
        setup();
        let ast = parse(source).map_err(|e| LanguloError::PythonError {
            message: format!("Parse error: {:?}", e),
        })?;
        let python_code = transpile(&ast).map_err(|e| LanguloError::PythonError {
            message: format!("Transpile error: {:?}", e),
        })?;
        eval_python(&python_code)
    }

    /// Helper: transpile and evaluate, return the display string
    fn eval_langulo_display(source: &str) -> String {
        let result = eval_langulo(source).unwrap().display;
        // Simplify function representation
        if result.starts_with("<function") {
            "<function>".to_string()
        } else {
            result
        }
    }

    // ===================
    // Bug regression tests
    // ===================

    #[test]
    fn test_bug_print_in_expression_multiline() {
        // This was failing because eval_python couldn't handle statements + expression
        // Error was: SyntaxError: invalid syntax
        setup();
        let result = eval_langulo("3 + $(4+2) * 2");
        assert!(result.is_ok(), "Failed with: {:?}", result.err());
        // 4+2 = 6, printed, then 3 + 6 * 2 = 3 + 12 = 15
        assert_eq!(result.unwrap().display, "15");
    }

    // ===================
    // Raw Python tests
    // ===================

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

    #[test]
    fn test_multiline_statements_then_expression() {
        setup();
        let code = "x = 10\ny = 20\nx + y";
        let result = eval_python(code).unwrap();
        assert_eq!(result.display, "30");
    }

    // ===================
    // Langulo integration tests
    // ===================

    #[test]
    fn test_langulo_simple_number() {
        assert_eq!(eval_langulo_display("42"), "42");
        assert_eq!(eval_langulo_display("3.14"), "3.14");
    }

    #[test]
    fn test_langulo_arithmetic() {
        assert_eq!(eval_langulo_display("1 + 2"), "3");
        assert_eq!(eval_langulo_display("10 - 3"), "7");
        assert_eq!(eval_langulo_display("4 * 5"), "20");
        assert_eq!(eval_langulo_display("15 / 3"), "5.0");
        assert_eq!(eval_langulo_display("17 % 5"), "2");
    }

    #[test]
    fn test_langulo_power() {
        assert_eq!(eval_langulo_display("2 ^ 10"), "1024");
        assert_eq!(eval_langulo_display("3 ^ 3"), "27");
    }

    #[test]
    fn test_langulo_precedence() {
        // 2 + 3 * 4 = 2 + 12 = 14
        assert_eq!(eval_langulo_display("2 + 3 * 4"), "14");
        // 2 * 3 + 4 = 6 + 4 = 10
        assert_eq!(eval_langulo_display("2 * 3 + 4"), "10");
        // 2 ^ 3 * 4 = 8 * 4 = 32
        assert_eq!(eval_langulo_display("2 ^ 3 * 4"), "32");
    }

    #[test]
    fn test_langulo_grouping() {
        assert_eq!(eval_langulo_display("(2 + 3) * 4"), "20");
        assert_eq!(eval_langulo_display("2 * (3 + 4)"), "14");
        assert_eq!(eval_langulo_display("((1 + 2) * (3 + 4))"), "21");
    }

    #[test]
    fn test_langulo_booleans() {
        assert_eq!(eval_langulo_display("true"), "True");
        assert_eq!(eval_langulo_display("false"), "False");
        assert_eq!(eval_langulo_display("true and false"), "False");
        assert_eq!(eval_langulo_display("true or false"), "True");
        assert_eq!(eval_langulo_display("not true"), "False");
        assert_eq!(eval_langulo_display("not false"), "True");
    }

    #[test]
    fn test_langulo_comparisons() {
        assert_eq!(eval_langulo_display("1 == 1"), "True");
        assert_eq!(eval_langulo_display("1 == 2"), "False");
        assert_eq!(eval_langulo_display("1 != 2"), "True");
        assert_eq!(eval_langulo_display("1 < 2"), "True");
        assert_eq!(eval_langulo_display("2 > 1"), "True");
        assert_eq!(eval_langulo_display("1 <= 1"), "True");
        assert_eq!(eval_langulo_display("1 >= 1"), "True");
    }

    #[test]
    fn test_langulo_print_simple() {
        // $3 should print 3 and return 3
        assert_eq!(eval_langulo_display("$3"), "3");
    }

    #[test]
    fn test_langulo_print_expression() {
        // $(1 + 2) should print 3 and return 3
        assert_eq!(eval_langulo_display("$(1 + 2)"), "3");
    }

    #[test]
    fn test_langulo_print_in_arithmetic() {
        // 1 + $2 + 3 = 1 + 2 + 3 = 6 (and prints 2)
        assert_eq!(eval_langulo_display("1 + $2 + 3"), "6");
    }

    #[test]
    fn test_langulo_nested_print() {
        // $($1) should print 1, then print 1 again, return 1
        assert_eq!(eval_langulo_display("$($1)"), "1");
    }

    #[test]
    fn test_langulo_complex_expression_with_print() {
        // (1 + $2) * (3 + $4) = (1 + 2) * (3 + 4) = 3 * 7 = 21
        assert_eq!(eval_langulo_display("(1 + $2) * (3 + $4)"), "21");
    }

    #[test]
    fn assignments() {
        assert_eq!(eval_langulo_display("y = 2"), "2");
        assert_eq!(eval_langulo_display("y"), "2");
        assert_eq!(eval_langulo_display("3 + (y=4)"), "7");
        assert_eq!(eval_langulo_display("y"), "4");
    }

    #[test]
    fn print_assignment() {
        assert_eq!(eval_langulo_display("x $= 1+2"), "3");
        assert_eq!(eval_langulo_display("x"), "3");
        assert_eq!(eval_langulo_display("$x = 44"), "44");
    }

    #[test]
    fn function_definition() {
        // Define a simple function
        assert_eq!(eval_langulo_display("double = |x| x * 2"), "<function>");
        assert_eq!(eval_langulo_display("double(5)"), "10");
        assert_eq!(eval_langulo_display("double(3)"), "6");
    }

    #[test]
    fn function_two_params() {
        assert_eq!(eval_langulo_display("add = |a, b| a + b"), "<function>");
        assert_eq!(eval_langulo_display("add(1, 2)"), "3");
        assert_eq!(eval_langulo_display("add(10, 20)"), "30");
    }

    #[test]
    fn function_no_params() {
        assert_eq!(eval_langulo_display("always_five = || 5"), "<function>");
        assert_eq!(eval_langulo_display("always_five()"), "5");
    }

    #[test]
    fn function_complex_body() {
        assert_eq!(
            eval_langulo_display("calc = |x, y| (x + y) * 2"),
            "<function>"
        );
        assert_eq!(eval_langulo_display("calc(3, 4)"), "14");
    }

    #[test]
    fn function_nested_calls() {
        assert_eq!(eval_langulo_display("inc = |x| x + 1"), "<function>");
        assert_eq!(eval_langulo_display("dec = |x| x - 1"), "<function>");
        assert_eq!(eval_langulo_display("inc(dec(5))"), "5");
        assert_eq!(eval_langulo_display("inc(inc(inc(0)))"), "3");
    }

    #[test]
    fn function_in_expression() {
        assert_eq!(eval_langulo_display("square = |x| x * x"), "<function>");
        assert_eq!(eval_langulo_display("1 + square(3) + 2"), "12");
        assert_eq!(eval_langulo_display("square(2) * square(3)"), "36");
    }

    #[test]
    fn postfix_call_simple() {
        assert_eq!(eval_langulo_display("double = |@| @ * 2"), "<function>");
        assert_eq!(eval_langulo_display("5 @ double()"), "10");
        assert_eq!(eval_langulo_display("3 @ double()"), "6");
    }

    #[test]
    fn postfix_call_with_args() {
        assert_eq!(eval_langulo_display("plus = |@, n| @ + n"), "<function>");
        assert_eq!(eval_langulo_display("3 @ plus(2)"), "5");
        assert_eq!(eval_langulo_display("10 @ plus(5)"), "15");
    }

    #[test]
    fn postfix_call_chained() {
        assert_eq!(eval_langulo_display("inc = |@| @ + 1"), "<function>");
        assert_eq!(eval_langulo_display("double = |@| @ * 2"), "<function>");
        assert_eq!(eval_langulo_display("0 @ inc() @ inc() @ inc()"), "3");
        assert_eq!(eval_langulo_display("2 @ double() @ double()"), "8");
        assert_eq!(eval_langulo_display("1 @ inc() @ double()"), "4");
    }

    #[test]
    fn postfix_call_mixed_styles() {
        assert_eq!(eval_langulo_display("add = |a, b| a + b"), "<function>");
        assert_eq!(eval_langulo_display("inc = |@| @ + 1"), "<function>");
        // Mix prefix and postfix calls
        assert_eq!(eval_langulo_display("add(1, 2) @ inc()"), "4");
        assert_eq!(eval_langulo_display("5 @ inc() + 10"), "16");
    }

    #[test]
    fn function_with_print() {
        assert_eq!(eval_langulo_display("f = |x| $x + 1"), "<function>");
        // When called, $x should print x and return x+1
        assert_eq!(eval_langulo_display("f(5)"), "6");
    }

    #[test]
    fn function_closure_behavior() {
        // Functions should capture variables from outer scope
        assert_eq!(eval_langulo_display("multiplier = 3"), "3");
        assert_eq!(
            eval_langulo_display("scale = |x| x * multiplier"),
            "<function>"
        );
        assert_eq!(eval_langulo_display("scale(4)"), "12");
    }

    #[test]
    fn function_as_argument() {
        assert_eq!(
            eval_langulo_display("apply_twice = |f, x| f(f(x))"),
            "<function>"
        );
        assert_eq!(eval_langulo_display("inc = |x| x + 1"), "<function>");
        assert_eq!(eval_langulo_display("apply_twice(inc, 0)"), "2");
    }

    #[test]
    fn function_returning_function() {
        assert_eq!(
            eval_langulo_display("make_adder = |n| |x| x + n"),
            "<function>"
        );
        assert_eq!(eval_langulo_display("addfive = make_adder(5)"), "<function>");
        assert_eq!(eval_langulo_display("addfive(10)"), "15");
    }

    #[test]
    fn postfix_with_expression_arg() {
        assert_eq!(eval_langulo_display("plus = |@, n| @ + n"), "<function>");
        assert_eq!(eval_langulo_display("3 @ plus(1 + 1)"), "5");
        assert_eq!(eval_langulo_display("(1 + 2) @ plus(3 * 2)"), "9");
    }

    #[test]
    fn function_boolean_operations() {
        assert_eq!(
            eval_langulo_display("is_positive = |x| x > 0"),
            "<function>"
        );
        assert_eq!(eval_langulo_display("is_positive(5)"), "True");
        assert_eq!(eval_langulo_display("is_positive(-3)"), "False");
        assert_eq!(
            eval_langulo_display("both_positive = |a, b| a > 0 and b > 0"),
            "<function>"
        );
        assert_eq!(eval_langulo_display("both_positive(1, 2)"), "True");
        assert_eq!(eval_langulo_display("both_positive(1, -1)"), "False");
    }
}
