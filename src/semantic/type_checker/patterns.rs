//! `match`: patterns and exhaustiveness.

use super::*;

impl Checker {
    pub(super) fn check_match(&mut self, scrutinee: &AstNode, arms: &[MatchArm], expected: Option<&Type>) -> CheckResult<Type> {
        let scrutinee_ty = self.check_value(scrutinee, None)?;
        let mut arm_types = Vec::new();
        for arm in arms {
            self.push_scope();
            let result = (|| -> CheckResult<Type> {
                self.check_pattern(&arm.pattern, &scrutinee_ty)?;
                if let Some(guard) = &arm.guard {
                    self.check_condition(guard, "match guard")?;
                }
                self.check_block(&arm.body, expected, vec![])
            })();
            self.pop_scope();
            arm_types.push(result?);
        }
        self.check_exhaustive(&scrutinee_ty, arms)?;
        self.branch_types(&arm_types, expected, "match arm")
    }

    /// Whether `name` in a pattern refers to a variant of `ty`.
    pub(super) fn variant_of(&self, ty: &Type, name: &str) -> Option<(TypeId, Vec<Type>, VariantDef)> {
        let (id, args) = match ty {
            Type::Named { id, args } | Type::Variant { id, args, .. } => (*id, args.clone()),
            _ => return None,
        };
        self.table
            .variants_of(id)?
            .iter()
            .find(|v| v.name == name)
            .map(|v| (id, args, v.clone()))
    }

    pub(super) fn is_constructor_name(&self, name: &str) -> bool {
        matches!(
            self.lookup(name),
            Some(Symbol {
                kind: SymbolKind::Constructor { .. },
                ..
            })
        )
    }

    pub(super) fn check_pattern(&mut self, pattern: &Pattern, ty: &Type) -> CheckResult<()> {
        match pattern {
            Pattern::Wildcard => Ok(()),
            Pattern::Identifier(name) => {
                if let Some((_, _, variant)) = self.variant_of(ty, name) {
                    if !variant.fields.is_empty() {
                        return self.err(format!(
                            "variant '{}' has fields; use '{}(...)' in the pattern",
                            name, name
                        ));
                    }
                    return Ok(());
                }
                if self.is_constructor_name(name) && !ty.is_dynamic() {
                    return self.err(format!(
                        "pattern '{}' does not match a value of type {}",
                        name,
                        self.show(ty)
                    ));
                }
                if ty.is_dynamic() && self.is_constructor_name(name) {
                    return Ok(());
                }
                let mut symbol = Symbol::value(ty.clone(), true);
                symbol.is_imut = true;
                symbol.reason = Some("variables bound by 'match' are imut");
                self.current_scope().vars.insert(name.clone(), symbol);
                Ok(())
            }
            Pattern::Variant { name, args } => {
                if ty.is_dynamic() {
                    for arg in args {
                        self.check_pattern(arg, &Type::Unknown)?;
                    }
                    return Ok(());
                }
                let (_, type_args, variant) = match self.variant_of(ty, name) {
                    Some(found) => found,
                    None => {
                        return self.err(format!(
                            "'{}' is not a variant of {}",
                            name,
                            self.show(ty)
                        ))
                    }
                };
                if variant.fields.len() != args.len() {
                    return self.err(format!(
                        "variant '{}' has {} field(s), but the pattern has {}",
                        name,
                        variant.fields.len(),
                        args.len()
                    ));
                }
                for ((_, field_ty), arg) in variant.fields.iter().zip(args) {
                    self.check_pattern(arg, &field_ty.substitute(&type_args))?;
                }
                Ok(())
            }
            Pattern::Literal(literal) => {
                let literal_ty = self.check_expr(literal, None)?;
                if !self.assignable(ty, &literal_ty) && !self.assignable(&literal_ty, ty) {
                    return self.err(format!(
                        "pattern of type {} does not match a value of type {}",
                        self.show(&literal_ty),
                        self.show(ty)
                    ));
                }
                Ok(())
            }
        }
    }

    /// Pattern matches every value of its type.
    pub(super) fn irrefutable(&self, pattern: &Pattern, ty: &Type) -> bool {
        match pattern {
            Pattern::Wildcard => true,
            Pattern::Identifier(name) => self.variant_of(ty, name).is_none() && !self.is_constructor_name(name),
            _ => false,
        }
    }

    pub(super) fn check_exhaustive(&self, ty: &Type, arms: &[MatchArm]) -> CheckResult<()> {
        let rows: Vec<Vec<&Pattern>> = arms
            .iter()
            .filter(|a| a.guard.is_none())
            .map(|a| vec![&a.pattern])
            .collect();
        match self.missing_pattern(std::slice::from_ref(ty), rows) {
            None => Ok(()),
            Some(witness) => {
                let witness = witness.into_iter().next().unwrap_or_else(|| "_".to_string());
                if witness == "_" {
                    self.err("match is not exhaustive: add a wildcard arm '_ => ...'".to_string())
                } else {
                    self.err(format!(
                        "match is not exhaustive: {} is not handled (add an arm for it or a wildcard '_ => ...')",
                        witness
                    ))
                }
            }
        }
    }

