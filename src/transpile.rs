use crate::errors::{LanguloError, LanguloResult};
use crate::parser::{has_print_marker, AstNode, LanguloSyntaxNode};
use std::collections::HashSet;
use std::string::ToString;

const TMP_VAR_NAME: &'static str = "tmp";

pub fn transpile(ast: &LanguloSyntaxNode) -> LanguloResult<String> {
    let mut transpiler = Transpiler::new();
    transpiler.visit(ast)?;
    Ok(transpiler.finish())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum HelperFunction {
    Print,
    Return,
}

impl HelperFunction {
    fn definition(&self) -> &'static str {
        match self {
            HelperFunction::Print => "def _print(x):\n    print(x)\n    return x",
            HelperFunction::Return => "class _Return(Exception):\n    def __init__(self, v): self.value = v",
        }
    }
}

struct Scope {
    lines: Vec<String>,
    current_line: String,
    checkpoints: Vec<usize>,
}

impl Scope {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_line: String::new(),
            checkpoints: Vec::new(),
        }
    }
}

struct PythonEmitter {
    scopes: Vec<Scope>,
    indent: usize,
    helpers: HashSet<HelperFunction>,
    /// a counter to make sure that generated variables/function names are unique
    tmp_counter: usize,
    /// an index of current_line marked for future modification
    checkpoints: Vec<usize>,
}

impl PythonEmitter {
    fn new() -> Self {
        Self {
            scopes: vec![Scope::new()],
            indent: 0,
            helpers: HashSet::new(),
            tmp_counter: 0,
            checkpoints: Vec::new(),
        }
    }

    fn current_scope(&self) -> &Scope {
        self.scopes.last().expect("No active scope")
    }

    fn current_scope_mut(&mut self) -> &mut Scope {
        self.scopes.last_mut().expect("No active scope")
    }

    fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn pop_scope(&mut self) -> Scope {
        assert!(self.scopes.len() > 1);
        self.scopes.pop().expect("Cannot pop last scope")
    }

    fn write(&mut self, code: &str) {
        self.current_scope_mut().current_line.push_str(code);
    }

    fn indentation(&self) -> String {
        "    ".repeat(self.indent)
    }

    fn newline(&mut self) {
        let indent = self.indentation();
        let scope = self.current_scope_mut();
        assert!(!scope.current_line.is_empty());
        assert!(scope.checkpoints.is_empty());
        scope
            .lines
            .push(format!("{}{}", indent, scope.current_line));
        scope.current_line.clear();
    }

    fn emit_line(&mut self, line: &str) {
        let indent = self.indentation();
        self.current_scope_mut()
            .lines
            .push(format!("{}{}", indent, line));
    }

    fn increase_indentation(&mut self) {
        self.indent += 1;
    }

    fn decrease_indentation(&mut self) {
        assert!(self.indent > 0);
        self.indent -= 1;
    }

    fn require_helper(&mut self, helper: HelperFunction) {
        self.helpers.insert(helper);
    }

    fn fresh_tmp(&mut self) -> String {
        let name = format!("{}{}", TMP_VAR_NAME, self.tmp_counter);
        self.tmp_counter += 1;
        name
    }

    fn mark_checkpoint(&mut self) {
        let scope = self.current_scope_mut();
        scope.checkpoints.push(scope.current_line.len());
    }

    /// grabs the current_line slice since the last set checkpoint, assign it to variable and adds the assignment to the emitted code.
    /// returns the variable name that was used.
    fn hoist_checkpoint(&mut self, var: String) -> String {
        let indent = self.indentation();
        let scope = self.current_scope_mut();
        assert!(!scope.checkpoints.is_empty());
        let start = scope.checkpoints.pop().unwrap();
        assert!(scope.current_line.len() >= start);
        let expr = scope.current_line[start..].to_string();
        scope.current_line.truncate(start);
        scope.lines.push(format!("{}{} = {}", indent, var, expr));
        var
    }

    fn hoist_checkpoint_to_tmp(&mut self) -> String {
        let tmp = self.fresh_tmp();
        self.hoist_checkpoint(tmp)
    }

    fn print(&mut self, tmp_var: &String) {
        let indent = self.indentation();
        self.current_scope_mut()
            .lines
            .push(format!("{}_print({})", indent, tmp_var));
    }

