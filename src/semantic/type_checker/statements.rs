//! Statements, assignments and what can be modified.

use super::*;

impl Checker {
    pub(super) fn set_line(&mut self, node: &AstNode) {
        if let AstNode::Located { line, .. } = node {
            self.state.line = *line;
        }
    }

    /// Checks a sequence of statements in the current scope; returns the type of its value (the
    /// last statement, when it is an expression).
    pub(super) fn check_statements(&mut self, statements: &[AstNode], expected: Option<&Type>) -> CheckResult<Type> {
        self.hoist(statements)?;
        let mut tail = Type::Void;
        for (i, statement) in statements.iter().enumerate() {
            let is_last = i + 1 == statements.len();
            tail = self.check_statement(statement, if is_last { expected } else { None })?;
            if !is_last && !matches!(tail, Type::Never) {
                tail = Type::Void;
            }
        }
        if let Some(last) = statements.last() {
            if is_statement(last) {
                return Ok(Type::Void);
            }
        }
        Ok(tail)
    }

    pub(super) fn check_block(
        &mut self,
        node: &AstNode,
        expected: Option<&Type>,
        bindings: Vec<(String, Symbol)>,
    ) -> CheckResult<Type> {
        self.push_scope();
        for (name, symbol) in bindings {
            self.current_scope().vars.insert(name, symbol);
        }
        let result = match node.unlocated() {
            AstNode::Block(statements) => self.check_statements(statements, expected),
            other => self.check_statement(other, expected),
        };
        self.pop_scope();
        result
    }

    pub(super) fn check_statement(&mut self, node: &AstNode, expected: Option<&Type>) -> CheckResult<Type> {
        match node {
            AstNode::Located { line, node } => {
                self.state.line = *line;
                self.check_statement(node, expected)
            }
            AstNode::Export(node) => self.check_statement(node, expected),
            AstNode::VarDecl {
                name,
                ty,
                value,
                is_const,
                is_imut,
            } => {
                let declared = match ty {
                    Some(ty) => Some(self.resolve_type(ty)?),
                    None => None,
                };
                let value_ty = self.check_value(value, declared.as_ref())?;
                let var_ty = match declared {
                    Some(declared) => {
                        if !self.assignable(&declared, &value_ty) {
                            return self.err(format!(
                                "cannot assign a value of type {} to '{}' of type {}",
                                self.show(&value_ty),
                                name,
                                self.show(&declared)
                            ));
                        }
                        declared
                    }
                    None => value_ty,
                };
                let mut symbol = Symbol::value(var_ty, *is_const);
                symbol.is_imut = *is_imut;
                self.declare(name, symbol)?;
                Ok(Type::Void)
            }
            AstNode::Assign { target, op, value } => {
                self.check_assign(target, op, value)?;
                Ok(Type::Void)
            }
            AstNode::FunctionDecl {
                name,
                owner: Some(owner),
                params,
                body,
                ..
            } => {
                let full = format!("{}.{}", owner, name);
                let signature = match self.lookup_type(owner) {
                    Some(TypeRef::Type(Type::Named { id, .. })) => {
                        self.methods.get(&(*id, name.clone())).map(|m| m.ty.clone())
                    }
                    _ => None,
                };
                match signature {
                    Some(signature) => self.check_function(&full, params, signature, body)?,
                    None => return self.err(format!("unknown method '{}'", full)),
                };
                Ok(Type::Void)
            }
            AstNode::FunctionDecl {
                name,
                params,
                return_type,
                body,
                ..
            } => {
                let signature = match self.state.scopes.last().and_then(|s| s.vars.get(name)) {
                    Some(symbol) => symbol.ty.clone(),
                    None => self.function_signature(name, params, return_type.as_ref(), true)?,
                };
                self.check_function(name, params, signature, body)?;
                Ok(Type::Void)
            }
            AstNode::TypeDecl { .. } => Ok(Type::Void),
            // Imports are processed when the block is hoisted.
            AstNode::Import { .. } => Ok(Type::Void),
            AstNode::While { condition, body } => {
                self.check_condition(condition, "while")?;
                self.check_block(body, None, vec![])?;
                Ok(Type::Void)
            }
            AstNode::For {
                variable,
                is_mut,
                iterable,
                body,
            } => {
                let iterable_ty = self.check_value(iterable, None)?;
                let item = match &iterable_ty {
                    Type::List(inner) => (**inner).clone(),
                    Type::String => Type::String,
                    Type::Unknown => Type::Unknown,
                    Type::Named { id, .. } => match &self.table.get(*id).kind {
                        TypeKind::Dict { key, .. } => key.clone(),
                        _ => return self.not_iterable(&iterable_ty),
                    },
                    _ => return self.not_iterable(&iterable_ty),
                };
                let mut symbol = Symbol::value(item, !is_mut);
                symbol.is_imut = !is_mut;
                symbol.reason = Some("the 'for' variable is imut (use 'for mut' to change it)");
                self.check_block(body, None, vec![(variable.clone(), symbol)])?;
                Ok(Type::Void)
            }
            AstNode::Break | AstNode::Continue => Ok(Type::Never),
            AstNode::Return(value) => {
                self.check_return(value.as_deref())?;
                Ok(Type::Never)
            }
            other => self.check_expr(other, expected),
        }
    }

