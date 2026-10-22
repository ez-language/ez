//! Hoisting: imports, types, functions and methods declared in a block.

use super::*;

impl Checker {
    /// Declares the types, constructors and function signatures of a block before checking it,
    /// so they can be used before their definition (and recursively).
    pub(super) fn hoist(&mut self, statements: &[AstNode]) -> CheckResult<()> {
        // Imports first: types and function signatures may refer to imported modules.
        for statement in statements {
            self.set_line(statement);
            if let AstNode::Import { kind, path } = statement.unlocated() {
                self.check_import(kind, path)?;
            }
        }

        let mut pending: Vec<(&AstNode, Option<TypeId>)> = Vec::new();
        for statement in statements {
            self.set_line(statement);
            match statement.unlocated() {
                AstNode::TypeDecl {
                    name,
                    definition: TypeDef::Alias(_),
                } => {
                    self.check_type_name_free(name)?;
                    pending.push((statement, None));
                }
                AstNode::TypeDecl { name, .. } => {
                    self.check_type_name_free(name)?;
                    let id = self.table.add(name, 0, TypeKind::Pending);
                    self.current_scope()
                        .types
                        .insert(name.clone(), TypeRef::Type(Self::named(id)));
                    pending.push((statement, Some(id)));
                }
                _ => {}
            }
        }

        for (statement, id) in pending {
            self.set_line(statement);
            match (statement.unlocated(), id) {
                (
                    AstNode::TypeDecl {
                        name,
                        definition: TypeDef::Alias(expr),
                    },
                    None,
                ) => {
                    let ty = self.resolve_type(expr)?;
                    self.current_scope()
                        .types
                        .insert(name.clone(), TypeRef::Type(ty));
                }
                (AstNode::TypeDecl { name, definition }, Some(id)) => {
                    let kind = self.resolve_type_def(name, definition)?;
                    self.table.types[id].kind = kind;
                    if matches!(self.table.get(id).kind, TypeKind::Sum(_)) {
                        let mut scope = std::mem::take(self.current_scope());
                        self.declare_constructors_into(&mut scope, id);
                        *self.current_scope() = scope;
                    }
                }
                _ => {}
            }
        }

        for statement in statements {
            self.set_line(statement);
            if let AstNode::FunctionDecl {
                name,
                owner,
                params,
                return_type,
                ..
            } = statement.unlocated()
            {
                if let Some(owner) = owner {
                    self.declare_method(statements, statement, owner, name, params, return_type.as_ref())?;
                    continue;
                }
                let ty = self.function_signature(name, params, return_type.as_ref(), true)?;
                let mut symbol = Symbol::value(ty, true);
                if params.iter().any(|p| p.is_mut) {
                    symbol.mut_params = params.iter().map(|p| p.is_mut).collect();
                }
                self.declare(name, symbol)?;
            }
        }
        Ok(())
    }

    /// `function Person.greet(self)` or `function Person.create(name: string)`: only at the top
    /// level of the file that declares `Person`.
    pub(super) fn declare_method(
        &mut self,
        statements: &[AstNode],
        statement: &AstNode,
        owner: &str,
        name: &str,
        params: &[Param],
        return_type: Option<&TypeExpr>,
    ) -> CheckResult<()> {
        let full = format!("{}.{}", owner, name);
        if self.state.scopes.len() != 2 || !self.state.function_scopes.is_empty() {
            return self.err(format!(
                "'{}' must be declared at the top level of the file that declares '{}'",
                full, owner
            ));
        }
        if statement.is_exported() {
            return self.err(format!(
                "'{}' does not take 'export': methods are available wherever their type is",
                full
            ));
        }
        let declared_here = statements.iter().any(|s| {
            matches!(
                s.unlocated(),
                AstNode::TypeDecl { name, definition } if name == owner && !matches!(definition, TypeDef::Alias(_))
            )
        });
        let id = match self.current_scope().types.get(owner) {
            Some(TypeRef::Type(Type::Named { id, .. })) if declared_here => *id,
            _ => {
                return self.err(format!(
                    "cannot declare '{}': methods belong to a type with fields or variants declared in the same file",
                    full
                ))
            }
        };
        if self.methods.contains_key(&(id, name.to_string())) {
            return self.err(format!("'{}' is already declared", full));
        }
        let is_field = self
            .table
            .fields_of(&Self::named(id))
            .is_some_and(|fields| fields.iter().any(|(n, _)| n == name));
        let is_dict_length = name == "length" && matches!(self.table.get(id).kind, TypeKind::Dict { .. });
        if is_field || is_dict_length {
            return self.err(format!(
                "'{}' has the same name as a {} of '{}'",
                full,
                if is_field { "field" } else { "built-in method" },
                owner
            ));
        }

        let has_self = params.first().is_some_and(|p| p.name == "self");
        if name == "debug" {
            return self.err(format!(
                "cannot declare '{}': every type has 'debug()', which shows the structure of the value",
                full
            ));
        }
        if name == "display" {
            let returns_string = matches!(return_type, Some(TypeExpr::Named(t)) if t == "string");
            if !has_self || params.len() != 1 || params[0].is_mut || !returns_string {
                return self.err(format!(
                    "'{}' must be declared as 'function {}(self): string'",
                    full, full
                ));
            }
        }
        for (i, param) in params.iter().enumerate() {
            if param.name != "self" {
                continue;
            }
            if i > 0 {
                return self.err(format!("in '{}', 'self' must be the first parameter", full));
            }
            if param.ty.is_some() {
                return self.err(format!(
                    "'self' has no type annotation: in '{}' it is always '{}'",
                    full, owner
                ));
            }
        }
        let rest = if has_self { &params[1..] } else { params };
        let ty = match self.function_signature(&full, rest, return_type, true)? {
            Type::Function { mut params, ret } => {
                if has_self {
                    params.insert(0, Self::named(id));
                }
                Type::Function { params, ret }
            }
            other => other,
        };
        let info = MethodInfo {
            ty,
            has_self,
            mut_params: params.iter().map(|p| p.is_mut).collect(),
            module: self.state.path.clone(),
        };
        self.methods.insert((id, name.to_string()), info);
        Ok(())
    }

    pub(super) fn check_type_name_free(&mut self, name: &str) -> CheckResult<()> {
        if self.current_scope().types.contains_key(name) {
            return self.err(format!("type '{}' is already declared in this scope", name));
        }
        Ok(())
    }
}