    fn finish(mut self) -> String {
        assert_eq!(self.scopes.len(), 1, "Unbalanced scopes");
        let indent = self.indentation();
        let scope = self.current_scope_mut();
        if !scope.current_line.is_empty() {
            scope
                .lines
                .push(format!("{}{}", indent, scope.current_line));
            scope.current_line.clear();
        }

        let mut output = String::new();

        if !self.helpers.is_empty() {
            for helper in &self.helpers {
                output.push_str(helper.definition());
                output.push_str("\n\n");
            }
        }

        output.push_str(&self.scopes.pop().unwrap().lines.join("\n"));
        output
    }
}

struct Transpiler {
    emitter: PythonEmitter,
}

impl Transpiler {
    fn new() -> Self {
        Self {
            emitter: PythonEmitter::new(),
        }
    }

    fn finish(self) -> String {
        self.emitter.finish()
    }

    fn literal_to_var(node: &LanguloSyntaxNode) -> String {
        let text = node.text().to_string();
        // prepend var to make sure we avoid reserved keywords
        if text == "@" {
            "varself".to_string()
        } else {
            format!("var{}", text)
        }
    }

    fn visit(&mut self, node: &LanguloSyntaxNode) -> LanguloResult<()> {
        let should_print = has_print_marker(node);
        if should_print {
            self.emitter.require_helper(HelperFunction::Print);
            self.emitter.mark_checkpoint();
        }
        self.visit_inner(node)?;

        if should_print {
            let tmp = self.emitter.hoist_checkpoint_to_tmp();
            self.emitter.print(&tmp);
            self.emitter.write(tmp.as_str());
        }
        Ok(())
    }

    fn visit_lvalue(&mut self, node: &LanguloSyntaxNode) -> LanguloResult<String> {
        let should_print = has_print_marker(node);

        let target = match node.kind() {
            AstNode::Literal => Ok(Self::literal_to_var(node)),
            _ => Err(LanguloError::TranspileError {
                message: format!("Invalid assignment target: {:?}", node.kind()),
            }),
        }?;

        if should_print {
            self.emitter.require_helper(HelperFunction::Print);
            let tmp = self.emitter.fresh_tmp();
            self.emitter
                .emit_line(&format!("if not '{}' in vars():", target));
            self.emitter.increase_indentation();
            self.emitter.emit_line(&format!(
                "{}='(undefined variable `{}`)'",
                target,
                node.text().to_string()
            ));
            self.emitter.decrease_indentation();
            self.emitter.emit_line(&format!("{} = {}", tmp, target));
            self.emitter.print(&tmp);
        }

        Ok(target)
    }