    pub(super) fn not_iterable<T>(&self, ty: &Type) -> CheckResult<T> {
        self.err(format!(
            "cannot iterate over a value of type {} (use a list, a string or range())",
            self.show(ty)
        ))
    }

    pub(super) fn check_condition(&mut self, condition: &AstNode, keyword: &str) -> CheckResult<()> {
        let ty = self.check_value(condition, Some(&Type::Boolean))?;
        if !matches!(ty, Type::Boolean | Type::Unknown | Type::Never) {
            return self.err(format!(
                "the condition of '{}' must be boolean, found {}",
                keyword,
                self.show(&ty)
            ));
        }
        Ok(())
    }

    pub(super) fn check_return(&mut self, value: Option<&AstNode>) -> CheckResult<()> {
        let declared = match self.state.returns.last() {
            Some(context) => context.declared.clone(),
            None => return self.err("'return' outside of a function".to_string()),
        };
        let value_ty = match value {
            Some(value) => Some(self.check_value(value, declared.as_ref())?),
            None => None,
        };
        match (declared, value_ty) {
            (Some(Type::Void), Some(_)) => {
                self.err("cannot return a value from a function without a return type".to_string())
            }
            (Some(Type::Void), None) => Ok(()),
            (Some(declared), None) => self.err(format!(
                "this function must return a value of type {}",
                self.show(&declared)
            )),
            (Some(declared), Some(actual)) => {
                if self.assignable(&declared, &actual) {
                    Ok(())
                } else {
                    self.err(format!(
                        "returning {}, but the function returns {}",
                        self.show(&actual),
                        self.show(&declared)
                    ))
                }
            }
            (None, actual) => {
                let actual = actual.unwrap_or(Type::Void);
                let previous = self.state.returns.last().and_then(|c| c.inferred.clone());
                let inferred = match previous {
                    None => actual,
                    Some(previous) => match self.join(&previous, &actual) {
                        Some(joined) => joined,
                        None => {
                            return self.err(format!(
                                "inconsistent return types: {} and {}",
                                self.show(&previous),
                                self.show(&actual)
                            ))
                        }
                    },
                };
                if let Some(context) = self.state.returns.last_mut() {
                    context.inferred = Some(inferred);
                }
                Ok(())
            }
        }
    }

