//! Expressions: literals, operators, members, `?` and `is`.

use super::*;

impl Checker {
    /// Checks an expression whose value is used: it must not be `void`.
    pub(super) fn check_value(&mut self, node: &AstNode, expected: Option<&Type>) -> CheckResult<Type> {
        let ty = self.check_expr(node, expected)?;
        if ty == Type::Void {
            let hint = match node.unlocated() {
                AstNode::If { .. } | AstNode::Match { .. } => {
                    " (to be used as a value, every branch must produce a value of the same type, and 'if' needs an 'else')"
                }
                _ => "",
            };
            return self.err(format!("this expression has no value{}", hint));
        }
        Ok(ty)
    }

    pub(super) fn check_expr(&mut self, node: &AstNode, expected: Option<&Type>) -> CheckResult<Type> {
        match node {
            AstNode::Located { line, node } => {
                self.state.line = *line;
                self.check_expr(node, expected)
            }
            AstNode::IntLiteral(_) => Ok(Type::Int),
            AstNode::FloatLiteral(_) => Ok(Type::Float),
            AstNode::StringLiteral(_) => Ok(Type::String),
            AstNode::BooleanLiteral(_) => Ok(Type::Boolean),
            AstNode::TemplateString(parts) => {
                for part in parts {
                    if let TemplatePart::Expression(expression, display) = part {
                        let ty = self.check_value(expression, None)?;
                        *display.borrow_mut() = self.display_method(&ty);
                    }
                }
                Ok(Type::String)
            }
            AstNode::ArrayLiteral(elements) => {
                let expected_element = match expected {
                    Some(Type::List(inner)) => Some((**inner).clone()),
                    _ => None,
                };
                let mut element_ty = Type::Unknown;
                for element in elements {
                    let ty = self.check_value(element, expected_element.as_ref())?;
                    element_ty = match self.join(&element_ty, &ty) {
                        Some(joined) => joined,
                        None => {
                            return self.err(format!(
                                "list elements must have the same type: found {} and {}",
                                self.show(&element_ty),
                                self.show(&ty)
                            ))
                        }
                    };
                }
                if let Some(expected_element) = expected_element {
                    if self.assignable(&expected_element, &element_ty) {
                        element_ty = expected_element;
                    }
                }
                Ok(Type::List(Box::new(element_ty)))
            }
            AstNode::ObjectLiteral(entries) => self.check_object(entries, expected),
            AstNode::Identifier(name) => self.check_identifier(name, expected),
            AstNode::Unary { op, operand } => {
                let ty = self.check_value(operand, None)?;
                match (op, &ty) {
                    (_, Type::Unknown) => Ok(Type::Unknown),
                    (UnaryOperator::Negate, Type::Int | Type::Float) => Ok(ty),
                    (UnaryOperator::Not, Type::Boolean) => Ok(Type::Boolean),
                    (UnaryOperator::BitNot, Type::Int) => Ok(Type::Int),
                    (op, ty) => {
                        let symbol = match op {
                            UnaryOperator::Negate => "-",
                            UnaryOperator::Not => "not",
                            UnaryOperator::BitNot => "~",
                        };
                        self.err(format!("operator '{}' cannot be applied to {}", symbol, self.show(ty)))
                    }
                }
            }
            AstNode::BinaryOp { left, op, right } => {
                if *op == BinaryOperator::Is {
                    if let Some(result) = self.check_variant_test(left, right)? {
                        return Ok(result);
                    }
                }
                let left_ty = self.check_value(left, None)?;
                let right_ty = self.check_value(right, None)?;
                self.binary_type(*op, &left_ty, &right_ty)
            }
            AstNode::Call {
                callee,
                args,
                method,
                displays,
            } => {
                let ty = self.check_call(callee, args, method, expected)?;
                // `print(person)` and `string(person)` use `Person.display`, if declared.
                if let AstNode::Identifier(name) = callee.unlocated() {
                    let is_display = matches!(
                        self.lookup(name).map(|s| &s.kind),
                        Some(SymbolKind::Builtin(builtin)) if builtin == "print" || builtin == "string"
                    );
                    if is_display {
                        let types = std::mem::take(&mut self.last_arg_types);
                        *displays.borrow_mut() = types.iter().map(|t| self.display_method(t)).collect();
                    }
                }
                Ok(ty)
            }
            AstNode::MutArg(_) => self.err("'mut' is only used before a call argument: 'f(mut x)'".to_string()),
            AstNode::Index { target, index } => {
                let target_ty = self.check_value(target, None)?;
                let index_ty = self.check_value(index, None)?;
                match &target_ty {
                    Type::List(element) => {
                        self.expect_type(&Type::Int, &index_ty, "list index")?;
                        Ok((**element).clone())
                    }
                    Type::String => {
                        self.expect_type(&Type::Int, &index_ty, "string index")?;
                        Ok(Type::String)
                    }
                    Type::Unknown => Ok(Type::Unknown),
                    Type::Named { id, .. } => match &self.table.get(*id).kind {
                        TypeKind::Dict { key, value } => {
                            let (key, value) = (key.clone(), value.clone());
                            self.expect_type(&key, &index_ty, "dictionary key")?;
                            Ok(value)
                        }
                        _ => self.err(format!("cannot index a value of type {}", self.show(&target_ty))),
                    },
                    other => self.err(format!("cannot index a value of type {}", self.show(other))),
                }
            }
            AstNode::Member { object, name } => self.check_member(object, name),
            AstNode::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.check_condition(condition, "if")?;
                let narrowed = self.narrowing(condition);
                match else_branch {
                    None => {
                        self.check_block(then_branch, None, narrowed)?;
                        Ok(Type::Void)
                    }
                    Some(else_branch) => {
                        let then_ty = self.check_block(then_branch, expected, narrowed)?;
                        let else_ty = self.check_block(else_branch, expected, vec![])?;
                        self.branch_types(&[then_ty, else_ty], expected, "if")
                    }
                }
            }
            AstNode::Match { scrutinee, arms } => self.check_match(scrutinee, arms, expected),
            AstNode::Lambda {
                params,
                return_type,
                body,
            } => {
                let signature =
                    self.function_signature("anonymous function", params, return_type.as_ref(), false)?;
                self.check_function("anonymous function", params, signature, body)
            }
            AstNode::Block(_) => self.check_block(node, expected, vec![]),
            AstNode::Try { value, wrap } => self.check_try(value, wrap),
            statement => self.check_statement(statement, expected),
        }
    }

    /// `value?` on a `Result` or an `Option`: its type is the `Ok` / `Some` value.
    pub(super) fn check_try(&mut self, value: &AstNode, wrap: &RefCell<Option<TryWrap>>) -> CheckResult<Type> {
        let ty = self.check_value(value, None)?;
        let (id, args) = match &ty {
            Type::Named { id, args } if *id == self.builtins.result || *id == self.builtins.option => {
                (*id, args.clone())
            }
            Type::Unknown => return Ok(Type::Unknown),
            other => {
                return self.err(format!(
                    "'?' can only be used on a Result or an Option, found {}",
                    self.show(other)
                ))
            }
        };
        let inner = args.first().cloned().unwrap_or(Type::Unknown);
        let is_result = id == self.builtins.result;
        let kind = if is_result { "a Result" } else { "an Option" };

        let declared = match self.state.returns.last() {
            // At the top level of the main file, a failure stops the program.
            None if self.state.is_entry => return Ok(inner),
            None => {
                return self.err(
                    "'?' at the top level is only allowed in the main file".to_string(),
                )
            }
            Some(context) => context.declared.clone(),
        };
        let declared = match declared {
            Some(declared) => declared,
            None => {
                return self.err(
                    "'?' needs the function to declare its return type".to_string(),
                )
            }
        };
        let returned_args = match &declared {
            Type::Named { id: returned, args } if *returned == id => args.clone(),
            other => {
                return self.err(format!(
                    "'?' on {} needs the function to return {} too, but it returns {}",
                    kind,
                    kind,
                    self.show(other)
                ))
            }
        };
        if !is_result {
            return Ok(inner);
        }

        let error = args.get(1).cloned().unwrap_or(Type::Unknown);
        let target = returned_args.get(1).cloned().unwrap_or(Type::Unknown);
        if self.assignable(&target, &error) {
            return Ok(inner);
        }
        *wrap.borrow_mut() = Some(self.error_wrapper(&error, &target)?);
        Ok(inner)
    }

    /// Automatic conversion of `?`: the variant of `target` (the function's error type) that
    /// holds exactly one field of type `error`. It must be the only such variant.
    pub(super) fn error_wrapper(&self, error: &Type, target: &Type) -> CheckResult<TryWrap> {
        let use_map_err = format!(
            "use '.mapErr(...)' to convert {} into {}",
            self.show(error),
            self.show(target)
        );
        let (target_id, target_args) = match target {
            Type::Named { id, args } if self.table.variants_of(*id).is_some() => (*id, args.clone()),
            _ => {
                return self.err(format!(
                    "'?' cannot convert {} into {}: {}",
                    self.show(error),
                    self.show(target),
                    use_map_err
                ))
            }
        };
        let candidates: Vec<String> = self
            .table
            .variants_of(target_id)
            .into_iter()
            .flatten()
            .filter(|variant| match variant.fields.as_slice() {
                [(_, field)] => {
                    let field = field.substitute(&target_args);
                    field == *error || (self.assignable(&field, error) && self.assignable(error, &field))
                }
                _ => false,
            })
            .map(|variant| variant.name.clone())
            .collect();
        let variant = match candidates.as_slice() {
            [variant] => variant.clone(),
            [] => {
                return self.err(format!(
                    "'?' cannot convert {} into {}: no variant of {} holds a {} ({})",
                    self.show(error),
                    self.show(target),
                    self.show(target),
                    self.show(error),
                    use_map_err
                ))
            }
            several => {
                return self.err(format!(
                    "'?' cannot choose how to convert {} into {}: the variants {} all hold a {} ({})",
                    self.show(error),
                    self.show(target),
                    several.join(", "),
                    self.show(error),
                    use_map_err
                ))
            }
        };

        // The runtime builds the variant by name: find how it is reachable from here.
        let is_target_constructor = |symbol: &Symbol| {
            matches!(&symbol.kind, SymbolKind::Constructor { id, .. } if *id == target_id)
        };
        if self.lookup(&variant).is_some_and(is_target_constructor) {
            return Ok(TryWrap {
                module: None,
                variant,
            });
        }
        for scope in self.state.scopes.iter().rev() {
            for (alias, symbol) in &scope.vars {
                if let Type::Module(module) = symbol.ty {
                    if self.modules[module]
                        .members
                        .get(&variant)
                        .is_some_and(is_target_constructor)
                    {
                        return Ok(TryWrap {
                            module: Some(alias.clone()),
                            variant,
                        });
                    }
                }
            }
        }
        self.err(format!(
            "'?' would wrap the error in '{}', but '{}' is not in scope here (import it)",
            variant, variant
        ))
    }

    /// `x is Some` inside an `if` condition: in the `then` branch, `x` is known to be that variant.
    pub(super) fn narrowing(&self, condition: &AstNode) -> Vec<(String, Symbol)> {
        if let AstNode::BinaryOp {
            left,
            op: BinaryOperator::Is,
            right,
        } = condition.unlocated()
        {
            if let (AstNode::Identifier(name), AstNode::Identifier(variant)) =
                (left.unlocated(), right.unlocated())
            {
                if let (Some(symbol), Some(Symbol {
                    kind: SymbolKind::Constructor { id: variant_type, .. },
                    ..
                })) = (self.lookup(name), self.lookup(variant))
                {
                    if let Type::Named { id, args } = &symbol.ty {
                        if id == variant_type {
                            let mut narrowed = symbol.clone();
                            narrowed.ty = Type::Variant {
                                id: *id,
                                args: args.clone(),
                                variant: variant.clone(),
                            };
                            return vec![(name.clone(), narrowed)];
                        }
                    }
                }
            }
        }
        vec![]
    }

    /// `value is Variant`: checks whether a sum-type value is a given variant.
    pub(super) fn check_variant_test(&mut self, left: &AstNode, right: &AstNode) -> CheckResult<Option<Type>> {
        let variant = match right.unlocated() {
            AstNode::Identifier(name) => name,
            _ => return Ok(None),
        };
        let variant_type = match self.lookup(variant) {
            Some(Symbol {
                kind: SymbolKind::Constructor { id, .. },
                ..
            }) => *id,
            _ => return Ok(None),
        };
        let left_ty = self.check_value(left, None)?;
        match &left_ty {
            Type::Named { id, .. } | Type::Variant { id, .. } if *id == variant_type => {
                Ok(Some(Type::Boolean))
            }
            Type::Unknown => Ok(Some(Type::Boolean)),
            other => self.err(format!(
                "'{}' is not a variant of {}",
                variant,
                self.show(other)
            )),
        }
    }

    pub fn binary_type(&self, op: BinaryOperator, left: &Type, right: &Type) -> CheckResult<Type> {
        use BinaryOperator::*;
        let comparison = matches!(op, Equal | NotEqual | Greater | GreaterEqual | Less | LessEqual | Is | In);
        if left.is_dynamic() || right.is_dynamic() {
            return Ok(if comparison || matches!(op, And | Or) {
                Type::Boolean
            } else {
                Type::Unknown
            });
        }
        let numeric = left.is_numeric() && right.is_numeric();
        let both_int = *left == Type::Int && *right == Type::Int;
        let result = match op {
            Add if both_int => Some(Type::Int),
            Add if numeric => Some(Type::Float),
            Add if *left == Type::String && *right == Type::String => Some(Type::String),
            Add => match (left, right) {
                (Type::List(_), Type::List(_)) => self.join(left, right),
                _ => None,
            },
            // Like Go and Rust: dividing ints gives an int, dividing floats gives a float.
            Subtract | Multiply | Divide | Modulo | Power if both_int => Some(Type::Int),
            Subtract | Multiply | Divide | Modulo | Power if numeric => Some(Type::Float),
            Multiply => match (left, right) {
                (Type::String, Type::Int) | (Type::Int, Type::String) => Some(Type::String),
                _ => None,
            },
            Greater | GreaterEqual | Less | LessEqual
                if numeric || (*left == Type::String && *right == Type::String) =>
            {
                Some(Type::Boolean)
            }
            Equal | NotEqual | Is
                if numeric || self.assignable(left, right) || self.assignable(right, left) =>
            {
                Some(Type::Boolean)
            }
            And | Or if *left == Type::Boolean && *right == Type::Boolean => Some(Type::Boolean),
            In => match right {
                Type::String if *left == Type::String => Some(Type::Boolean),
                Type::List(element) if self.assignable(element, left) || self.assignable(left, element) => {
                    Some(Type::Boolean)
                }
                Type::Named { id, .. } => match &self.table.get(*id).kind {
                    TypeKind::Dict { key, .. } if self.assignable(key, left) => Some(Type::Boolean),
                    _ => None,
                },
                _ => None,
            },
            BitAnd | BitOr | BitXor | ShiftLeft | ShiftRight if both_int => Some(Type::Int),
            _ => None,
        };
        match result {
            Some(ty) => Ok(ty),
            None => self.err(format!(
                "operator '{}' cannot be applied to {} and {}",
                op.symbol(),
                self.show(left),
                self.show(right)
            )),
        }
    }

    pub(super) fn check_identifier(&mut self, name: &str, expected: Option<&Type>) -> CheckResult<Type> {
        let symbol = match self.lookup(name) {
            Some(symbol) => symbol.clone(),
            None => return self.err(format!("undefined variable '{}'", name)),
        };
        match symbol.kind {
            SymbolKind::Builtin(builtin) => {
                self.err(format!("'{}' is a built-in function and must be called", builtin))
            }
            SymbolKind::Constructor { id, .. } => {
                // `None` takes its generic arguments from the expected type, when known.
                match (expected, &symbol.ty) {
                    (Some(Type::Named { id: expected_id, args }), Type::Named { .. })
                        if *expected_id == id =>
                    {
                        Ok(Type::Named {
                            id,
                            args: args.clone(),
                        })
                    }
                    _ => Ok(symbol.ty),
                }
            }
            SymbolKind::Value if !symbol.mut_params.is_empty() => self.err(format!(
                "'{}' has 'mut' parameters and can only be called, not used as a value",
                name
            )),
            SymbolKind::Value => Ok(symbol.ty),
        }
    }

    pub(super) fn check_object(&mut self, entries: &[crate::parser::ast::ObjectEntry], expected: Option<&Type>) -> CheckResult<Type> {
        let target = expected.and_then(|ty| match ty {
            Type::Named { id, .. } => Some((ty.clone(), self.table.get(*id).kind.clone())),
            _ => None,
        });
        match target {
            Some((ty, TypeKind::Shape(fields))) => {
                let type_name = self.show(&ty);
                for entry in entries {
                    match fields.iter().find(|(n, _)| *n == entry.key) {
                        Some((_, field_ty)) => {
                            let value_ty = self.check_value(&entry.value, Some(field_ty))?;
                            if !self.assignable(field_ty, &value_ty) {
                                return self.err(format!(
                                    "field '{}' of {} must be {}, found {}",
                                    entry.key,
                                    type_name,
                                    self.show(field_ty),
                                    self.show(&value_ty)
                                ));
                            }
                        }
                        None => {
                            return self.err(format!("{} has no field '{}'", type_name, entry.key))
                        }
                    }
                }
                let missing: Vec<&str> = fields
                    .iter()
                    .filter(|(n, _)| !entries.iter().any(|e| e.key == *n))
                    .map(|(n, _)| n.as_str())
                    .collect();
                if !missing.is_empty() {
                    return self.err(format!(
                        "missing field(s) {} in {}",
                        missing.join(", "),
                        type_name
                    ));
                }
                Ok(ty)
            }
            Some((ty, TypeKind::Dict { value, .. })) => {
                for entry in entries {
                    let value_ty = self.check_value(&entry.value, Some(&value))?;
                    if !self.assignable(&value, &value_ty) {
                        return self.err(format!(
                            "value of '{}' must be {}, found {}",
                            entry.key,
                            self.show(&value),
                            self.show(&value_ty)
                        ));
                    }
                }
                Ok(ty)
            }
            _ => {
                let mut fields = Vec::new();
                for entry in entries {
                    let ty = self.check_value(&entry.value, None)?;
                    fields.push((entry.key.clone(), ty));
                }
                Ok(Type::Record(fields))
            }
        }
    }

    pub(super) fn check_member(&mut self, object: &AstNode, name: &str) -> CheckResult<Type> {
        let object_ty = self.check_value(object, None)?;
        match &object_ty {
            Type::Module(id) => match self.modules[*id].members.get(name).cloned() {
                Some(symbol) => match symbol.kind {
                    SymbolKind::Builtin(builtin) => {
                        self.err(format!("'{}' is a built-in function and must be called", builtin))
                    }
                    _ if !symbol.mut_params.is_empty() => self.err(format!(
                        "'{}' has 'mut' parameters and can only be called, not used as a value",
                        name
                    )),
                    _ => Ok(symbol.ty),
                },
                None => self.missing_member(*id, name),
            },
            Type::Unknown => Ok(Type::Unknown),
            Type::String | Type::List(_) => {
                self.err(format!("method '{}' must be called: '{}()'", name, name))
            }
            other => match self.table.fields_of(other) {
                Some(fields) => match fields.into_iter().find(|(n, _)| n == name) {
                    Some((_, ty)) => Ok(ty),
                    None => self.err(format!("{} has no field '{}'", self.show(other), name)),
                },
                None => {
                    let hint = match other {
                        Type::Named { id, .. } if self.table.variants_of(*id).is_some() => {
                            " (use 'match' or check the variant with 'is' first)"
                        }
                        _ => "",
                    };
                    self.err(format!(
                        "a value of type {} has no field '{}'{}",
                        self.show(other),
                        name,
                        hint
                    ))
                }
            },
        }
    }
}
