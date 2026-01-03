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
            _message: format!("Failed to import math: {}", e),
        })?;
        globals
            .set_item("math", math)
            .map_err(|e| LanguloError::PythonError {
                _message: format!("Failed to set math in globals: {}", e),
            })?;

        let _ = PYTHON_GLOBALS.set(globals.into());
        Ok(())
    })
}

pub fn eval_python(statements: &[String]) -> LanguloResult<EvalResult> {
    eval_python_internal(statements, PYTHON_GLOBALS.get().unwrap())
}

pub fn eval_python_internal(
    statements: &[String],
    globals: &Py<PyDict>,
) -> LanguloResult<EvalResult> {
    Python::with_gil(|py| {
        let globals = globals.as_ref(py);

        if statements.is_empty() {
            return Err(LanguloError::PythonError {
                _message: "Empty code".into(),
            });
        }

        // Execute all statements except the last
        for stmt in &statements[..statements.len() - 1] {
            py.run(stmt, Some(globals), None)
                .map_err(|e| LanguloError::PythonError {
                    _message: format!("Statement error: {}", e),
                })?;
        }

        // Eval the last statement as an expression
        let last = statements.last().unwrap();
        match py.eval(last, Some(globals), None) {
            Ok(result) => {
                let display = result
                    .str()
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
                _message: format!("{}", e),
            }),
        }
    })
}

pub fn exec_python(code: &str) -> LanguloResult<()> {
    Python::with_gil(|py| {
        let globals = PYTHON_GLOBALS
            .get()
            .ok_or_else(|| LanguloError::PythonError {
                _message: "Python not initialized".into(),
            })?
            .as_ref(py);

        py.run(code, Some(globals), None)
            .map_err(|e| LanguloError::PythonError {
                _message: format!("{}", e),
            })
    })
}