    /// Constructors of a type with finitely many of them: name and field types.
    pub(super) fn constructors(&self, ty: &Type) -> Option<Vec<(String, Vec<Type>)>> {
        match ty {
            Type::Named { id, args } | Type::Variant { id, args, .. } => {
                let variants = self.table.variants_of(*id)?;
                let only = match ty {
                    Type::Variant { variant, .. } => Some(variant),
                    _ => None,
                };
                Some(
                    variants
                        .iter()
                        .filter(|v| only.is_none_or(|o| *o == v.name))
                        .map(|v| {
                            (
                                v.name.clone(),
                                v.fields.iter().map(|(_, t)| t.substitute(args)).collect(),
                            )
                        })
                        .collect(),
                )
            }
            Type::Boolean => Some(vec![
                ("true".to_string(), vec![]),
                ("false".to_string(), vec![]),
            ]),
            _ => None,
        }
    }

    /// Classic usefulness check on a pattern matrix: returns an example of values (one per
    /// column) that no row matches, or `None` when the rows are exhaustive.
    pub(super) fn missing_pattern(&self, types: &[Type], rows: Vec<Vec<&Pattern>>) -> Option<Vec<String>> {
        let (first, rest) = match types.split_first() {
            None => return if rows.is_empty() { Some(vec![]) } else { None },
            Some(split) => split,
        };
        if first.is_dynamic() {
            return None;
        }
        let wildcard = Pattern::Wildcard;

        let head_name = |pattern: &Pattern| -> Option<String> {
            match pattern {
                Pattern::Identifier(n) | Pattern::Variant { name: n, .. } => Some(n.clone()),
                Pattern::Literal(AstNode::BooleanLiteral(b)) => Some(b.to_string()),
                _ => None,
            }
        };
        let constructors = self.constructors(first).filter(|constructors| {
            // Split by constructor only when the column mentions all of them; otherwise the
            // rows with an irrefutable head (the default matrix) must cover the rest.
            !constructors.is_empty()
                && constructors.iter().all(|(name, _)| {
                    rows.iter().any(|row| {
                        !self.irrefutable(row[0], first) && head_name(row[0]).as_deref() == Some(name)
                    })
                })
        });
        if let Some(constructors) = constructors {
            for (name, fields) in constructors {
                let mut specialized: Vec<Vec<&Pattern>> = Vec::new();
                for row in &rows {
                    let head = row[0];
                    let args: Option<Vec<&Pattern>> = match head {
                        _ if self.irrefutable(head, first) => Some(vec![&wildcard; fields.len()]),
                        Pattern::Identifier(n) if *n == name => Some(vec![]),
                        Pattern::Variant { name: n, args } if *n == name => Some(args.iter().collect()),
                        Pattern::Literal(AstNode::BooleanLiteral(b)) if b.to_string() == name => Some(vec![]),
                        _ => None,
                    };
                    if let Some(mut args) = args {
                        args.extend(row[1..].iter().copied());
                        specialized.push(args);
                    }
                }
                let mut sub_types = fields.clone();
                sub_types.extend(rest.iter().cloned());
                if let Some(witness) = self.missing_pattern(&sub_types, specialized) {
                    let (inner, outer) = witness.split_at(fields.len());
                    let head = if inner.is_empty() {
                        name
                    } else {
                        format!("{}({})", name, inner.join(", "))
                    };
                    let mut result = vec![head];
                    result.extend(outer.iter().cloned());
                    return Some(result);
                }
            }
            return None;
        }

        // Only rows with an irrefutable head cover the values not mentioned in the column.
        let defaults: Vec<Vec<&Pattern>> = rows
            .iter()
            .filter(|row| self.irrefutable(row[0], first))
            .map(|row| row[1..].to_vec())
            .collect();
        let missing_head = self.constructors(first).and_then(|constructors| {
            constructors
                .into_iter()
                .find(|(name, _)| {
                    !rows.iter().any(|row| {
                        !self.irrefutable(row[0], first) && head_name(row[0]).as_deref() == Some(name)
                    })
                })
                .map(|(name, fields)| {
                    if fields.is_empty() {
                        name
                    } else {
                        format!("{}({})", name, vec!["_"; fields.len()].join(", "))
                    }
                })
        });
        self.missing_pattern(rest, defaults).map(|witness| {
            let mut result = vec![missing_head.unwrap_or_else(|| "_".to_string())];
            result.extend(witness);
            result
        })
    }
}
