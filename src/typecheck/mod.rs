use std::collections::HashMap;
use crate::parser::syntax_tree::lang::LanguloSyntaxNode;
use crate::parser::syntax_tree::node::AstNodeKind;
use crate::typecheck::types::{LanguloType, LanguloVariant};
use rusttyc::{TcErr, TcKey, TypeTable, VarlessTypeChecker};
use crate::errors::err::LanguloErr;

mod types;

pub struct TypeChecker {
    node_to_key: HashMap<u64, TcKey>,
    key_to_type: Option<TypeTable<LanguloVariant>>,
}

macro_rules! assert_children_count {
    ($node:expr, $expected_count:expr) => {{
        let children: Vec<_> = $node.children().collect();
        let actual_count = children.len();
        let node_type = $node.kind();
        if actual_count != $expected_count {
            eprintln!(
                "Assertion failed: Node type {:?} expected {} children, but got {}. Node children: {:?}",
                node_type, $expected_count, actual_count, children
            );
            panic!(
                "Assertion failed: Node type {:?} expected {} children, but got {}.",
                node_type, $expected_count, actual_count
            );
        }
    }};
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            node_to_key: HashMap::new(),
            key_to_type: None,
        }
    }

    pub fn type_of(&self, node: &LanguloSyntaxNode) -> &LanguloType {
        // let key = self.node_to_key.get(node.id).unwrap();

        // self.key_to_type
        //     .expect("need to perform typechecking before types of nodes can be queried")
        //     .get(key)
        //     .expect(&*format!("no type was inferred for node {:?}", node))
        &LanguloType::Int
    }

    /// also runs assert on the expected structure of the AST while typechecking
    pub fn typecheck(&mut self, root: &LanguloSyntaxNode) -> Result<(), LanguloErr> {

        let mut tc = VarlessTypeChecker::new();
        self.tc_node(&mut tc, &root)
            .map_err(|tc_err| LanguloErr::typecheck("todo".to_string()))?;
        let table = tc.type_check()
            .map_err(|tc_err| LanguloErr::typecheck("todo".to_string()))?;
        self.key_to_type = Some(table);
        Ok(())
    }

    fn tc_node(&mut self, tc: &mut VarlessTypeChecker<LanguloVariant>, node: &LanguloSyntaxNode) -> Result<TcKey, TcErr<LanguloVariant>> {
        let key = tc.new_term_key();

        match node.kind() {
            AstNodeKind::Root => {
                let mut last_key = match node.first_child() {
                    None => panic!("cannot typecheck an empty program"),
                    Some(child) => self.tc_node(tc, &child)?
                };
                for child in node.children().take(1) {
                    last_key = self.tc_node(tc, &child)?;
                }
                tc.impose(key.concretizes(last_key))?;
            }
            AstNodeKind::Whitespace => panic!("trivia appears in AST"),
            AstNodeKind::Comment => panic!("trivia appears in AST"),
            AstNodeKind::Int => tc.impose(key.concretizes_explicit(LanguloVariant::Int))?,
            AstNodeKind::Float => tc.impose(key.concretizes_explicit(LanguloVariant::Float))?,
            AstNodeKind::Bool => tc.impose(key.concretizes_explicit(LanguloVariant::Bool))?,
            AstNodeKind::Str => tc.impose(key.concretizes_explicit(LanguloVariant::Str))?,
            AstNodeKind::Char => tc.impose(key.concretizes_explicit(LanguloVariant::Char))?,

            AstNodeKind::Add => {
                assert_children_count!(node, 2);
                tc.impose(key.concretizes_explicit(LanguloVariant::Addable))?;
                let children: Vec<_> = node.children().collect();
                let lhs = self.tc_node(tc, &children[0])?;
                let rhs = self.tc_node(tc, &children[1])?;
                tc.impose(key.is_meet_of(lhs, rhs))?;
            }
            AstNodeKind::Subtract => unimplemented!(),
            AstNodeKind::Identifier => unimplemented!(),
            AstNodeKind::Multiply => {
                assert_children_count!(node, 2);
                tc.impose(key.concretizes_explicit(LanguloVariant::Multipliable))?;
                let children: Vec<_> = node.children().collect();
                let lhs = self.tc_node(tc, &children[0])?;
                let rhs = self.tc_node(tc, &children[1])?;
                tc.impose(key.is_meet_of(lhs, rhs))?;
            }
            AstNodeKind::Divide => unimplemented!(),
            AstNodeKind::LogicalAnd => unimplemented!(),
            AstNodeKind::LogicalOr => unimplemented!(),
            AstNodeKind::LogicalXor => unimplemented!(),
            AstNodeKind::LogicalNot => unimplemented!(),
            AstNodeKind::Modulo => unimplemented!(),
        }
        // self.node_to_key.insert(node.id, key);
        Ok(key)
    }
}


#[cfg(test)]
mod tests {
    use crate::parser::Parser;
    use crate::typecheck::TypeChecker;
    use crate::typecheck::types::LanguloType;

    fn expect_typecheck(input: &str, expected_type: Option<LanguloType>) {
        let mut parser = Parser::new(input);
        parser.parse().expect("could not parse");
        let root = parser.to_ast();

        let mut type_checker = TypeChecker::new();
        if expected_type.is_none() {
            assert!(type_checker.typecheck(&root).is_err())
        } else {
            type_checker.typecheck(&root).expect("type check err");
            assert_eq!(Some(type_checker.type_of(&root).clone()), expected_type);
        }
    }

    #[test]
    fn int() { expect_typecheck("1;", Some(LanguloType::Int)) }
    #[test]
    fn arithmetic() { expect_typecheck("1 + 2;", Some(LanguloType::Int)) }
    #[test]
    fn arithmetic_fails() { expect_typecheck("1 + 'c';", None) }
    #[test]
    fn cannot_sum_chars() { expect_typecheck("'c' + 'd';", None) }
}
