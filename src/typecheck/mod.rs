use crate::syntax_tree::lang::LanguloSyntaxNode;
use crate::syntax_tree::node::AstNode;
use crate::typecheck::types::LanguloVariant;
use rusttyc::{TcErr, TcKey, TypeTable, Variant, VarlessTypeChecker};

mod types;

pub fn typecheck(node: &LanguloSyntaxNode) -> Result<(TcKey, TypeTable<LanguloVariant>), TcErr<LanguloVariant>> {
    let mut tc = VarlessTypeChecker::new();
    let root_key = tc_node(&mut tc, node)?;
    let table = tc.type_check()?;
    Ok((root_key, table))
}

fn tc_node(tc: &mut VarlessTypeChecker<LanguloVariant>, node: &LanguloSyntaxNode) -> Result<TcKey, TcErr<LanguloVariant>> {
    match node.kind() {
        AstNode::Root => {
            assert_eq!(node.children().count(), 1, "node does not have exactly one child");
            tc_node(tc, &node.first_child().unwrap())
        }
        AstNode::Identifier => unimplemented!(),
        AstNode::Literal => unimplemented!(),
        AstNode::Whitespace => {
            let next = node.next_sibling();
            assert!(next.is_some(), "standalone whitespace");
            tc_node(tc, &next.unwrap())
        }
        AstNode::Binary => unimplemented!(),
        AstNode::Unary => unimplemented!(),
        AstNode::Int => {
            let key = tc.new_term_key();
            tc.impose(key.concretizes_explicit(LanguloVariant::Int))?;
            Ok(key)
        }
        AstNode::Comment => {
            let next = node.next_sibling();
            assert!(next.is_some(), "standalone comment");
            tc_node(tc, &next.unwrap())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::Parser;
    use crate::typecheck::types::LanguloType;
    use crate::typecheck::{typecheck};

    #[test]
    fn typecheck_int() {
        let mut parser = Parser::new("1");
        parser.parse().expect("failed to parse");
        let root = parser.to_ast();
        println!("AST: {:#?}", root);
        let (root_key, table) = typecheck(&root).expect("failed to typecheck");
        assert_eq!(table.get(&root_key), Some(&LanguloType::Int));
    }
}