    fn visit_inner(&mut self, node: &LanguloSyntaxNode) -> LanguloResult<()> {
        match node.kind() {
            AstNode::Root => {
                for child in node.children() {
                    self.visit(&child)?;
                }
            }
            AstNode::Literal => self.emitter.write(&Transpiler::literal_to_var(node)),
            AstNode::Num => {
                let text = node.text().to_string();
                let clean = text.replace('_', "");
                if clean.is_empty() {
                    return Err(LanguloError::TranspileError {
                        message: format!("Invalid number literal: {}", text),
                    });
                }
                self.emitter.write(&text);
            }
            AstNode::Bool => {
                let text = node.text().to_string();
                let capitalized = text
                    .chars()
                    .next()
                    .unwrap()
                    .to_uppercase()
                    .collect::<String>()
                    + &text[1..];
                self.emitter.write(&capitalized);
            }
            AstNode::Not => self.visit_unary(node, "not ")?,
            AstNode::Add => self.visit_binary(node, " + ")?,
            AstNode::Subtract => self.visit_binary(node, " - ")?,
            AstNode::Multiply => self.visit_binary(node, " * ")?,
            AstNode::Divide => self.visit_binary(node, " / ")?,
            AstNode::Modulo => self.visit_binary(node, " % ")?,
            AstNode::Power => self.visit_binary(node, " ** ")?,
            AstNode::And => self.visit_binary(node, " and ")?,
            AstNode::Or => self.visit_binary(node, " or ")?,
            AstNode::Xor => self.visit_binary(node, " ^ ")?,
            AstNode::Eq => self.visit_binary(node, " == ")?,
            AstNode::Neq => self.visit_binary(node, " != ")?,
            AstNode::Gt => self.visit_binary(node, " > ")?,
            AstNode::Lt => self.visit_binary(node, " < ")?,
            AstNode::Geq => self.visit_binary(node, " >= ")?,
            AstNode::Leq => self.visit_binary(node, " <= ")?,
            AstNode::Grouping => {
                let child = node.first_child().ok_or(LanguloError::TranspileError {
                    message: "Internal error: Grouping node has no children".to_string(),
                })?;
                self.emitter.write("(");
                self.visit(&child)?;
                self.emitter.write(")");
            }
            AstNode::Print => {
                return Err(LanguloError::InternalError {
                    message: "print nodes should never get visited after parsing".to_string(),
                });
            }
            AstNode::Assign => {
                let children: Vec<_> = node.children().collect();
                assert_eq!(children.len(), 2);

                let var_node = &children[0];
                let val_node = &children[1];

                let lvalue = self.visit_lvalue(var_node)?;

                self.emitter.mark_checkpoint();
                self.visit(val_node)?;
                self.emitter.hoist_checkpoint(lvalue.clone());
                self.emitter.write(&lvalue);
            }
            AstNode::FunctionDecl => {
                self.visit_function(node)?;
            }
            AstNode::FunctionParams | AstNode::FunctionBody => {
                return Err(LanguloError::InternalError {
                    message: "FunctionParams/FunctionBody should not be visited directly"
                        .to_string(),
                });
            }
            AstNode::PrefixFnCall => {
                let children: Vec<_> = node.children().collect();
                let func = &children[0];
                let args = &children[1];

                self.visit(func)?;
                self.emitter.write("(");

                let arg_exprs: Vec<_> = args.children().collect();
                for (i, arg) in arg_exprs.iter().enumerate() {
                    if i > 0 {
                        self.emitter.write(", ");
                    }
                    self.visit(arg)?;
                }

                self.emitter.write(")");
            }
            AstNode::PostfixFnCall => {
                let children: Vec<_> = node.children().collect();
                let at_value = &children[0];
                let call = &children[1];

                // call is a FunctionCall with children[0]=func, children[1]=args
                let call_children: Vec<_> = call.children().collect();
                let func = &call_children[0];
                let args = &call_children[1];

                self.visit(func)?;
                self.emitter.write("(");

                // First arg is the @ value
                self.visit(at_value)?;

                // Then the rest of the args
                let arg_exprs: Vec<_> = args.children().collect();
                for arg in arg_exprs.iter() {
                    self.emitter.write(", ");
                    self.visit(arg)?;
                }

                self.emitter.write(")");
            }

            AstNode::CallArgs => {
                return Err(LanguloError::InternalError {
                    message: "CallArgs should not be visited directly".to_string(),
                });
            },

            AstNode::Block => {
                let children: Vec<_> = node.children().collect();
                assert!(!children.is_empty(), "Empty block should have been caught by parser");
                self.emitter.require_helper(HelperFunction::Return);

                let result_var = self.emitter.fresh_tmp();
                self.emitter.emit_line("try:");
                self.emitter.increase_indentation();

                for (i, child) in children.iter().enumerate() {
                    let is_last = i == children.len() - 1;

                    if is_last {
                        self.emitter.mark_checkpoint();
                        self.visit(child)?;
                        let val = self.emitter.hoist_checkpoint_to_tmp();
                        self.emitter.emit_line(&format!("{} = {}", result_var, val));
                    } else {
                        self.emitter.mark_checkpoint();
                        self.visit(child)?;
                        self.emitter.hoist_checkpoint_to_tmp();
                    }
                }

                self.emitter.decrease_indentation();
                self.emitter.emit_line("except _Return as _r:");
                self.emitter.increase_indentation();
                self.emitter.emit_line(&format!("{} = _r.value", result_var));
                self.emitter.decrease_indentation();

                self.emitter.write(&result_var);
            }

            AstNode::Return => {
                self.emitter.require_helper(HelperFunction::Return);
                let child = node.first_child().ok_or(LanguloError::TranspileError {
                    message: "Return node has no children".to_string(),
                })?;
                self.emitter.mark_checkpoint();
                self.visit(&child)?;
                let val = self.emitter.hoist_checkpoint_to_tmp();
                self.emitter.emit_line(&format!("raise _Return({})", val));
                self.emitter.write("None");
            },
            AstNode::StringLit => {
                let children: Vec<_> = node.children().collect();

                if children.is_empty() {
                    self.emitter.write("\"\"");
                } else if children.len() == 1 && children[0].kind() == AstNode::StringPart {
                    let text = children[0].text().to_string();
                    self.emitter.write(&text);
                } else {
                    self.emitter.write("f\"");
                    for child in children {
                        match child.kind() {
                            AstNode::StringPart => {
                                let text = child.text().to_string();
                                let content = &text[1..text.len()-1];
                                self.emitter.write(content);
                            }
                            AstNode::InterpolationPart => {
                                self.emitter.write("{");
                                let expr = child.first_child().ok_or(LanguloError::TranspileError {
                                    message: "InterpolationPart has no expression".to_string(),
                                })?;
                                self.visit(&expr)?;
                                self.emitter.write("}");
                            }
                            _ => {}
                        }
                    }
                    self.emitter.write("\"");
                }
            }

            AstNode::StringPart | AstNode::InterpolationPart => {
                return Err(LanguloError::InternalError {
                    message: "StringPart/InterpolationPart should be handled by StringLit".to_string(),
                });
            }
        }
        Ok(())
    }

