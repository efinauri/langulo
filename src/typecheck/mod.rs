use fxhash::FxHashMap;
use crate::parser::ast::lang::{NodeId, LanguloSyntaxNode, LanguloSyntaxNodeExt};
use crate::parser::ast::node::AstNode;
use crate::typecheck::types::{LanguloType, LanguloVariant};
use rusttyc::{TcErr, TcKey, TypeTable, VarlessTypeChecker};
use crate::errors::err::LanguloErr;

mod types;

pub struct TypeChecker {
    node_to_key: FxHashMap<NodeId, TcKey>,
    key_to_type: TypeTable<LanguloVariant>,
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
            node_to_key: Default::default(),
            key_to_type: Default::default(),
        }
    }

    pub fn type_of(&self, node: &LanguloSyntaxNode) -> &LanguloType {
        let key = self.node_to_key.get(&node.id()).unwrap();

        self.key_to_type
            .get(key)
            .expect(&*format!("no type was inferred for node {:?}", node))
    }

    /// also runs assert on the expected structure of the AST while typechecking
    pub fn typecheck(&mut self, root: &LanguloSyntaxNode) -> Result<(), LanguloErr> {
        let mut tc = VarlessTypeChecker::new();
        self.tc_node(&mut tc, &root)
            .map_err(|_| LanguloErr::typecheck("todo".to_string()))?;
        let table = tc.type_check()
            .map_err(|_| LanguloErr::typecheck("todo".to_string()))?;
        self.key_to_type = table;
        Ok(())
    }

    fn tc_node(&mut self, tc: &mut VarlessTypeChecker<LanguloVariant>, node: &LanguloSyntaxNode) -> Result<TcKey, TcErr<LanguloVariant>> {
        let key = tc.new_term_key();

        match node.kind() {
            AstNode::Root => {
                let mut last_key = match node.first_child() {
                    None => panic!("cannot typecheck an empty program"),
                    Some(child) => self.tc_node(tc, &child)?
                };
                for child in node.children().take(1) {
                    last_key = self.tc_node(tc, &child)?;
                }
                tc.impose(key.concretizes(last_key))?;
            }
            AstNode::Whitespace => panic!("trivia appears in AST"),
            AstNode::Comment => panic!("trivia appears in AST"),
            AstNode::Int => tc.impose(key.concretizes_explicit(LanguloVariant::Int))?,
            AstNode::Float => tc.impose(key.concretizes_explicit(LanguloVariant::Float))?,
            AstNode::Bool => tc.impose(key.concretizes_explicit(LanguloVariant::Bool))?,
            AstNode::Str => tc.impose(key.concretizes_explicit(LanguloVariant::Str))?,
            AstNode::Char => tc.impose(key.concretizes_explicit(LanguloVariant::Char))?,
            AstNode::Grouping => {
                assert_children_count!(node, 1);
                let inner = self.tc_node(tc, &node.first_child().unwrap())?;
                tc.impose(key.concretizes(inner))?;
            }

            AstNode::Add => {
                assert_children_count!(node, 2);
                tc.impose(key.concretizes_explicit(LanguloVariant::Addable))?;
                let children: Vec<_> = node.children().collect();
                let lhs = self.tc_node(tc, &children[0])?;
                let rhs = self.tc_node(tc, &children[1])?;
                tc.impose(key.is_meet_of(lhs, rhs))?;
            }
            AstNode::Subtract => unimplemented!(),
            AstNode::Identifier => unimplemented!(),
            AstNode::Multiply => {
                assert_children_count!(node, 2);
                tc.impose(key.concretizes_explicit(LanguloVariant::Multipliable))?;
                let children: Vec<_> = node.children().collect();
                let lhs = self.tc_node(tc, &children[0])?;
                let rhs = self.tc_node(tc, &children[1])?;
                tc.impose(key.is_meet_of(lhs, rhs))?;
            }
            AstNode::Divide => unimplemented!(),
            AstNode::LogicalAnd => unimplemented!(),
            AstNode::LogicalOr => unimplemented!(),
            AstNode::LogicalXor => unimplemented!(),
            AstNode::LogicalNot => unimplemented!(),
            AstNode::Modulo => unimplemented!(),
            // todo added while implementing parser
            AstNode::Scope => unimplemented!(),
            AstNode::Else => unimplemented!(),
            AstNode::If => unimplemented!(),
            AstNode::VarDecl => unimplemented!(),
            AstNode::TypeAnnotation => unimplemented!(),
            AstNode::TypeChar => unimplemented!(),
            AstNode::TypeInt => unimplemented!(),
            AstNode::TypeFloat => unimplemented!(),
            AstNode::TypeBool => unimplemented!(),
            AstNode::TypeStr => unimplemented!(),
            _ => unimplemented!("todo: write type checking for node type {:?}", node),
        }
        self.node_to_key.insert(node.id(), key);
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
