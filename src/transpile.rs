use crate::errors::{LanguloError, LanguloResult};
use crate::parser::{has_print_marker, AstNode, LanguloSyntaxNode};
use std::collections::HashSet;
use std::fmt::format;
use std::string::ToString;

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
    helpers: HashSet<HelperFunction>,
    tmp_counter: usize,
    /// an index of current_line marked for future modification
    checkpoints: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum HelperFunction {
    Print,
}

impl HelperFunction {
    fn definition(&self) -> &'static str {
        match self {
            HelperFunction::Print => "def _print(x):\n    print(x)\n    return x",
        }
    }
}

struct LValue {
    /// like varx, varx[1], etc.
    target: String,
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

    fn require_helper(&mut self, helper: HelperFunction) {
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
        let start = self.checkpoints.pop().unwrap();
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

    fn print(&mut self, tmp_var: &String) {
        let indent = "    ".repeat(self.indent);
        self.lines.push(format!("{}_print({})", indent, tmp_var));
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

    fn visit_lvalue(&mut self, node: &LanguloSyntaxNode) -> LanguloResult<LValue> {
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
            let indent = "    ".repeat(self.emitter.indent);
            self.emitter.lines.push(format!("{}if not '{}' in vars():", indent, target));
            self.emitter.lines.push(format!("{}{}{}='(undefined variable `{}`)'",
            indent, "    ", target, node.text().to_string()));
            self.emitter.lines.push(format!("{}{} = {}", indent, tmp, target));
            self.emitter.print(&tmp);
        }

        Ok(LValue { target })
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
                self.emitter.hoist_checkpoint(lvalue.target.clone());
                self.emitter.write(&lvalue.target);
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
}
