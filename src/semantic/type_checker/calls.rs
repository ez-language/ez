//! Calls: functions, methods, associated functions, constructors and `mut` arguments.

use super::*;

impl Checker {
    pub(super) fn check_call(
        &mut self,
        callee: &AstNode,
        args: &[AstNode],
        method: &RefCell<Option<MethodRef>>,
        expected: Option<&Type>,
    ) -> CheckResult<Type> {
        match callee.unlocated() {
            AstNode::Identifier(name) => {
                if let Some(symbol) = self.lookup(name).cloned() {
                    match symbol.kind {
                        SymbolKind::Builtin(builtin) => return self.check_builtin_call(&builtin, args),
                        SymbolKind::Constructor { id, variant } => {
                            return self.check_constructor(id, &variant, args, expected)
                        }
                        SymbolKind::Value if !symbol.mut_params.is_empty() => {
                            return self.check_function_call_mut(&symbol.ty, &symbol.mut_params, args)
                        }
                        SymbolKind::Value => {}
                    }
                }
            }
            AstNode::Member { object, name } => {
                // `Person.create(...)`: an associated function.
                if let Some(id) = self.type_path(object) {
                    return self.check_associated_call(id, name, args, method);
                }
                let object_ty = self.check_value(object, None)?;
                if let Type::Named { id, .. } | Type::Variant { id, .. } = &object_ty {
                    if let Some(info) = self.methods.get(&(*id, name.clone())).cloned() {
                        return self.check_method(object, *id, name, info, args, method);
                    }
                }
                return self.check_method_call(&object_ty, name, args, expected);
            }
            _ => {}
        }
        let callee_ty = self.check_value(callee, None)?;
        self.check_function_call(&callee_ty, args)
    }

