use crate::errors::{LanguloError, LanguloResult};
use crate::parser::{has_print_marker, AstNode, LanguloSyntaxNode};
use miette::SourceSpan;
use std::collections::HashSet;
use std::string::ToString;

const HIDDEN_VARIABLE_PREFIX: &'static str = "h";
const USER_LITERAL_PREFIX: &'static str = "uu"; // double to avoid hitting potential reserved keywords
const USER_AT_VAR: &'static str = "ua";
const ITER_VAL_VAR: &'static str = "iv";
const ITER_KEY_VAR: &'static str = "ik";
const ITER_INDEX_VAR: &'static str = "ii";

pub fn transpile(ast: &LanguloSyntaxNode, source: &str) -> LanguloResult<Vec<String>> {
    let mut transpiler = Transpiler::new(source);
    transpiler.visit(ast)?;
    transpiler.finish()
}

/// a static piece of python code that implements language constructs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum HelperFunction {
    Print,
    Return,
    Option,
    ToIterable,
}

impl HelperFunction {
    fn definition(&self) -> String {
        match self {
            HelperFunction::Print => {
                r#"
try:
    _print
except NameError:
    def _print(x):
        print(x)
        return x
"#
            }
            HelperFunction::Return => {
                r#"
try:
    _Return
except NameError:
    class _Return(Exception):
        def __init__(self, v): self.value = v
"#
            }
            HelperFunction::Option => {
                r#"
try:
    _Some
except NameError:
    class _Some:
        def __init__(self, v): self.value = v
        def __str__(self): return f'{self.value}!'
        def __bool__(self): return True
    _None = type('_None', (), {'__str__': lambda self: '?', '__bool__': lambda self: False})()
"#
            }
            HelperFunction::ToIterable => {
                r#"
try:
    _to_iterable
except NameError:
    def _to_iterable(x):
        if isinstance(x, str):
            return {i: c for i, c in enumerate(x)}
        return x
"#
            }
        }
        .into()
    }
}

/// when transpiling simple language constructs, you usually emit a one-liner that just goes to grow `current_line`.
///
/// more complex constructs, on the other hand, may require you to have executed other python logic beforehand.
/// in this scenario, the workflow is:
/// - push the index of `current_line` that you need to go back to once done into `checkpoints`;
/// - put the necessary python logic in `lines` and assign that overall result to a variable;
/// - resolve the checkpoint by growing `current_line` with that variable
///
/// this is implemented in: [PythonEmitter::resolve_checkpoint_to_var]
struct Scope {
    current_line: String,
    lines: Vec<String>,
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
}

impl PythonEmitter {
    fn new() -> Self {
        Self {
            scopes: vec![Scope::new()],
            indent: 0,
            helpers: HashSet::new(),
            tmp_counter: 0,
        }
    }

    /// Finalizes the current line as a standalone statement.
    /// Used to separate top-level expressions in Root.
    fn finalize_statement(&mut self) -> LanguloResult<()> {
        let indent = self.indentation();
        let scope = self.current_scope_mut()?;
        if !scope.current_line.is_empty() {
            scope.lines.push(format!("{}{}", indent, scope.current_line));
            scope.current_line.clear();
        }
        Ok(())
    }

    fn current_scope_mut(&mut self) -> LanguloResult<&mut Scope> {
        self.scopes.last_mut().ok_or(LanguloError::InternalError {
            _message: "No active scope".to_string(),
        })
    }

    fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn pop_scope(&mut self) -> LanguloResult<Scope> {
        self.scopes.pop().ok_or(LanguloError::InternalError {
            _message: "Attempted to remove main scope".to_string(),
        })
    }

    fn grow_current_line_with(&mut self, code: &str) -> LanguloResult<()> {
        self.current_scope_mut()?.current_line.push_str(code);
        Ok(())
    }

    fn indentation(&self) -> String {
        "    ".repeat(self.indent)
    }

    fn finish_current_line(&mut self) -> LanguloResult<()> {
        let indent = self.indentation();
        let scope = self.current_scope_mut()?;
        if scope.current_line.is_empty() {
            return Err(LanguloError::InternalError {
                _message: "tried to finalize am empty line".into(),
            });
        }
        if !scope.checkpoints.is_empty() {
            return Err(LanguloError::InternalError {
                _message: "tried to finalize a line with unresolved checkpoints".into(),
            });
        }
        scope
            .lines
            .push(format!("{}{}", indent, scope.current_line));
        scope.current_line.clear();
        Ok(())
    }

    fn increase_indentation(&mut self) {
        self.indent += 1;
    }

    fn decrease_indentation(&mut self) -> LanguloResult<()> {
        if self.indent == 0 {
            return Err(LanguloError::InternalError {
                _message: "tried to decrease indentation below base".into(),
            });
        }
        self.indent -= 1;
        Ok(())
    }

