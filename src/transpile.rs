use crate::errors::{LanguloError, LanguloResult};
use crate::parser::{AstNode, LanguloSyntaxNode};
use std::collections::HashSet;
use std::string::ToString;
use pyo3::IntoPy;

const TMP_VAR_NAME: &'static str = "tmp";

pub fn transpile(ast: &LanguloSyntaxNode) -> LanguloResult<String> {
    let mut transpiler = Transpiler::new();
    transpiler.visit(ast)?;
    Ok(transpiler.finish())
}

struct PythonEmitter {
    lines: Vec<String>,
    current_line: String,
    indent: usize,
    helpers: HashSet<Helper>,
    tmp_counter: usize,
    /// an index of current_line marked for future modification
    checkpoints: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Helper {
    Print,
}

impl Helper {
    fn definition(&self) -> &'static str {
        match self {
            Helper::Print => "def _print(x):\n    print(x)\n    return x",
        }
    }
}

impl PythonEmitter {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_line: String::new(),
            indent: 0,
            helpers: HashSet::new(),
            tmp_counter: 0,
            checkpoints: Vec::new(),
        }
    }

    fn write(&mut self, code: &str) {
        self.current_line.push_str(code);
    }

    fn newline(&mut self) {
        assert!(!self.current_line.is_empty());
        assert!(self.checkpoints.is_empty());
        let indent = "    ".repeat(self.indent);
        self.lines.push(format!("{}{}", indent, self.current_line));
        self.current_line.clear();
    }

    fn indent(&mut self) {
        self.indent += 1;
    }

    fn dedent(&mut self) {
        assert!(self.indent > 0);
        self.indent -= 1;
    }

    fn require_helper(&mut self, helper: Helper) {
        self.helpers.insert(helper);
    }

    fn fresh_tmp(&mut self) -> String {
        let name = format!("{}{}", TMP_VAR_NAME, self.tmp_counter);
        self.tmp_counter += 1;
        name
    }

    fn mark_checkpoint(&mut self) {
        self.checkpoints.push(self.current_line.len());
    }

    /// grabs the current_line slice since the last set checkpoint, assign it to variable and adds the assignment to the emitted code.
    /// returns the variable name that was used.
    fn hoist_checkpoint(&mut self, var: String) -> String {
        assert!(!self.checkpoints.is_empty());
        let start = self
            .checkpoints
            .pop()
            .unwrap();
        assert!(self.current_line.len() >= start);
        let expr = self.current_line[start..].to_string();
        self.current_line.truncate(start);

        let indent = "    ".repeat(self.indent);
        self.lines.push(format!("{}{} = {}", indent, var, expr));

        var
    }

    fn hoist_checkpoint_to_tmp(&mut self) -> String {
        let tmp = self.fresh_tmp();
        self.hoist_checkpoint(tmp)
    }



    fn finish(mut self) -> String {
        self.newline();

        let mut output = String::new();

        if !self.helpers.is_empty() {
            for helper in &self.helpers {
                output.push_str(helper.definition());
                output.push_str("\n\n");
            }
        }

        output.push_str(&self.lines.join("\n"));
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
        // prepend var to make sure we avoid reserved keywords
        format!("var{}", node.text().to_string())
    }

    fn visit(&mut self, node: &LanguloSyntaxNode) -> LanguloResult<()> {
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
                self.emitter.require_helper(Helper::Print);
                let child = node.first_child().ok_or(LanguloError::TranspileError {
                    message: "Internal error: Print node has no children".to_string(),
                })?;
                self.emitter.mark_checkpoint();
                self.visit(&child)?;
                let tmp = self.emitter.hoist_checkpoint_to_tmp();
                self.emitter.write(&format!("_print({})", tmp));
            }
            AstNode::Assign => {
                assert_eq!(node.children().count(), 2);
                let var_node = node.first_child().unwrap();
                if let Some(AstNode::Literal) = var_node.kind().into() {
                    // an assignment evaluates to the assigned value, so it needs hoisting as well (this time named hoisting)
                    // langulo: 3 + x = 2
                    // python: x = 2    3 + x
                    //var can be visited, will just place the varname where the evaluated expression should be
                    self.visit(&var_node)?;
                    self.emitter.mark_checkpoint();
                    let val_node = node.last_child().unwrap();
                    self.visit(&val_node)?;
                    let _ = self.emitter.hoist_checkpoint(Transpiler::literal_to_var(&var_node));
                }
                else {
                    return Err(LanguloError::TranspileError {
                        message: format!("Invalid assignment target: {:?}", var_node.kind()),
                    });
                }

            }
        }
        Ok(())
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
        transpile(&ast).unwrap()
    }

    #[test]
    fn test_simple_number() {
        assert_eq!(transpile_source("42"), "42");
    }

    #[test]
    fn test_print_hoists() {
        // $3 should hoist the 3 to a temp var
        let result = transpile_source("$3");
        assert!(result.contains("tmp0 = 3"));
        assert!(result.contains("_print(tmp0)"));
    }

    #[test]
    fn test_print_in_expression() {
        // 1 + $2 + 3 should hoist just the print operand
        let result = transpile_source("1 + $2 + 3");
        assert!(result.contains("tmp0 = 2"));
        assert!(result.contains("_print(tmp0)"));
    }

    #[test]
    fn test_nested_print() {
        // $($1) - nested prints
        let result = transpile_source("$($1)");
        assert!(result.contains("tmp0 = 1"));
        assert!(result.contains("tmp1 = _print(tmp0)"));
        assert!(result.contains("_print(tmp1)"));
    }

    #[test]
    fn test_assignment() {
        let mut result = transpile_source("x = 42");
        assert!(result.contains("varx = 42"));
        result = transpile_source("3 + (x=2)");
        assert!(result.contains("varx = 2"));
        assert!(result.contains("3 + (varx)"));
    }
}
