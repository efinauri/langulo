use crate::parser::LanguriaSyntaxNode;
use koto::parser::Ast as KotoAst;

pub fn translate_ast(languria_ast: LanguriaSyntaxNode) -> KotoAst {
    let mut result = KotoAst::with_capacity(8);
    // TODO
    result
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;
    use super::*;

    #[test]
    fn test_translate_ast() {
        let languria_ast = parse("1 + 2").unwrap();
        let koto_ast = translate_ast(languria_ast);
        println!("{koto_ast:?}")
    }
}