    fn require_helper(&mut self, helper: HelperFunction) {
        self.helpers.insert(helper);
    }

    fn fresh_hidden_var(&mut self) -> String {
        let name = format!("{}{}", HIDDEN_VARIABLE_PREFIX, self.tmp_counter);
        self.tmp_counter += 1;
        name
    }

    fn mark_checkpoint(&mut self) -> LanguloResult<()> {
        let scope = self.current_scope_mut()?;
        scope.checkpoints.push(scope.current_line.len());
        Ok(())
    }

    fn add_full_line_before_current(&mut self, line: &str) -> LanguloResult<()> {
        let indent = self.indentation();
        self.current_scope_mut()?
            .lines
            .push(format!("{}{}", indent, line));
        Ok(())
    }
    /// resolves the topmost checkpoint of the topmost scope by:
    /// - cutting up the [current_line](Scope) slice from the checkpoint onwards;
    /// - making it a variable assignment;
    /// - putting it into the translated python code before `current_line`
    fn resolve_checkpoint_to_var(&mut self, var: &String) -> LanguloResult<()> {
        let scope = self.current_scope_mut()?;
        let start = scope.checkpoints.pop().ok_or(LanguloError::InternalError {
            _message: "Tried to resolve checkpoint without any actual checkpoints".to_string(),
        })?;
        if scope.current_line.len() <= start {
            return Err(LanguloError::InternalError {
                _message: format!(
                    "Tried to resolve checkpoint at index {} but line is only {} chars long",
                    start,
                    scope.current_line.len()
                ),
            });
        }
        let expr = scope.current_line[start..].to_string();
        scope.current_line.truncate(start);
        self.add_full_line_before_current(&format!("{} = {}", var, expr))
    }

    /// calls [PythonEmitter::resolve_checkpoint_to_var] with an internal variable.
    ///
    /// returns the variable name that was used.
    fn resolve_checkpoint_to_hidden_var(&mut self) -> LanguloResult<String> {
        let hid = self.fresh_hidden_var();
        self.resolve_checkpoint_to_var(&hid)?;
        Ok(hid)
    }

    fn print(&mut self, var: &String) -> LanguloResult<()> {
        self.add_full_line_before_current(format!("_print({})", var).as_str())
    }

    /// emits the complete python code
    fn finish(mut self) -> LanguloResult<Vec<String>> {
        if self.scopes.len() != 1 {
            return Err(LanguloError::InternalError {
                _message: "Tried to finish emitter with unbalanced scopes".to_string(),
            });
        }

        let mut scope = self.pop_scope()?;
        if !scope.checkpoints.is_empty() {
            return Err(LanguloError::InternalError {
                _message: "Tried to finish emitter with unresolved checkpoints".to_string(),
            });
        }

        if !scope.current_line.is_empty() {
            scope
                .lines
                .push(format!("{}{}", self.indentation(), scope.current_line));
            scope.current_line.clear();
        }

        let mut result = vec![];

        for helper in self.helpers {
            result.push(helper.definition());
        }
        for line in scope.lines {
            result.push(line);
        }

        Ok(result)
    }
}

struct Transpiler {
    emitter: PythonEmitter,
    source: String,
    is_in_block: bool,
}

impl Transpiler {
    fn new(source: &str) -> Self {
        Self {
            emitter: PythonEmitter::new(),
            source: source.to_string(),
            is_in_block: false,
        }
    }

    fn finish(self) -> LanguloResult<Vec<String>> {
        self.emitter.finish()
    }

    fn node_span(&self, node: &LanguloSyntaxNode) -> SourceSpan {
        let range = node.text_range();
        let start: usize = range.start().into();
        let len: usize = range.len().into();
        (start, len).into()
    }

    fn literal_to_var(node: &LanguloSyntaxNode) -> String {
        match node.text().to_string().as_str() {
            "index" => ITER_INDEX_VAR.into(),
            "key" => ITER_KEY_VAR.into(),
            "value" => ITER_VAL_VAR.into(),
            "@" => USER_AT_VAR.into(),
            other => format!("{}{}", USER_LITERAL_PREFIX, other),
        }
    }

    fn visit(&mut self, node: &LanguloSyntaxNode) -> LanguloResult<()> {
        let should_print = has_print_marker(node);
        if should_print {
            self.emitter.require_helper(HelperFunction::Print);
            self.emitter.mark_checkpoint()?;
        }
        self.visit_inner(node)?;

        if should_print {
            let tmp = self.emitter.resolve_checkpoint_to_hidden_var()?;
            self.emitter.print(&tmp)?;
            self.emitter.grow_current_line_with(tmp.as_str())?;
        }
        Ok(())
    }