    /// The type named by `Person` or `module.Person` when used before a `.`, if it is a type
    /// and not a value.
    pub(super) fn type_path(&self, node: &AstNode) -> Option<TypeId> {
        let found = match node.unlocated() {
            AstNode::Identifier(name) if self.lookup(name).is_none() => self.lookup_type(name).cloned(),
            AstNode::Member { object, name } => match object.unlocated() {
                AstNode::Identifier(module) => match self.lookup(module).map(|s| &s.ty) {
                    Some(Type::Module(id)) if !self.modules[*id].members.contains_key(name) => {
                        self.modules[*id].types.get(name).cloned()
                    }
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
        match found {
            Some(TypeRef::Type(Type::Named { id, .. })) => Some(id),
            _ => None,
        }
    }

    /// The `display` method declared for the type, used by `print` and interpolation.
    pub(super) fn display_method(&self, ty: &Type) -> Option<MethodRef> {
        match ty {
            Type::Named { id, .. } | Type::Variant { id, .. } => {
                self.methods.get(&(*id, "display".to_string())).map(|info| MethodRef {
                    module: info.module.clone(),
                    key: self.method_name(*id, "display"),
                    has_self: true,
                })
            }
            _ => None,
        }
    }

    pub(super) fn method_name(&self, id: TypeId, name: &str) -> String {
        format!("{}.{}", self.table.get(id).name, name)
    }

    /// `Person.create('Ana')`
    pub(super) fn check_associated_call(
        &mut self,
        id: TypeId,
        name: &str,
        args: &[AstNode],
        method: &RefCell<Option<MethodRef>>,
    ) -> CheckResult<Type> {
        let full = self.method_name(id, name);
        let info = match self.methods.get(&(id, name.to_string())) {
            Some(info) => info.clone(),
            None => return self.err(format!("'{}' is not declared", full)),
        };
        if info.has_self {
            return self.err(format!(
                "'{}' takes 'self': call it on a value ('value.{}(...)')",
                full, name
            ));
        }
        *method.borrow_mut() = Some(MethodRef {
            module: info.module.clone(),
            key: full,
            has_self: false,
        });
        self.check_function_call_mut(&info.ty, &info.mut_params, args)
    }

    /// `person.greet()`
    pub(super) fn check_method(
        &mut self,
        object: &AstNode,
        id: TypeId,
        name: &str,
        info: MethodInfo,
        args: &[AstNode],
        method: &RefCell<Option<MethodRef>>,
    ) -> CheckResult<Type> {
        let full = self.method_name(id, name);
        if !info.has_self {
            return self.err(format!(
                "'{}' has no 'self': call it on the type ('{}(...)')",
                full, full
            ));
        }
        // `mut self`: the call needs no marker, but the value must be one that can change.
        if info.mut_params[0] {
            self.check_mut_place(object, Some(&full))?;
        }
        *method.borrow_mut() = Some(MethodRef {
            module: info.module.clone(),
            key: full,
            has_self: true,
        });
        let ty = match info.ty {
            Type::Function { params, ret } => Type::Function {
                params: params[1..].to_vec(),
                ret,
            },
            other => other,
        };
        self.check_function_call_mut(&ty, &info.mut_params[1..], args)
    }

    pub(super) fn check_function_call(&mut self, callee_ty: &Type, args: &[AstNode]) -> CheckResult<Type> {
        self.check_function_call_mut(callee_ty, &[], args)
    }

    /// A call to a function whose parameters marked in `mut_params` are `mut`.
    pub(super) fn check_function_call_mut(
        &mut self,
        callee_ty: &Type,
        mut_params: &[bool],
        args: &[AstNode],
    ) -> CheckResult<Type> {
        match callee_ty {
            Type::Function { params, ret } => {
                self.check_args_mut(params, mut_params, args)?;
                Ok((**ret).clone())
            }
            Type::Unknown => {
                for arg in args {
                    self.check_value(arg, None)?;
                }
                Ok(Type::Unknown)
            }
            other => self.err(format!("a value of type {} cannot be called", self.show(other))),
        }
    }

    pub(super) fn check_args(&mut self, params: &[Type], args: &[AstNode]) -> CheckResult<Vec<Type>> {
        self.check_args_mut(params, &[], args)
    }

    pub(super) fn check_args_mut(&mut self, params: &[Type], mut_params: &[bool], args: &[AstNode]) -> CheckResult<Vec<Type>> {
        if params.len() != args.len() {
            return self.err(format!(
                "expected {} argument(s), found {}",
                params.len(),
                args.len()
            ));
        }
        let mut types = Vec::new();
        for (i, (param, arg)) in params.iter().zip(args).enumerate() {
            let is_mut = mut_params.get(i).copied().unwrap_or(false);
            match (arg.unlocated(), is_mut) {
                (AstNode::MutArg(place), true) => {
                    let ty = self.check_mut_place(place, None)?;
                    // The function may store any value of the parameter type in the variable,
                    // so the types must be the same.
                    if !self.assignable(param, &ty) || !self.assignable(&ty, param) {
                        return self.err(format!(
                            "argument {} is 'mut' and must be exactly {}, found {}",
                            i + 1,
                            self.show(param),
                            self.show(&ty)
                        ));
                    }
                    types.push(ty);
                    continue;
                }
                (AstNode::MutArg(_), false) => {
                    return self.err(format!(
                        "argument {} is not a 'mut' parameter: remove 'mut'",
                        i + 1
                    ))
                }
                (_, true) => {
                    return self.err(format!(
                        "argument {} is a 'mut' parameter: pass a variable with 'mut' ('f(mut x)')",
                        i + 1
                    ))
                }
                _ => {}
            }
            let ty = self.check_value(arg, Some(param))?;
            if !self.assignable(param, &ty) {
                let hint = match (param, &ty) {
                    (Type::Named { .. }, Type::Named { .. })
                        if self.table.fields_of(param).is_some() && self.table.fields_of(&ty).is_some() =>
                    {
                        format!(
                            " ({} is not a subtype of {}: ez has no subtyping, pass the fields explicitly)",
                            self.show(&ty),
                            self.show(param)
                        )
                    }
                    _ => String::new(),
                };
                return self.err(format!(
                    "argument {} must be {}, found {}{}",
                    i + 1,
                    self.show(param),
                    self.show(&ty),
                    hint
                ));
            }
            types.push(ty);
        }
        Ok(types)
    }

    pub(super) fn check_constructor(
        &mut self,
        id: TypeId,
        variant: &str,
        args: &[AstNode],
        expected: Option<&Type>,
    ) -> CheckResult<Type> {
        let info = self.table.get(id).clone();
        let fields = match info.kind {
            TypeKind::Sum(variants) => variants
                .into_iter()
                .find(|v| v.name == variant)
                .map(|v| v.fields)
                .unwrap_or_default(),
            _ => vec![],
        };
        if fields.is_empty() {
            return self.err(format!("variant '{}' has no fields and is used without '()'", variant));
        }
        if fields.len() != args.len() {
            return self.err(format!(
                "'{}' expects {} argument(s), found {}",
                variant,
                fields.len(),
                args.len()
            ));
        }
        // Generic arguments: start from the expected type, then infer from the arguments.
        let mut generic: Vec<Type> = match expected {
            Some(Type::Named { id: expected_id, args }) if *expected_id == id => args.clone(),
            _ => vec![Type::Unknown; info.params],
        };
        for ((field, field_ty), arg) in fields.iter().zip(args) {
            let wanted = field_ty.substitute(&generic);
            let actual = self.check_value(arg, Some(&wanted))?;
            if let Type::Param(i) = field_ty {
                if generic[*i].contains_unknown() {
                    generic[*i] = self.join(&generic[*i], &actual).unwrap_or(actual.clone());
                    continue;
                }
            }
            if !self.assignable(&wanted, &actual) {
                return self.err(format!(
                    "field '{}' of '{}' must be {}, found {}",
                    field,
                    variant,
                    self.show(&wanted),
                    self.show(&actual)
                ));
            }
        }
        Ok(Type::Named { id, args: generic })
    }
}
