use crate::errors::{LanguloError, LanguloResult};
use crate::parser::{AstNode, LanguloSyntaxNode};

pub fn transpile(ast: &LanguloSyntaxNode) -> LanguloResult<String> {
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

    fn visit(&mut self, node: &LanguloSyntaxNode) -> LanguloResult<()> {
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
                    return Err(LanguloError::TranspileError {
                        message: format!("Invalid number literal: {}", text),
                    });
                }
                self.output.push_str(&text);
            }
            AstNode::Bool => {
                let text = node.text().to_string();
                // python is True/False, needs to be capitalized
                let capitalized = text.chars().next().unwrap().to_uppercase().collect::<String>() + &text[1..];
                self.output.push_str(&capitalized);
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
                let child = node.first_child()
                    .ok_or(LanguloError::TranspileError {
                        message: "Internal error: Grouping node has no children".to_string(),
                    })?;
                self.output.push('(');
                self.visit(&child)?;
                self.output.push(')');
            },
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

        // to avoid any headache set very explicit priority by surrounding everything with parens
        self.output.push('(');
        self.visit(&children[0])?;
        self.output.push_str(op);
        self.visit(&children[1])?;
        self.output.push(')');
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
        let child = node.first_child()
            // safe due to above check
            .unwrap();
        self.output.push_str(op);
        self.output.push('(');
        self.visit(&child)?;
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
    
    #[test]
    fn test_booleans() { 
        assert_eq!(transpile_source("not true and false"), "(not(True) and False)");
    }
}