    /// lvalue is any assignment target, mainly literals but also indexed arrays (in the case of
    /// `x[1] = 2`).
    ///
    /// the main reason why lvalues are being handled in a separate method is to add some graceful
    /// fallback logic in case they're being printed whilst being re still undefined:
    ///
    /// `$new = 1 // prints ("undefined variable `new`)"`
    /// `$new = 2 // now would correctly print its previously assigned value, 1`
    ///
    fn visit_assignment(&mut self, node: &LanguloSyntaxNode) -> LanguloResult<()> {
        let children: Vec<_> = node.children().collect();
        if children.len() != 2 {
            return Err(LanguloError::InternalError {
                _message: format!("Assignment node found to have {} children", children.len()),
            });
        }

        let var_node = &children[0];
        let val_node = &children[1];

        let should_print = has_print_marker(var_node);

        match var_node.kind() {
            AstNode::Literal => {
                let target = Self::literal_to_var(var_node);

                if should_print {
                    self.emitter.require_helper(HelperFunction::Print);
                    let tmp = self.emitter.fresh_hidden_var();
                    // the lines below are emitted in a scope because it needs to be treated as a single statement
                    self.emitter.push_scope();
                    self.emitter
                        .add_full_line_before_current(&format!("if '{}' not in vars():", target))?;
                    self.emitter.increase_indentation();
                    self.emitter.add_full_line_before_current(&format!(
                        "{}='(undefined variable `{}`)'",
                        target,
                        var_node.text().to_string()
                    ))?;
                    self.emitter.decrease_indentation()?;
                    self.end_scope()?;
                    self.emitter
                        .add_full_line_before_current(&format!("{} = {}", tmp, target))?;
                    self.emitter.print(&tmp)?;
                }

                self.emitter.mark_checkpoint()?;
                self.visit(val_node)?;
                self.emitter.resolve_checkpoint_to_var(&target)?;
                self.emitter.grow_current_line_with(&target)?;
            }

            AstNode::MapIndex => {
                let index_children: Vec<_> = var_node.children().collect();
                if index_children.len() != 2 {
                    return Err(LanguloError::InternalError {
                        _message: format!(
                            "MapIndex should have 2 children, found {}",
                            index_children.len()
                        ),
                    });
                }
                let map_expr = &index_children[0];
                let key_expr = &index_children[1];

                self.emitter.mark_checkpoint()?;
                self.visit(map_expr)?;
                let map_var = self.emitter.resolve_checkpoint_to_hidden_var()?;
                self.emitter.mark_checkpoint()?;
                self.visit(key_expr)?;
                let key_var = self.emitter.resolve_checkpoint_to_hidden_var()?;

                if should_print {
                    self.emitter.require_helper(HelperFunction::Print);
                    self.emitter.require_helper(HelperFunction::Option);
                    let tmp = self.emitter.fresh_hidden_var();
                    self.emitter.add_full_line_before_current(&format!(
                        "{} = _Some({}[{}]) if {} in {} else _None",
                        tmp, map_var, key_var, key_var, map_var
                    ))?;
                    self.emitter.print(&tmp)?;
                }

                self.emitter.mark_checkpoint()?;
                self.visit(val_node)?;
                let val_var = self.emitter.resolve_checkpoint_to_hidden_var()?;

                self.emitter.add_full_line_before_current(&format!(
                    "{}[{}] = {}",
                    map_var, key_var, val_var
                ))?;

                self.emitter.grow_current_line_with(&val_var)?;
            }

            _ => {
                return Err(LanguloError::InvalidAssignmentTarget {
                    _src: self.source.clone(),
                    _span: self.node_span(var_node),
                });
            }
        }

        Ok(())
    }

