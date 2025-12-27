use crate::errors::{LanguriaError, LanguriaResult};
use crate::parser::{AstNode, LanguriaSyntaxNode};

pub fn transpile(ast: &LanguriaSyntaxNode) -> LanguriaResult<String> {
    let mut transpiler = Transpiler::new();
    transpiler.visit(ast)?;
    Ok(transpiler.output)
}

struct Transpiler {
    output: String,
}

impl Transpiler {
    fn new() -> Self {
        Self {
            output: String::new(),
        }
    }

    fn visit(&mut self, node: &LanguriaSyntaxNode) -> LanguriaResult<()> {
        match node.kind() {
            AstNode::Root => {
                for child in node.children() {
                    self.visit(&child)?;
                }
            }
            AstNode::Num => {
                let text = node.text().to_string();
                // just in case that python struggles with arbitrary underscores for nums
                let clean = text.replace('_', "");
                if clean.is_empty() {
                    return Err(LanguriaError::TranspileError {
                        message: format!("Invalid number literal: {}", text),
                    });
                }
                self.output.push_str(&text);
            }
            AstNode::Add => self.visit_binary(node, "+")?,
            AstNode::Subtract => self.visit_binary(node, "-")?,
            AstNode::Multiply => self.visit_binary(node, "*")?,
            AstNode::Divide => self.visit_binary(node, "/")?,
            AstNode::Modulo => self.visit_binary(node, "%")?,
            AstNode::Power => self.visit_binary(node, "**")?,
            AstNode::Grouping => {
                let child = node.first_child()
                    .ok_or(LanguriaError::TranspileError {
                        message: "Internal error: Grouping node has no children".to_string(),
                    })?;
                self.output.push('(');
                self.visit(&child)?;
                self.output.push(')');
            },
        }
        Ok(())
    }

    fn visit_binary(&mut self, node: &LanguriaSyntaxNode, op: &str) -> LanguriaResult<()> {
        let children: Vec<_> = node.children().collect();
        if children.len() != 2 {
            return Err(LanguriaError::TranspileError {
                message: format!(
                    "Binary operator {:?} expected 2 children, got {}",
                    node.kind(),
                    children.len()
                ),
            });
        }

        // to avoid any headache set very explicit priority by surrounding everything with parens
        self.output.push('(');
        self.visit(&children[0])?;
        self.output.push_str(op);
        self.visit(&children[1])?;
        self.output.push(')');
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
        assert_eq!(transpile_source("3.14"), "3.14");
        assert_eq!(transpile_source("1_000_000"), "1_000_000");
    }

    #[test]
    fn test_simple_operations() {
        assert_eq!(transpile_source("1 + 2"), "(1+2)");
        assert_eq!(transpile_source("3 - 1"), "(3-1)");
        assert_eq!(transpile_source("2 * 3"), "(2*3)");
        assert_eq!(transpile_source("6 / 2"), "(6/2)");
        assert_eq!(transpile_source("7 % 3"), "(7%3)");
    }

    #[test]
    fn test_power_transpiles_correctly() {
        assert_eq!(transpile_source("2 ^ 3"), "(2**3)");
    }

    #[test]
    fn test_precedence_preserved() {
        assert_eq!(transpile_source("1 + 2 * 3"), "(1+(2*3))");
    }

    #[test]
    fn test_complex_expression() {
        assert_eq!(
            transpile_source("1 + 2 * 3 ^ 4"),
            "(1+(2*(3**4)))"
        );
    }
    #[test]
    fn test_grouping() {
        assert_eq!(transpile_source("(2-3)*(4-5)"), "(((2-3))*((4-5)))");
    }
}