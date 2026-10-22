//! Function bodies: parameters and return types.

use super::*;

impl Checker {
    pub(super) fn check_function(
        &mut self,
        name: &str,
        params: &[Param],
        signature: Type,
        body: &AstNode,
    ) -> CheckResult<Type> {
        let (param_types, declared_ret) = match signature {
            Type::Function { params, ret } => (params, *ret),
            _ => unreachable!("function signature is always a function type"),
        };
        let declared = if declared_ret == Type::Unknown {
            None
        } else {
            Some(declared_ret.clone())
        };

        self.push_scope();
        self.state.function_scopes.push(self.state.scopes.len() - 1);
        for (param, ty) in params.iter().zip(&param_types) {
            if param.is_mut && name == "anonymous function" {
                return self.err(format!(
                    "anonymous functions cannot have 'mut' parameters ('{}')",
                    param.name
                ));
            }
            // Only `mut` parameters can be modified; the others (and `self`) are imut. `mut self`
            // can change the value's contents, but not replace it.
            let is_mut_self = param.is_mut && param.name == "self";
            let mut symbol = Symbol::value(ty.clone(), !param.is_mut || is_mut_self);
            symbol.is_imut = !param.is_mut;
            symbol.reason = Some(if is_mut_self {
                "'mut self' can change its fields, not replace the value"
            } else {
                "parameters can only be modified when declared 'mut'"
            });
            self.declare(&param.name, symbol)?;
        }
        self.state.returns.push(ReturnContext {
            declared: declared.clone(),
            inferred: None,
        });

        let value_expected = declared.as_ref().filter(|t| **t != Type::Void);
        let body_result = match body.unlocated() {
            AstNode::Block(statements) => self.check_statements(statements, value_expected),
            expression => self.check_value(expression, value_expected),
        };
        let context = self.state.returns.pop();
        self.state.function_scopes.pop();
        self.pop_scope();
        let body_ty = body_result?;

        let ret = match declared {
            Some(Type::Void) => Type::Void,
            Some(declared) => {
                let ends_with_expression = match body.unlocated() {
                    AstNode::Block(statements) => {
                        statements.last().is_some_and(|s| !is_statement(s))
                    }
                    _ => true,
                };
                if body_ty == Type::Never {
                    // Every path returns explicitly.
                } else if !ends_with_expression || body_ty == Type::Void {
                    return self.err(format!(
                        "'{}' must return a value of type {} (add a 'return' or end with an expression)",
                        name,
                        self.show(&declared)
                    ));
                } else if !self.assignable(&declared, &body_ty) {
                    return self.err(format!(
                        "'{}' returns {}, but its declared return type is {}",
                        name,
                        self.show(&body_ty),
                        self.show(&declared)
                    ));
                }
                declared
            }
            None => {
                let inferred = context.and_then(|c| c.inferred);
                let is_block = matches!(body.unlocated(), AstNode::Block(_));
                match (inferred, body_ty) {
                    (Some(inferred), Type::Never) => inferred,
                    (Some(inferred), tail) if is_block && !matches!(tail, Type::Void) => {
                        self.join(&inferred, &tail).unwrap_or(inferred)
                    }
                    (Some(inferred), _) => inferred,
                    (None, Type::Never) => Type::Void,
                    (None, tail) => tail,
                }
            }
        };
        Ok(Type::Function {
            params: param_types,
            ret: Box::new(ret),
        })
    }
}