pub fn get_variable(name: &str) -> LanguloResult<Option<String>> {
    Python::with_gil(|py| {
        let globals = PYTHON_GLOBALS
            .get()
            .ok_or_else(|| LanguloError::PythonError {
                _message: "Python not initialized".into(),
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
                _message: format!("Failed to get variable {}: {}", name, e),
            }),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use crate::transpile::transpile;

    /// Isolated test context with its own Python globals.
    /// Each test creates its own TestContext to avoid state pollution.
    struct TestContext {
        globals: Py<PyDict>,
    }

    impl TestContext {
        fn new() -> Self {
            Python::with_gil(|py| {
                let globals = PyDict::new(py);

                // Import math like the real runtime does
                if let Ok(math) = py.import("math") {
                    let _ = globals.set_item("math", math);
                }

                TestContext {
                    globals: globals.into(),
                }
            })
        }

        fn eval_python(&self, statements: &[String]) -> LanguloResult<EvalResult> {
            eval_python_internal(statements, &self.globals)
        }

        fn eval_langulo(&self, source: &str) -> LanguloResult<EvalResult> {
            let ast = parse(source).map_err(|e| LanguloError::PythonError {
                _message: format!("Parse error: {:?}", e),
            })?;
            let python_code = transpile(&ast, source).map_err(|e| LanguloError::PythonError {
                _message: format!("Transpile error: {:?}", e),
            })?;
            println!("Transpiled to:\n{:?}", python_code);
            self.eval_python(&python_code)
        }

        fn eval_langulo_display(&self, source: &str) -> String {
            let result = self.eval_langulo(source).unwrap().display;
            // Simplify function representation
            if result.starts_with("<function") {
                "<function>".to_string()
            } else {
                result
            }
        }
    }

    // ===================
    // Bug regression tests
    // ===================

    #[test]
    fn test_bug_print_in_expression_multiline() {
        let ctx = TestContext::new();
        // This was failing because eval_python couldn't handle statements + expression
        // Error was: SyntaxError: invalid syntax
        let result = ctx.eval_langulo("3 + $(4+2) * 2");
        assert!(result.is_ok(), "Failed with: {:?}", result.err());
        // 4+2 = 6, printed, then 3 + 6 * 2 = 3 + 12 = 15
        assert_eq!(result.unwrap().display, "15");
    }

    // ===================
    // Raw Python tests
    // ===================

    #[test]
    fn test_eval_simple() {
        let ctx = TestContext::new();
        let result = ctx.eval_python(&["1 + 2".into()]).unwrap();
        assert_eq!(result.display, "3");
        assert_eq!(result.py_type, "int");
    }

    #[test]
    fn test_eval_float() {
        let ctx = TestContext::new();
        let result = ctx.eval_python(&["3.14 * 2".into()]).unwrap();
        assert!(result.display.starts_with("6.28"));
        assert_eq!(result.py_type, "float");
    }

    #[test]
    fn test_eval_power() {
        let ctx = TestContext::new();
        let result = ctx.eval_python(&["2 ** 10".into()]).unwrap();
        assert_eq!(result.display, "1024");
    }

    #[test]
    fn test_underscore_variable() {
        let ctx = TestContext::new();
        let _ = ctx.eval_python(&["42".into()]).unwrap();
        let result = ctx.eval_python(&["_ + 1".into()]).unwrap();
        assert_eq!(result.display, "43");
    }

    #[test]
    fn test_math_available() {
        let ctx = TestContext::new();
        let result = ctx.eval_python(&["math.pi".into()]).unwrap();
        assert!(result.display.starts_with("3.14"));
    }

    #[test]
    fn test_multiline_statements_then_expression() {
        let ctx = TestContext::new();
        let result = ctx
            .eval_python(&["x = 10".into(), "y = 20".into(), "x + y".into()])
            .unwrap();
        assert_eq!(result.display, "30");
    }

    // ===================
    // Langulo integration tests
    // ===================

    #[test]
    fn test_langulo_simple_number() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("42"), "42");
        assert_eq!(ctx.eval_langulo_display("3.14"), "3.14");
    }

    #[test]
    fn test_langulo_arithmetic() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("1 + 2"), "3");
        assert_eq!(ctx.eval_langulo_display("10 - 3"), "7");
        assert_eq!(ctx.eval_langulo_display("4 * 5"), "20");
        assert_eq!(ctx.eval_langulo_display("15 / 3"), "5.0");
        assert_eq!(ctx.eval_langulo_display("17 % 5"), "2");
    }

    #[test]
    fn test_langulo_power() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("2 ^ 10"), "1024");
        assert_eq!(ctx.eval_langulo_display("3 ^ 3"), "27");
    }

    #[test]
    fn test_langulo_precedence() {
        let ctx = TestContext::new();
        // 2 + 3 * 4 = 2 + 12 = 14
        assert_eq!(ctx.eval_langulo_display("2 + 3 * 4"), "14");
        // 2 * 3 + 4 = 6 + 4 = 10
        assert_eq!(ctx.eval_langulo_display("2 * 3 + 4"), "10");
        // 2 ^ 3 * 4 = 8 * 4 = 32
        assert_eq!(ctx.eval_langulo_display("2 ^ 3 * 4"), "32");
    }

    #[test]
    fn test_langulo_grouping() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("(2 + 3) * 4"), "20");
        assert_eq!(ctx.eval_langulo_display("2 * (3 + 4)"), "14");
        assert_eq!(ctx.eval_langulo_display("((1 + 2) * (3 + 4))"), "21");
    }

    #[test]
    fn test_langulo_booleans() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("true"), "True");
        assert_eq!(ctx.eval_langulo_display("false"), "False");
        assert_eq!(ctx.eval_langulo_display("true and false"), "False");
        assert_eq!(ctx.eval_langulo_display("true or false"), "True");
        assert_eq!(ctx.eval_langulo_display("not true"), "False");
        assert_eq!(ctx.eval_langulo_display("not false"), "True");
    }

    #[test]
    fn test_langulo_comparisons() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("1 == 1"), "True");
        assert_eq!(ctx.eval_langulo_display("1 == 2"), "False");
        assert_eq!(ctx.eval_langulo_display("1 != 2"), "True");
        assert_eq!(ctx.eval_langulo_display("1 < 2"), "True");
        assert_eq!(ctx.eval_langulo_display("2 > 1"), "True");
        assert_eq!(ctx.eval_langulo_display("1 <= 1"), "True");
        assert_eq!(ctx.eval_langulo_display("1 >= 1"), "True");
    }

    #[test]
    fn test_langulo_print_simple() {
        let ctx = TestContext::new();
        // $3 should print 3 and return 3
        assert_eq!(ctx.eval_langulo_display("$3"), "3");
    }

    #[test]
    fn test_langulo_print_expression() {
        let ctx = TestContext::new();
        // $(1 + 2) should print 3 and return 3
        assert_eq!(ctx.eval_langulo_display("$(1 + 2)"), "3");
    }

    #[test]
    fn test_langulo_print_in_arithmetic() {
        let ctx = TestContext::new();
        // 1 + $2 + 3 = 1 + 2 + 3 = 6 (and prints 2)
        assert_eq!(ctx.eval_langulo_display("1 + $2 + 3"), "6");
    }

    #[test]
    fn test_langulo_nested_print() {
        let ctx = TestContext::new();
        // $($1) should print 1, then print 1 again, return 1
        assert_eq!(ctx.eval_langulo_display("$($1)"), "1");
    }

    #[test]
    fn test_langulo_complex_expression_with_print() {
        let ctx = TestContext::new();
        // (1 + $2) * (3 + $4) = (1 + 2) * (3 + 4) = 3 * 7 = 21
        assert_eq!(ctx.eval_langulo_display("(1 + $2) * (3 + $4)"), "21");
    }

    #[test]
    fn assignments() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("y = 2"), "2");
        assert_eq!(ctx.eval_langulo_display("y"), "2");
        assert_eq!(ctx.eval_langulo_display("3 + (y=4)"), "7");
        assert_eq!(ctx.eval_langulo_display("y"), "4");
    }

    #[test]
    fn print_assignment() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("x $= 1+2"), "3");
        assert_eq!(ctx.eval_langulo_display("x"), "3");
        assert_eq!(ctx.eval_langulo_display("$x = 44"), "44");
    }

    #[test]
    fn function_definition() {
        let ctx = TestContext::new();
        // Define a simple function
        assert_eq!(ctx.eval_langulo_display("double = |x| x * 2"), "<function>");
        assert_eq!(ctx.eval_langulo_display("double(5)"), "10");
        assert_eq!(ctx.eval_langulo_display("double(3)"), "6");
    }

    #[test]
    fn function_two_params() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("add = |a, b| a + b"), "<function>");
        assert_eq!(ctx.eval_langulo_display("add(1, 2)"), "3");
        assert_eq!(ctx.eval_langulo_display("add(10, 20)"), "30");
    }

    #[test]
    fn function_no_params() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("always_five = || 5"), "<function>");
        assert_eq!(ctx.eval_langulo_display("always_five()"), "5");
    }

    #[test]
    fn function_complex_body() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display("calc = |x, y| (x + y) * 2"),
            "<function>"
        );
        assert_eq!(ctx.eval_langulo_display("calc(3, 4)"), "14");
    }

    #[test]
    fn function_nested_calls() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("inc = |x| x + 1"), "<function>");
        assert_eq!(ctx.eval_langulo_display("dec = |x| x - 1"), "<function>");
        assert_eq!(ctx.eval_langulo_display("inc(dec(5))"), "5");
        assert_eq!(ctx.eval_langulo_display("inc(inc(inc(0)))"), "3");
    }

    #[test]
    fn function_in_expression() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("square = |x| x * x"), "<function>");
        assert_eq!(ctx.eval_langulo_display("1 + square(3) + 2"), "12");
        assert_eq!(ctx.eval_langulo_display("square(2) * square(3)"), "36");
    }

    #[test]
    fn postfix_call_simple() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("double = |@| @ * 2"), "<function>");
        assert_eq!(ctx.eval_langulo_display("5 @ double()"), "10");
        assert_eq!(ctx.eval_langulo_display("3 @ double()"), "6");
    }

    #[test]
    fn postfix_call_with_args() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display("plus = |@, n| @ + n"),
            "<function>"
        );
        assert_eq!(ctx.eval_langulo_display("3 @ plus(2)"), "5");
        assert_eq!(ctx.eval_langulo_display("10 @ plus(5)"), "15");
    }

    #[test]
    fn postfix_call_chained() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("inc = |@| @ + 1"), "<function>");
        assert_eq!(ctx.eval_langulo_display("double = |@| @ * 2"), "<function>");
        assert_eq!(ctx.eval_langulo_display("0 @ inc() @ inc() @ inc()"), "3");
        assert_eq!(ctx.eval_langulo_display("2 @ double() @ double()"), "8");
        assert_eq!(ctx.eval_langulo_display("1 @ inc() @ double()"), "4");
    }

    #[test]
    fn postfix_call_mixed_styles() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("add = |a, b| a + b"), "<function>");
        assert_eq!(ctx.eval_langulo_display("inc = |@| @ + 1"), "<function>");
        // Mix prefix and postfix calls
        assert_eq!(ctx.eval_langulo_display("add(1, 2) @ inc()"), "4");
        assert_eq!(ctx.eval_langulo_display("5 @ inc() + 10"), "16");
    }

    #[test]
    fn function_with_print() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("f = |x| $x + 1"), "<function>");
        // When called, $x should print x and return x+1
        assert_eq!(ctx.eval_langulo_display("f(5)"), "6");
    }

    #[test]
    fn function_closure_behavior() {
        let ctx = TestContext::new();
        // Functions should capture variables from outer scope
        assert_eq!(ctx.eval_langulo_display("multiplier = 3"), "3");
        assert_eq!(
            ctx.eval_langulo_display("scale = |x| x * multiplier"),
            "<function>"
        );
        assert_eq!(ctx.eval_langulo_display("scale(4)"), "12");
    }

    #[test]
    fn function_as_argument() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display("apply_twice = |f, x| f(f(x))"),
            "<function>"
        );
        assert_eq!(ctx.eval_langulo_display("inc = |x| x + 1"), "<function>");
        assert_eq!(ctx.eval_langulo_display("apply_twice(inc, 0)"), "2");
    }

    #[test]
    fn function_returning_function() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display("make_adder = |n| |x| x + n"),
            "<function>"
        );
        assert_eq!(
            ctx.eval_langulo_display("addfive = make_adder(5)"),
            "<function>"
        );
        assert_eq!(ctx.eval_langulo_display("addfive(10)"), "15");
    }

    #[test]
    fn postfix_with_expression_arg() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display("plus = |@, n| @ + n"),
            "<function>"
        );
        assert_eq!(ctx.eval_langulo_display("3 @ plus(1 + 1)"), "5");
        assert_eq!(ctx.eval_langulo_display("(1 + 2) @ plus(3 * 2)"), "9");
    }

    #[test]
    fn function_boolean_operations() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display("is_positive = |x| x > 0"),
            "<function>"
        );
        assert_eq!(ctx.eval_langulo_display("is_positive(5)"), "True");
        assert_eq!(ctx.eval_langulo_display("is_positive(-3)"), "False");
        assert_eq!(
            ctx.eval_langulo_display("both_positive = |a, b| a > 0 and b > 0"),
            "<function>"
        );
        assert_eq!(ctx.eval_langulo_display("both_positive(1, 2)"), "True");
        assert_eq!(ctx.eval_langulo_display("both_positive(1, -1)"), "False");
    }

    #[test]
    fn test_block_evaluates_to_last() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("{ 1\n2\n3 }"), "3");
    }

    #[test]
    fn test_block_with_assignments() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("{ x = 1\ny = 2\nx + y }"), "3");
    }

    #[test]
    fn test_block_return_early() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("{ return 42\n99 }"), "42");
    }

    #[test]
    fn test_block_return_middle() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display("{ x = 1\nreturn x + 1\nx + 100 }"),
            "2"
        );
    }

    #[test]
    fn test_block_no_return() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("{ x = 5\nx * 2 }"), "10");
    }

    #[test]
    fn test_block_in_expression() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("1 + { 2 }"), "3");
        assert_eq!(ctx.eval_langulo_display("{ 2 } + { 3 }"), "5");
    }

    #[test]
    fn test_nested_blocks() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("{ { 42 } }"), "42");
        assert_eq!(ctx.eval_langulo_display("{ x = { 1 + 2 }\nx * 2 }"), "6");
    }

    #[test]
    fn test_block_scoping() {
        let ctx = TestContext::new();
        // Variables defined in block should persist (no lexical scoping yet)
        assert_eq!(ctx.eval_langulo_display("{ uux = 42 }"), "42");
        assert_eq!(ctx.eval_langulo_display("uux"), "42");
    }

    #[test]
    fn test_function_with_block() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display("f = |x| { y = x + 1\ny * 2 }"),
            "<function>"
        );
        assert_eq!(ctx.eval_langulo_display("f(5)"), "12");
    }

    #[test]
    fn test_function_with_block_return() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display("g = |x| { return x * 2\nx + 100 }"),
            "<function>"
        );
        assert_eq!(ctx.eval_langulo_display("g(5)"), "10");
    }

    /////////////
    // strings //
    /////////////

    #[test]
    fn test_string_simple() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display(r#""hello""#), "hello");
        assert_eq!(ctx.eval_langulo_display(r#"'hello'"#), "hello");
    }

    #[test]
    fn test_string_concatenation() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display(r#""hello" + " world""#),
            "hello world"
        );
    }

    #[test]
    fn test_string_interpolation_var() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display(r#"name = "Alice""#), "Alice");
        assert_eq!(
            ctx.eval_langulo_display(r#""hello {name}""#),
            "hello Alice"
        );
    }

    #[test]
    fn test_string_interpolation_expr() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display(r#""2 + 2 = {2 + 2}""#),
            "2 + 2 = 4"
        );
    }

    #[test]
    fn test_string_interpolation_multiple() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display(r#"a = 1"#), "1");
        assert_eq!(ctx.eval_langulo_display(r#"b = 2"#), "2");
        assert_eq!(
            ctx.eval_langulo_display(r#""{a} + {b} = {a + b}""#),
            "1 + 2 = 3"
        );
    }

    #[test]
    fn test_string_nested_quotes() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display(r#""hello {'world'}""#),
            "hello world"
        );
    }

    #[test]
    fn test_string_interpolation_nested_string() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display(r#"x = 'inner'"#), "inner");
        assert_eq!(
            ctx.eval_langulo_display(r#""outer {x} end""#),
            "outer inner end"
        );
    }

    #[test]
    fn test_multiline_string() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display(
                r#""""hello
world""""#
            ),
            "hello\nworld"
        );
    }

    #[test]
    fn test_string_escape() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display(r#""hello \"world\"""#),
            r#"hello "world""#
        );
    }

    #[test]
    fn test_string_escape_single() {
        let ctx = TestContext::new();
        assert_eq!(
            ctx.eval_langulo_display(r#"'hello \'world\''"#),
            "hello 'world'"
        );
    }

    #[test]
    fn test_empty_string() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display(r#""""#), "");
        assert_eq!(ctx.eval_langulo_display(r#"''"#), "");
    }

    #[test]
    fn test_string_only_interpolation() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display(r#"x = 42"#), "42");
        assert_eq!(ctx.eval_langulo_display(r#""{x}""#), "42");
    }

    ///////////////
    // options   //
    ///////////////

    #[test]
    fn test_some_basic() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("42!"), "42!");
    }

    #[test]
    fn test_none_basic() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("?"), "?");
    }

    #[test]
    fn test_some_else_returns_value() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("42! else 0"), "42");
    }

    #[test]
    fn test_none_else_returns_default() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("? else 99"), "99");
    }

    #[test]
    fn test_if_true_returns_some() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("if true: 42"), "42!");
    }

    #[test]
    fn test_if_false_returns_none() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("if false: 42"), "?");
    }

    #[test]
    fn test_if_else_true_case() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("if true: 2 else 3"), "2");
    }

    #[test]
    fn test_if_else_false_case() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("if false: 2 else 3"), "3");
    }

    #[test]
    fn test_if_with_complex_condition() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("if (1 < 2): 10 else 20"), "10");
        assert_eq!(ctx.eval_langulo_display("if (1 > 2): 10 else 20"), "20");
    }

    #[test]
    fn test_if_with_complex_body() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("if true: (2 + 3) else 0"), "5");
    }

    #[test]
    fn test_nested_if_else() {
        let ctx = TestContext::new();
        // if false: 1 else if true: 2 else 3
        // -> ? else (if true: 2 else 3) -> 2
        assert_eq!(ctx.eval_langulo_display("if false: 1 else if true: 2 else 3"), "2");
    }

    #[test]
    fn test_option_in_variable() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("x = 5!"), "5!");
        assert_eq!(ctx.eval_langulo_display("x else 0"), "5");
    }

    #[test]
    fn test_none_in_variable() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("y = ?"), "?");
        assert_eq!(ctx.eval_langulo_display("y else 99"), "99");
    }

    #[test]
    fn test_some_with_expression() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("(1 + 2)!"), "3!");
    }

    #[test]
    fn test_chained_else() {
        let ctx = TestContext::new();
        // ? else ? else 42 -> (? else ?) else 42 -> ? else 42 -> 42
        assert_eq!(ctx.eval_langulo_display("? else ? else 42"), "42");
    }

    #[test]
    fn test_map_option_bug() {
        let ctx = TestContext::new();
        assert_eq!(ctx.eval_langulo_display("map = |@, fn| if ask @: fn(@ else ?)"), "<function>");
        assert_eq!(ctx.eval_langulo_display("5! @ map(|x|x+1)"), "6!");
        assert_eq!(ctx.eval_langulo_display("? @ map(|x|x+1)"), "?");
    }
}