    pub(super) fn check_assign(&mut self, target: &AstNode, op: &AssignOperator, value: &AstNode) -> CheckResult<()> {
        match target {
            AstNode::Identifier(name) => {
                let existing = self.lookup(name).cloned();
                let symbol = match existing {
                    Some(symbol) => symbol,
                    None => {
                        if *op != AssignOperator::Assign {
                            return self.err(format!("undefined variable '{}'", name));
                        }
                        // Assigning to a new name declares a mutable variable.
                        let ty = self.check_value(value, None)?;
                        return self.declare(name, Symbol::value(ty, false));
                    }
                };
                if let Some(owner) = &symbol.owner {
                    return self.err(format!("'{}' is read-only outside '{}'", name, owner));
                }
                self.ensure_not_captured(name)?;
                if !matches!(symbol.kind, SymbolKind::Value) || matches!(symbol.ty, Type::Module(_)) {
                    return self.err(format!("cannot assign to '{}'", name));
                }
                if symbol.is_imut {
                    return self.err(format!("cannot modify '{}': {}", name, self.imut_reason(&symbol)));
                }
                if symbol.is_const {
                    if let Some(reason) = symbol.reason {
                        return self.err(format!("cannot modify '{}': {}", name, reason));
                    }
                    return self.err(format!("cannot modify a 'const' value ('{}')", name));
                }
                let new_ty = self.check_assigned_value(&symbol.ty, op, value, name)?;
                if symbol.ty.contains_unknown() {
                    let refined = self.join(&symbol.ty, &new_ty).unwrap_or(symbol.ty.clone());
                    if let Some(symbol) = self.lookup_mut(name) {
                        symbol.ty = refined;
                    }
                }
                Ok(())
            }
            AstNode::Index { target: inner, index } => {
                self.ensure_contents_mutable(inner)?;
                let container = self.check_value(inner, None)?;
                let index_ty = self.check_value(index, None)?;
                let element = match &container {
                    Type::List(element) => {
                        self.expect_type(&Type::Int, &index_ty, "list index")?;
                        (**element).clone()
                    }
                    Type::Unknown => Type::Unknown,
                    Type::Named { id, .. } => match &self.table.get(*id).kind {
                        TypeKind::Dict { key, value } => {
                            let (key, value) = (key.clone(), value.clone());
                            self.expect_type(&key, &index_ty, "dictionary key")?;
                            value
                        }
                        _ => return self.err(format!("cannot assign by index to {}", self.show(&container))),
                    },
                    Type::String => return self.err("strings cannot be modified".to_string()),
                    other => return self.err(format!("cannot assign by index to {}", self.show(other))),
                };
                self.check_assigned_value(&element, op, value, "element")?;
                Ok(())
            }
            AstNode::Member { object, name } => {
                self.ensure_contents_mutable(object)?;
                let object_ty = self.check_value(object, None)?;
                let field_ty = match &object_ty {
                    Type::Unknown => Type::Unknown,
                    Type::Module(id) => {
                        return self.err(format!(
                            "'{}' is read-only outside '{}'",
                            name, self.modules[*id].display
                        ))
                    }
                    Type::Variant { .. } => {
                        return self.err(format!("variant fields cannot be modified ('{}')", name))
                    }
                    Type::Named { id, .. }
                        if matches!(self.table.get(*id).kind, TypeKind::Builtin { .. }) =>
                    {
                        return self.err(format!("'{}' is read-only", name))
                    }
                    other => match self.table.fields_of(other) {
                        Some(fields) => match fields.into_iter().find(|(n, _)| n == name) {
                            Some((_, ty)) => ty,
                            None => {
                                return self.err(format!(
                                    "{} has no field '{}'",
                                    self.show(other),
                                    name
                                ))
                            }
                        },
                        None => {
                            return self.err(format!(
                                "cannot assign field '{}' on {}",
                                name,
                                self.show(other)
                            ))
                        }
                    },
                };
                self.check_assigned_value(&field_ty, op, value, name)?;
                Ok(())
            }
            _ => self.err("invalid assignment target".to_string()),
        }
    }