    fn visit_function(&mut self, node: &LanguloSyntaxNode) -> LanguloResult<()> {
        let children: Vec<_> = node.children().collect();
        assert_eq!(
            children.len(),
            2,
            "FunctionDecl should have params and body"
        );

        let params_node = &children[0];
        let body_node = &children[1];

        assert_eq!(params_node.kind(), AstNode::FunctionParams);
        assert_eq!(body_node.kind(), AstNode::FunctionBody);

        let params: Vec<String> = params_node
            .children()
            .map(|child| Self::literal_to_var(&child))
            .collect();

        let fn_name = self.emitter.fresh_tmp();
        self.emitter
            .emit_line(&format!("def {}({}):", fn_name, params.join(", ")));
        self.emitter.push_scope();
        self.emitter.increase_indentation();

        let body_expr = body_node
            .first_child()
            .ok_or(LanguloError::TranspileError {
                message: "Function body is empty".to_string(),
            })?;
        self.emitter.write("return ");
        self.visit(&body_expr)?;
        self.emitter.newline();

        self.emitter.decrease_indentation();

        self.end_scope();

        self.emitter.write(&fn_name);
        Ok(())
    }

    fn end_scope(&mut self) {
        let scope = self.emitter.pop_scope();
        for line in scope.lines {
            self.emitter.current_scope_mut().lines.push(line);
        }
    }

    fn visit_binary(&mut self, node: &LanguloSyntaxNode, op: &str) -> LanguloResult<()> {
        let children: Vec<_> = node.children().collect();
        if children.len() != 2 {
            return Err(LanguloError::TranspileError {
                message: format!(
                    "Binary operator {:?} expected 2 children, got {}",
                    node.kind(),
                    children.len()
                ),
            });
        }

        self.emitter.write("(");
        self.visit(&children[0])?;
        self.emitter.write(op);
        self.visit(&children[1])?;
        self.emitter.write(")");
        Ok(())
    }