    fn visit_inner(&mut self, node: &LanguloSyntaxNode) -> LanguloResult<()> {
        match node.kind() {
            AstNode::SomeOption => {
                self.emitter.require_helper(HelperFunction::Option);
                let child = node.first_child().ok_or(LanguloError::InternalError {
                    _message: "SomeOption node should have a child".to_string(),
                })?;
                self.emitter.grow_current_line_with("_Some(")?;
                self.visit(&child)?;
                self.emitter.grow_current_line_with(")")?;
            }
            AstNode::NoOption => {
                self.emitter.require_helper(HelperFunction::Option);
                self.emitter.grow_current_line_with("_None")?;
            }
            AstNode::If => {
                self.emitter.require_helper(HelperFunction::Option);
                let children: Vec<_> = node.children().collect();
                if children.len() != 2 {
                    return Err(LanguloError::InternalError {
                        _message: format!("If node found to have {} children", children.len()),
                    });
                }
                let cond = &children[0];
                let body = &children[1];
                let result_var = self.emitter.fresh_hidden_var();

                self.emitter.push_scope();
                self.emitter.grow_current_line_with("if ")?;
                self.visit(cond)?;
                self.emitter.grow_current_line_with(":")?;
                self.emitter.finish_current_line()?;

                self.emitter.increase_indentation();

                self.emitter.mark_checkpoint()?;
                self.visit(body)?;
                let body_var = self.emitter.resolve_checkpoint_to_hidden_var()?;
                self.emitter.add_full_line_before_current(&format!(
                    "{} = _Some({})",
                    result_var, body_var
                ))?;

                self.emitter.decrease_indentation()?;

                self.emitter.add_full_line_before_current("else:")?;
                self.emitter.increase_indentation();
                self.emitter
                    .add_full_line_before_current(&format!("{} = _None", result_var))?;
                self.emitter.decrease_indentation()?;

                self.end_scope()?;
                self.emitter.grow_current_line_with(&result_var)?;
            }
            AstNode::Else => {
                self.emitter.require_helper(HelperFunction::Option);
                let children: Vec<_> = node.children().collect();
                if children.len() != 2 {
                    return Err(LanguloError::InternalError {
                        _message: format!(
                            "Else node should have 2 children, found {}",
                            children.len()
                        ),
                    });
                }
                let option = &children[0];
                let default = &children[1];

                self.emitter.mark_checkpoint()?;
                self.visit(option)?;
                let opt_var = self.emitter.resolve_checkpoint_to_hidden_var()?;

                let result_var = self.emitter.fresh_hidden_var();

                self.emitter.push_scope();
                self.emitter
                    .add_full_line_before_current(&format!("if isinstance({}, _Some):", opt_var))?;
                self.emitter.increase_indentation();
                self.emitter
                    .add_full_line_before_current(&format!("{} = {}.value", result_var, opt_var))?;
                self.emitter.decrease_indentation()?;

                self.emitter.add_full_line_before_current("else:")?;
                self.emitter.increase_indentation();
                self.emitter.mark_checkpoint()?;
                self.visit(default)?;
                let default_var = self.emitter.resolve_checkpoint_to_hidden_var()?;
                self.emitter
                    .add_full_line_before_current(&format!("{} = {}", result_var, default_var))?;
                self.emitter.decrease_indentation()?;

                self.end_scope()?;
                self.emitter.grow_current_line_with(&result_var)?;
            }
            AstNode::Root => {
                let children: Vec<_> = node.children().collect();
                for (i, child) in children.iter().enumerate() {
                    self.visit(&child)?;
                    if i < children.len() - 1 {
                        self.emitter.finalize_statement()?;
                    }
                }
            }
            AstNode::Literal => self
                .emitter
                .grow_current_line_with(&Transpiler::literal_to_var(node))?,
            AstNode::Num => {
                let text = node.text().to_string();
                let clean = text.replace('_', "");
                if clean.is_empty() {
                    return Err(LanguloError::InvalidNumber {
                        _value: text,
                        _src: self.source.clone(),
                        _span: self.node_span(node),
                    });
                }
                self.emitter.grow_current_line_with(&text)?;
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
                self.emitter.grow_current_line_with(&capitalized)?;
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
            AstNode::Ask => {
                let child = node.first_child().ok_or(LanguloError::InternalError {
                    _message: "ask node should have a child".to_string(),
                })?;
                self.emitter.grow_current_line_with("bool(")?;
                self.visit(&child)?;
                self.emitter.grow_current_line_with(")")?;
            }
            AstNode::Grouping => {
                let child = node.first_child().ok_or(LanguloError::InternalError {
                    _message: "grouping node should have a child".to_string(),
                })?;
                self.emitter.grow_current_line_with("(")?;
                self.visit(&child)?;
                self.emitter.grow_current_line_with(")")?;
            }
            AstNode::Print => {
                return Err(LanguloError::InternalError {
                    _message: "print nodes should never get visited".to_string(),
                });
            }
            AstNode::Assign => self.visit_assignment(node)?,
            AstNode::FunctionDecl => self.visit_function(node)?,
            AstNode::FunctionParams | AstNode::FunctionBody => {
                return Err(LanguloError::InternalError {
                    _message: "FunctionParams/FunctionBody should not be visited directly"
                        .to_string(),
                });
            }
            AstNode::PrefixFnCall => {
                let children: Vec<_> = node.children().collect();
                if children.len() != 2 {
                    return Err(LanguloError::InternalError {
                        _message: format!(
                            "PrefixFnCall node found to have {} children",
                            children.len()
                        ),
                    });
                }
                let func = &children[0];
                let args = &children[1];

                self.visit(func)?;
                self.emitter.grow_current_line_with("(")?;

                for (i, arg) in args.children().enumerate() {
                    if i > 0 {
                        self.emitter.grow_current_line_with(", ")?;
                    }
                    self.visit(&arg)?;
                }
                self.emitter.grow_current_line_with(")")?;
            }
            AstNode::PostfixFnCall => {
                // example: `at_value @ func(args)`
                let children: Vec<_> = node.children().collect();

                if children.len() != 2 {
                    return Err(LanguloError::InternalError {
                        _message: format!(
                            "PostfixFnCall node found to have {} children",
                            children.len()
                        ),
                    });
                }
                let at_value = &children[0];
                let call = &children[1]; // func(args)

                let call_children: Vec<_> = call.children().collect();
                if call_children.len() != 2 {
                    return Err(LanguloError::InternalError {
                        _message: format!(
                            "PrefixFnCall node found to have {} children",
                            children.len()
                        ),
                    });
                }
                let func = &call_children[0];
                let args = &call_children[1];

                // python translation is rearranged:
                // `func(at_value, other_args)`
                self.visit(func)?;
                self.emitter.grow_current_line_with("(")?;

                self.visit(at_value)?;

                for arg in args.children() {
                    self.emitter.grow_current_line_with(", ")?;
                    self.visit(&arg)?;
                }

                self.emitter.grow_current_line_with(")")?;
            }

            AstNode::CallArgs => {
                return Err(LanguloError::InternalError {
                    _message: "CallArgs should not be visited directly".to_string(),
                });
            }

            AstNode::Block => {
                let children: Vec<_> = node.children().collect();
                if children.is_empty() {
                    return Err(LanguloError::InternalError {
                        _message: "Empty block should have been caught by parser".to_string(),
                    });
                }
                self.emitter.require_helper(HelperFunction::Return);
                let was_in_block = self.is_in_block;
                self.is_in_block = true;

                self.emitter.push_scope();
                // block is translated to a try/catch, you raise a returnException out of the scope
                let result_var = self.emitter.fresh_hidden_var();
                self.emitter.add_full_line_before_current("try:")?;
                self.emitter.increase_indentation();

                for (i, child) in children.iter().enumerate() {
                    let is_last = i == children.len() - 1;

                    if is_last {
                        // by default, always treated as a return
                        self.emitter.mark_checkpoint()?;
                        self.visit(child)?;
                        let val = self.emitter.resolve_checkpoint_to_hidden_var()?;
                        self.emitter
                            .add_full_line_before_current(&format!("{} = {}", result_var, val))?;
                    } else {
                        self.emitter.mark_checkpoint()?;
                        self.visit(child)?;
                        self.emitter.resolve_checkpoint_to_hidden_var()?;
                    }
                }

                self.emitter.decrease_indentation()?;
                self.emitter
                    .add_full_line_before_current("except _Return as _r:")?;
                self.emitter.increase_indentation();
                self.emitter
                    .add_full_line_before_current(&format!("{} = _r.value", result_var))?;
                self.emitter.decrease_indentation()?;

                self.end_scope()?;
                self.emitter.grow_current_line_with(&result_var)?;
                self.is_in_block = was_in_block;
            }

            AstNode::Return => {
                self.emitter.require_helper(HelperFunction::Return);
                if !self.is_in_block {
                    return Err(LanguloError::ReturnOutsideBlock {
                        _src: self.source.clone(),
                        _span: self.node_span(node),
                    });
                }

                if node.children().count() != 1 {
                    return Err(LanguloError::InternalError {
                        _message: format!(
                            "return node found to have operands: {:?}",
                            node.children().map(|c| c.kind())
                        ),
                    });
                }
                let child = node.first_child().unwrap();
                self.emitter.mark_checkpoint()?;
                self.visit(&child)?;
                let val = self.emitter.resolve_checkpoint_to_hidden_var()?;
                self.emitter
                    .add_full_line_before_current(&format!("raise _Return({})", val))?;
                self.emitter.grow_current_line_with("None")?;
            }
            AstNode::StringLit => {
                let children: Vec<_> = node.children().collect();

                if children.is_empty() {
                    self.emitter.grow_current_line_with("\"\"")?;
                } else if children.len() == 1 && children[0].kind() == AstNode::StringPart {
                    let text = children[0].text().to_string();
                    self.emitter.grow_current_line_with(&text)?;
                } else {
                    self.emitter.grow_current_line_with("f\"")?;
                    for child in children {
                        match child.kind() {
                            AstNode::StringPart => {
                                let text = child.text().to_string();
                                let content = &text[1..text.len() - 1];
                                self.emitter.grow_current_line_with(content)?;
                            }
                            AstNode::InterpolationPart => {
                                self.emitter.grow_current_line_with("{")?;
                                let expr =
                                    child.first_child().ok_or(LanguloError::InternalError {
                                        _message: "String interpolation without children nodes"
                                            .into(),
                                        // _src: self.source.clone(),
                                        // _span: self.node_span(&child),
                                    })?;
                                self.visit(&expr)?;
                                self.emitter.grow_current_line_with("}")?;
                            }
                            _ => {}
                        }
                    }
                    self.emitter.grow_current_line_with("\"")?;
                }
            }

            AstNode::StringPart | AstNode::InterpolationPart => {
                return Err(LanguloError::InternalError {
                    _message: "StringPart/InterpolationPart should be handled by StringLit"
                        .to_string(),
                });
            }
            AstNode::MapLit => {
                let entries: Vec<_> = node.children().collect();
                self.emitter.grow_current_line_with("{")?;
                for (i, entry) in entries.iter().enumerate() {
                    if i > 0 {
                        self.emitter.grow_current_line_with(", ")?;
                    }
                    let entry_children: Vec<_> = entry.children().collect();
                    if entry_children.len() != 2 {
                        return Err(LanguloError::InternalError {
                            _message: format!(
                                "MapEntry should have 2 children, found {}",
                                entry_children.len()
                            ),
                        });
                    }
                    self.visit(&entry_children[0])?;
                    self.emitter.grow_current_line_with(": ")?;
                    self.visit(&entry_children[1])?;
                }
                self.emitter.grow_current_line_with("}")?;
            }

            AstNode::MapEntry => {
                return Err(LanguloError::InternalError {
                    _message: "MapEntry should be handled by MapLit".to_string(),
                });
            }

            AstNode::SetLit => {
                let items: Vec<_> = node.children().collect();
                self.emitter.grow_current_line_with("{")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.emitter.grow_current_line_with(", ")?;
                    }
                    self.visit(item)?;
                    self.emitter.grow_current_line_with(": True")?;
                }
                self.emitter.grow_current_line_with("}")?;
            }

            AstNode::ListLit => {
                let items: Vec<_> = node.children().collect();
                self.emitter.grow_current_line_with("{")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.emitter.grow_current_line_with(", ")?;
                    }
                    self.emitter.grow_current_line_with(&format!("{}: ", i))?;
                    self.visit(item)?;
                }
                self.emitter.grow_current_line_with("}")?;
            }

            AstNode::Range => {
                let children: Vec<_> = node.children().collect();
                if children.len() != 2 {
                    return Err(LanguloError::InternalError {
                        _message: format!("Range should have 2 children, found {}", children.len()),
                    });
                }
                self.emitter.grow_current_line_with("{")?;
                self.emitter
                    .grow_current_line_with("i: i for i in range(")?;
                self.visit(&children[0])?;
                self.emitter.grow_current_line_with(", ")?;
                self.visit(&children[1])?;
                self.emitter.grow_current_line_with(")}")?;
            }

            AstNode::MapIndex => {
                self.emitter.require_helper(HelperFunction::Option);
                self.emitter.require_helper(HelperFunction::ToIterable);
                let children: Vec<_> = node.children().collect();
                if children.len() != 2 {
                    return Err(LanguloError::InternalError {
                        _message: format!(
                            "MapIndex should have 2 children, found {}",
                            children.len()
                        ),
                    });
                }
                let map_expr = &children[0];
                let key_expr = &children[1];

                self.emitter.mark_checkpoint()?;
                self.visit(map_expr)?;
                let uniterable_map_var = self.emitter.resolve_checkpoint_to_hidden_var()?;
                let map_var = self.emitter.fresh_hidden_var();
                self.emitter.add_full_line_before_current(&format!(
                    "{} = _to_iterable({})",
                    map_var, uniterable_map_var
                ))?;

                self.emitter.mark_checkpoint()?;
                self.visit(key_expr)?;
                let key_var = self.emitter.resolve_checkpoint_to_hidden_var()?;

                self.emitter.grow_current_line_with(&format!(
                    "(_Some({}[{}]) if {} in {} else _None)",
                    map_var, key_var, key_var, map_var
                ))?;
            }

            AstNode::Del => {
                self.emitter.require_helper(HelperFunction::Option);

                let child = node.first_child().ok_or(LanguloError::InternalError {
                    _message: "Del node should have a child".to_string(),
                })?;

                if child.kind() != AstNode::MapIndex {
                    return Err(LanguloError::InvalidDelTarget {
                        _src: self.source.clone(),
                        _span: self.node_span(&child),
                    });
                }

                let index_children: Vec<_> = child.children().collect();
                if index_children.len() != 2 {
                    return Err(LanguloError::InternalError {
                        _message: format!(
                            "MapIndex should have 2 children, found {}",
                            index_children.len()
                        ),
                    });
                }
                let map_expr = &index_children[0];
                let key_expr = &index_children[1];

                self.emitter.mark_checkpoint()?;
                self.visit(map_expr)?;
                let map_var = self.emitter.resolve_checkpoint_to_hidden_var()?;

                self.emitter.mark_checkpoint()?;
                self.visit(key_expr)?;
                let key_var = self.emitter.resolve_checkpoint_to_hidden_var()?;

                let result_var = self.emitter.fresh_hidden_var();
                self.emitter.add_full_line_before_current(&format!(
                    "{} = _Some({}.pop({})) if {} in {} else _None",
                    result_var, map_var, key_var, key_var, map_var
                ))?;

                self.emitter.grow_current_line_with(&result_var)?;
            }
            AstNode::Iter => {
                self.emitter.require_helper(HelperFunction::Return);
                self.emitter.require_helper(HelperFunction::ToIterable);
                let was_in_block = self.is_in_block;
                self.is_in_block = true;

                let children: Vec<_> = node.children().collect();
                if children.len() < 2 {
                    return Err(LanguloError::InternalError {
                        _message: format!(
                            "Iter node should have at least 2 children (map + body expressions), found {}",
                            children.len()
                        ),
                    });
                }

                let map_expr = &children[0];
                let body_exprs = &children[1..];

                self.emitter.mark_checkpoint()?;
                self.visit(map_expr)?;
                let uniterable_map_var = self.emitter.resolve_checkpoint_to_hidden_var()?;
                let map_var = self.emitter.fresh_hidden_var();
                self.emitter.add_full_line_before_current(&format!(
                    "{} = _to_iterable({})",
                    map_var, uniterable_map_var
                ))?;

                let result_var = self.emitter.fresh_hidden_var();

                self.emitter.push_scope();
                self.emitter
                    .add_full_line_before_current(&format!("{} = None", result_var))?;
                self.emitter.add_full_line_before_current("try:")?;
                self.emitter.increase_indentation();
                self.emitter.add_full_line_before_current(&format!(
                    "for {ITER_INDEX_VAR}, ({ITER_KEY_VAR}, {ITER_VAL_VAR}) in enumerate(sorted({map_var}.items())):",
                ))?;
                self.emitter.increase_indentation();

                for (i, expr) in body_exprs.iter().enumerate() {
                    let is_last = i == body_exprs.len() - 1;
                    self.emitter.mark_checkpoint()?;
                    self.visit(expr)?;
                    let var = self.emitter.resolve_checkpoint_to_hidden_var()?;
                    if is_last {
                        self.emitter
                            .add_full_line_before_current(&format!("{} = {}", result_var, var))?;
                    }
                }
                self.emitter.decrease_indentation()?;
                self.emitter.decrease_indentation()?;
                self.emitter
                    .add_full_line_before_current("except _Return as _r:")?;
                self.emitter.increase_indentation();
                self.emitter
                    .add_full_line_before_current(&format!("{} = _r.value", result_var))?;
                self.emitter.decrease_indentation()?;

                self.end_scope()?;
                self.emitter.grow_current_line_with(&result_var)?;
                self.is_in_block = was_in_block;
            }

            AstNode::IterVar => self
                .emitter
                .grow_current_line_with(Self::literal_to_var(&node).as_str())?,
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

        self.emitter.push_scope();
        let fn_name = self.emitter.fresh_hidden_var();
        self.emitter.add_full_line_before_current(&format!(
            "def {}({}):",
            fn_name,
            params.join(", ")
        ))?;
        self.emitter.increase_indentation();

        let body_expr = body_node.first_child().ok_or(LanguloError::InternalError {
            _message: "FunctionBody should have at least one child node".to_string(),
        })?;
        self.emitter.grow_current_line_with("return ")?;
        self.visit(&body_expr)?;
        self.emitter.finish_current_line()?;

        self.emitter.decrease_indentation()?;

        self.end_scope()?;

        self.emitter.grow_current_line_with(&fn_name)?;
        Ok(())
    }

    fn end_scope(&mut self) -> LanguloResult<()> {
        let scope = self.emitter.pop_scope()?;
        self.emitter
            .current_scope_mut()?
            .lines
            .push(scope.lines.join("\n"));
        Ok(())
    }

    fn visit_binary(&mut self, node: &LanguloSyntaxNode, op: &str) -> LanguloResult<()> {
        let children: Vec<_> = node.children().collect();
        if node.children().count() != 2 {
            return Err(LanguloError::InternalError {
                _message: format!(
                    "Binary operator {:?} found to have operands {:?}",
                    node.kind(),
                    node.children().map(|c| c.kind())
                ),
            });
        }

        self.emitter.grow_current_line_with("(")?;
        self.visit(&children[0])?;
        self.emitter.grow_current_line_with(op)?;
        self.visit(&children[1])?;
        self.emitter.grow_current_line_with(")")?;
        Ok(())
    }

    fn visit_unary(&mut self, node: &LanguloSyntaxNode, op: &str) -> LanguloResult<()> {
        if node.children().count() != 1 {
            return Err(LanguloError::InternalError {
                _message: format!(
                    "Unary operator {:?} found to have operands {:?}",
                    node.kind(),
                    node.children().map(|c| c.kind())
                ),
            });
        }
        assert_eq!(
            node.children().count(),
            1,
            "Unary operator requires exactly 1 operand"
        );

        let child = node.first_child().unwrap();
        self.emitter.grow_current_line_with(op)?;
        self.emitter.grow_current_line_with("(")?;
        self.visit(&child)?;
        self.emitter.grow_current_line_with(")")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn transpile_source(source: &str) -> String {
        let ast = parse(source).unwrap();
        let result = transpile(&ast, source).unwrap().join("\n");
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
        assert!(result.contains("h0 = 3"));
        assert!(result.contains("_print(h0)"));
    }

    #[test]
    fn test_print_in_expression() {
        let result = transpile_source("1 + $2 + 3");
        assert!(result.contains("h0 = 2"));
        assert!(result.contains("_print(h0)"));
    }

    #[test]
    fn test_nested_print() {
        let result = transpile_source("$($1)");
        assert!(result.contains("_print"));
    }

    #[test]
    fn test_assignment() {
        let result = transpile_source("x = 42");
        assert!(result.contains("uux = 42"));
    }

    #[test]
    fn test_assignment_in_expression() {
        let result = transpile_source("3 + (x = 2)");
        assert!(result.contains("uux = 2"));
        assert!(result.contains("(3 + (uux))"));
    }

    #[test]
    fn test_print_assignment() {
        // x $= 3 means: assign 3 to x, then print the result
        let result = transpile_source("x $= 3");
        assert!(result.contains("uux = 3"));
        assert!(result.contains("_print"));
    }

    #[test]
    fn test_print_assignment_in_expression() {
        // 1 + (x $= 2) means: assign 2 to x, print 2, then add to 1
        let result = transpile_source("1 + (x $= 2)");
        assert!(result.contains("uux = 2"));
        assert!(result.contains("_print"));
    }
    ///////////////
    // functions //
    ///////////////

    #[test]
    fn test_function_no_params() {
        let result = transpile_source("always_two = || 2");
        assert!(result.contains("def h0():"));
        assert!(result.contains("return 2"));
        assert!(result.contains("uualways_two = h0"));
    }

    #[test]
    fn test_function_one_param() {
        let result = transpile_source("double = |x| x * 2");
        assert!(result.contains("def h0(uux):"));
        assert!(result.contains("return (uux * 2)"));
        assert!(result.contains("uudouble = h0"));
    }

    #[test]
    fn test_function_two_params() {
        let result = transpile_source("add = |a, b| a + b");
        assert!(result.contains("def h0(uua, uub):"));
        assert!(result.contains("return (uua + uub)"));
        assert!(result.contains("uuadd = h0"));
    }

    #[test]
    fn test_function_with_atparam() {
        let result = transpile_source("plus = |@, other| @ + other");
        assert!(result.contains("def h0(ua, uuother):"));
        assert!(result.contains("return (ua + uuother)"));
        assert!(result.contains("uuplus = h0"));
    }

    #[test]
    fn test_function_complex_body() {
        let result = transpile_source("calc = |x, y| (x + y) * 2");
        assert!(result.contains("def h0(uux, uuy):"));
        assert!(result.contains("return (((uux + uuy)) * 2)"));
        assert!(result.contains("uucalc = h0"));
    }

    #[test]
    fn test_function_in_expression() {
        // Function as part of a larger expression
        let result = transpile_source("1 + (f = |x| x)");
        assert!(result.contains("def h0(uux):"));
        assert!(result.contains("return uux"));
        assert!(result.contains("uuf = h0"));
    }
    //////////////
    // fn calls //
    //////////////

    #[test]
    fn test_function_call_transpile() {
        let result = transpile_source("add(1, 2)");
        assert!(result.contains("uuadd(1, 2)"));
    }

    #[test]
    fn test_postfix_call_transpile() {
        let result = transpile_source("3 .plus(2)");
        assert!(result.contains("uuplus(3, 2)"));
    }

    #[test]
    fn test_postfix_call_no_extra_args() {
        let result = transpile_source("5 .double()");
        assert!(result.contains("uudouble(5)"));
    }

    #[test]
    fn test_define_and_call() {
        let result = transpile_source("add = |a, b| a + b");
        assert!(result.contains("def h0(uua, uub):"));
        assert!(result.contains("return (uua + uub)"));
        assert!(result.contains("uuadd = h0"));
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
        assert!(result.contains("uux = 1"));
        assert!(result.contains("(uux + 1)"));
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

    ///////////////
    // options   //
    ///////////////

    #[test]
    fn test_some_option() {
        let result = transpile_source("42!");
        assert!(result.contains("_Some(42)"));
    }

    #[test]
    fn test_none_option() {
        let result = transpile_source("?");
        assert!(result.contains("_None"));
    }
}