    /// Changing a field or an element (`person.age = 31`, `items[0] = 1`) is not allowed when the
    /// variable that holds the value is `imut`.
    pub(super) fn ensure_contents_mutable(&self, target: &AstNode) -> CheckResult<()> {
        let mut node = target.unlocated();
        loop {
            match node {
                AstNode::Member { object, .. } => node = object.unlocated(),
                AstNode::Index { target, .. } => node = target.unlocated(),
                AstNode::Identifier(name) => {
                    return match self.lookup(name) {
                        Some(symbol) if symbol.is_imut && matches!(symbol.kind, SymbolKind::Value) => {
                            self.err(format!("cannot modify '{}': {}", name, self.imut_reason(symbol)))
                        }
                        _ => self.ensure_not_captured(name),
                    };
                }
                _ => return Ok(()),
            }
        }
    }

    pub(super) fn imut_reason(&self, symbol: &Symbol) -> &'static str {
        symbol.reason.unwrap_or("it is imut")
    }

    /// What can be passed as `mut`: a `var` (not `const` or `imut`), or a field or an element
    /// of a value that can be modified. Returns its type.
    ///
    /// For the value a `mut self` method is called on (`receiver` is the method), a `const` is
    /// accepted too: the method can change the contents, not replace the value.
    pub(super) fn check_mut_place(&mut self, place: &AstNode, receiver: Option<&str>) -> CheckResult<Type> {
        let what = |name: &str| match receiver {
            Some(method) => format!("cannot call '{}' on '{}', as it changes the value", method, name),
            None => format!("cannot pass '{}' as 'mut'", name),
        };
        match place.unlocated() {
            AstNode::Identifier(name) => {
                let symbol = match self.lookup(name) {
                    Some(symbol) => symbol.clone(),
                    None => return self.err(format!("undefined variable '{}'", name)),
                };
                if let Some(owner) = &symbol.owner {
                    return self.err(format!("'{}' is read-only outside '{}'", name, owner));
                }
                self.ensure_not_captured(name)?;
                if !matches!(symbol.kind, SymbolKind::Value) || matches!(symbol.ty, Type::Module(_)) {
                    return self.err(what(name));
                }
                if symbol.is_imut {
                    return self.err(format!("{}: {}", what(name), self.imut_reason(&symbol)));
                }
                if symbol.is_const && receiver.is_none() {
                    return self.err(format!("{}: it is const", what(name)));
                }
                Ok(symbol.ty)
            }
            AstNode::Member { object, name } => {
                self.ensure_contents_mutable(object)?;
                let object_ty = self.check_value(object, None)?;
                let read_only = match &object_ty {
                    Type::Module(_) | Type::Variant { .. } => true,
                    Type::Named { id, .. } => matches!(self.table.get(*id).kind, TypeKind::Builtin { .. }),
                    _ => false,
                };
                if read_only {
                    return self.err(format!("{}: it is read-only", what(name)));
                }
                self.check_member(object, name)
            }
            AstNode::Index { target, .. } => {
                self.ensure_contents_mutable(target)?;
                if self.check_value(target, None)? == Type::String {
                    return self.err("strings cannot be modified".to_string());
                }
                self.check_value(place, None)
            }
            _ => match receiver {
                Some(method) => self.err(format!(
                    "'{}' changes the value it is called on: call it on a variable, a field or an element",
                    method
                )),
                None => self.err("only a variable, a field or an element can be passed as 'mut'".to_string()),
            },
        }
    }

    /// Checks `target op= value` where the target has type `target_ty`; returns the value type.
    pub(super) fn check_assigned_value(
        &mut self,
        target_ty: &Type,
        op: &AssignOperator,
        value: &AstNode,
        name: &str,
    ) -> CheckResult<Type> {
        let value_ty = match op.binary() {
            None => self.check_value(value, Some(target_ty))?,
            Some(binary) => {
                let right = self.check_value(value, None)?;
                self.binary_type(binary, target_ty, &right)?
            }
        };
        if !self.assignable(target_ty, &value_ty) {
            return self.err(format!(
                "cannot assign a value of type {} to '{}' of type {}",
                self.show(&value_ty),
                name,
                self.show(target_ty)
            ));
        }
        Ok(value_ty)
    }
}