    fn visit_unary(&mut self, node: &LanguloSyntaxNode, op: &str) -> LanguloResult<()> {
        if node.children().count() != 1 {
            return Err(LanguloError::TranspileError {
                message: format!(
                    "Unary operator {:?} expected 1 child, got {}",
                    node.kind(),
                    node.children().count()
                ),
            });
        }
        let child = node.first_child().unwrap();
        self.emitter.write(op);
        self.emitter.write("(");
        self.visit(&child)?;
        self.emitter.write(")");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn transpile_source(source: &str) -> String {
        let ast = parse(source).unwrap();
        let result = transpile(&ast).unwrap();
        println!("{}", result);
        result
    }

    #[test]
    fn test_simple_number() {
        assert_eq!(transpile_source("42"), "42");
    }

    #[test]
    fn test_print_simple() {
        let result = transpile_source("$3");
        assert!(result.contains("tmp0 = 3"));
        assert!(result.contains("_print(tmp0)"));
    }

    #[test]
    fn test_print_in_expression() {
        let result = transpile_source("1 + $2 + 3");
        assert!(result.contains("tmp0 = 2"));
        assert!(result.contains("_print(tmp0)"));
    }

    #[test]
    fn test_nested_print() {
        let result = transpile_source("$($1)");
        assert!(result.contains("_print"));
    }

    #[test]
    fn test_assignment() {
        let result = transpile_source("x = 42");
        assert!(result.contains("varx = 42"));
    }

    #[test]
    fn test_assignment_in_expression() {
        let result = transpile_source("3 + (x = 2)");
        assert!(result.contains("varx = 2"));
        assert!(result.contains("(3 + (varx))"));
    }

    #[test]
    fn test_print_assignment() {
        // x $= 3 means: assign 3 to x, then print the result
        let result = transpile_source("x $= 3");
        assert!(result.contains("varx = 3"));
        assert!(result.contains("_print"));
    }

    #[test]
    fn test_print_assignment_in_expression() {
        // 1 + (x $= 2) means: assign 2 to x, print 2, then add to 1
        let result = transpile_source("1 + (x $= 2)");
        assert!(result.contains("varx = 2"));
        assert!(result.contains("_print"));
    }
    ///////////////
    // functions //
    ///////////////

    #[test]
    fn test_function_no_params() {
        let result = transpile_source("always_two = || 2");
        assert!(result.contains("def tmp0():"));
        assert!(result.contains("return 2"));
        assert!(result.contains("varalways_two = tmp0"));
    }

    #[test]
    fn test_function_one_param() {
        let result = transpile_source("double = |x| x * 2");
        assert!(result.contains("def tmp0(varx):"));
        assert!(result.contains("return (varx * 2)"));
        assert!(result.contains("vardouble = tmp0"));
    }

    #[test]
    fn test_function_two_params() {
        let result = transpile_source("add = |a, b| a + b");
        assert!(result.contains("def tmp0(vara, varb):"));
        assert!(result.contains("return (vara + varb)"));
        assert!(result.contains("varadd = tmp0"));
    }

    #[test]
    fn test_function_withvarself() {
        let result = transpile_source("plus = |@, other| @ + other");
        assert!(result.contains("def tmp0(varself, varother):"));
        assert!(result.contains("return (varself + varother)"));
        assert!(result.contains("varplus = tmp0"));
    }

    #[test]
    fn test_function_complex_body() {
        let result = transpile_source("calc = |x, y| (x + y) * 2");
        assert!(result.contains("def tmp0(varx, vary):"));
        assert!(result.contains("return (((varx + vary)) * 2)"));
        assert!(result.contains("varcalc = tmp0"));
    }

    #[test]
    fn test_function_in_expression() {
        // Function as part of a larger expression
        let result = transpile_source("1 + (f = |x| x)");
        assert!(result.contains("def tmp0(varx):"));
        assert!(result.contains("return varx"));
        assert!(result.contains("varf = tmp0"));
    }
    //////////////
    // fn calls //
    //////////////

    #[test]
    fn test_function_call_transpile() {
        let result = transpile_source("add(1, 2)");
        assert!(result.contains("varadd(1, 2)"));
    }

    #[test]
    fn test_postfix_call_transpile() {
        let result = transpile_source("3 @ plus(2)");
        assert!(result.contains("varplus(3, 2)"));
    }

    #[test]
    fn test_postfix_call_no_extra_args() {
        let result = transpile_source("5 @ double()");
        assert!(result.contains("vardouble(5)"));
    }

    #[test]
    fn test_define_and_call() {
        let result = transpile_source("add = |a, b| a + b");
        assert!(result.contains("def tmp0(vara, varb):"));
        assert!(result.contains("return (vara + varb)"));
        assert!(result.contains("varadd = tmp0"));
    }

    #[test]
    fn test_block_single() {
        let result = transpile_source("{ 42 }");
        assert!(result.contains("class _Return"));
        assert!(result.contains("try:"));
        assert!(result.contains("except _Return"));
    }

    #[test]
    fn test_block_multiple() {
        let result = transpile_source("{ x = 1\nx + 1 }");
        assert!(result.contains("varx = 1"));
        assert!(result.contains("(varx + 1)"));
    }

    #[test]
    fn test_block_with_return() {
        let result = transpile_source("{ return 42\n99 }");
        assert!(result.contains("raise _Return("));
    }

    #[test]
    fn test_block_return_value() {
        let result = transpile_source("{ return 42 }");
        assert!(result.contains("raise _Return("));
    }
}
