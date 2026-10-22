//! Type compatibility and the resolution of written types.

use super::*;

impl Checker {
    /// Whether a value of type `actual` can be used where `expected` is required.
    pub fn assignable(&self, expected: &Type, actual: &Type) -> bool {
        match (expected, actual) {
            (_, Type::Never | Type::Unknown) | (Type::Unknown, _) => true,
            (Type::Float, Type::Int) => true,
            (Type::List(e), Type::List(a)) => self.assignable(e, a),
            (Type::Named { id: e, args: ea }, Type::Named { id: a, args: aa })
            | (Type::Named { id: e, args: ea }, Type::Variant { id: a, args: aa, .. })
                if e == a =>
            {
                ea.iter().zip(aa).all(|(x, y)| self.assignable(x, y))
            }
            (
                Type::Variant {
                    id: e,
                    args: ea,
                    variant: ev,
                },
                Type::Variant {
                    id: a,
                    args: aa,
                    variant: av,
                },
            ) => e == a && ev == av && ea.iter().zip(aa).all(|(x, y)| self.assignable(x, y)),
            (Type::Named { id, .. }, actual) => match &self.table.get(*id).kind {
                // Shapes are nominal: only an object literal with the same fields (or a value of
                // the same type, handled above) is accepted.
                TypeKind::Shape(fields) => match actual {
                    Type::Record(actual_fields) => self.same_fields(fields, actual_fields),
                    _ => false,
                },
                TypeKind::Dict { value, .. } => match actual {
                    Type::Record(actual_fields) => {
                        actual_fields.iter().all(|(_, t)| self.assignable(value, t))
                    }
                    _ => false,
                },
                _ => false,
            },
            (Type::Record(fields), actual) => match self.table.fields_of(actual) {
                Some(actual_fields) => self.same_fields(fields, &actual_fields),
                None => false,
            },
            (
                Type::Function { params: ep, ret: er },
                Type::Function { params: ap, ret: ar },
            ) => {
                ep.len() == ap.len()
                    && ep.iter().zip(ap).all(|(e, a)| self.assignable(a, e))
                    && self.assignable(er, ar)
            }
            (e, a) => e == a,
        }
    }

    /// Same set of field names, with compatible types.
    pub(super) fn same_fields(&self, expected: &[(String, Type)], actual: &[(String, Type)]) -> bool {
        expected.len() == actual.len()
            && expected.iter().all(|(name, ty)| {
                actual
                    .iter()
                    .any(|(n, t)| n == name && self.assignable(ty, t))
            })
    }

    /// Common type of two branches, if there is one.
    pub(super) fn join(&self, a: &Type, b: &Type) -> Option<Type> {
        match (a, b) {
            _ if a == b => Some(a.clone()),
            (Type::Never, other) | (other, Type::Never) => Some(other.clone()),
            (Type::Unknown, other) | (other, Type::Unknown) => Some(other.clone()),
            (Type::Int, Type::Float) | (Type::Float, Type::Int) => Some(Type::Float),
            (Type::List(x), Type::List(y)) => Some(Type::List(Box::new(self.join(x, y)?))),
            (
                Type::Named { id: x, args: xa } | Type::Variant { id: x, args: xa, .. },
                Type::Named { id: y, args: ya } | Type::Variant { id: y, args: ya, .. },
            ) if x == y => {
                let args = xa
                    .iter()
                    .zip(ya)
                    .map(|(p, q)| self.join(p, q))
                    .collect::<Option<Vec<_>>>()?;
                Some(Type::Named { id: *x, args })
            }
            _ if self.assignable(a, b) => Some(a.clone()),
            _ if self.assignable(b, a) => Some(b.clone()),
            _ => None,
        }
    }

    pub(super) fn resolve_type(&self, expr: &TypeExpr) -> CheckResult<Type> {
        match expr {
            TypeExpr::Named(name) => match self.find_type(name)? {
                TypeRef::Type(ty) => Ok(ty),
                TypeRef::Generic(id) => {
                    let info = self.table.get(id);
                    self.err(format!(
                        "type '{}' needs {} type argument(s), e.g. {}[int]",
                        info.name, info.params, info.name
                    ))
                }
            },
            TypeExpr::Generic(name, args) => match self.find_type(name)? {
                TypeRef::Generic(id) => {
                    let expected = self.table.get(id).params;
                    if expected != args.len() {
                        return self.err(format!(
                            "type '{}' expects {} type argument(s), found {}",
                            name,
                            expected,
                            args.len()
                        ));
                    }
                    let args = args
                        .iter()
                        .map(|a| self.resolve_type(a))
                        .collect::<CheckResult<Vec<_>>>()?;
                    Ok(Type::Named { id, args })
                }
                TypeRef::Type(_) => self.err(format!("type '{}' is not generic", name)),
            },
            TypeExpr::Array(inner) => Ok(Type::List(Box::new(self.resolve_type(inner)?))),
            TypeExpr::Function { params, ret } => Ok(Type::Function {
                params: params
                    .iter()
                    .map(|p| self.resolve_type(p))
                    .collect::<CheckResult<Vec<_>>>()?,
                ret: Box::new(self.resolve_type(ret)?),
            }),
        }
    }

    pub(super) fn find_type(&self, name: &str) -> CheckResult<TypeRef> {
        if let Some((module, type_name)) = name.split_once('.') {
            let module_id = match self.lookup(module).map(|s| &s.ty) {
                Some(Type::Module(id)) => *id,
                _ => return self.err(format!("'{}' is not an imported module", module)),
            };
            return match self.modules[module_id].types.get(type_name) {
                Some(found) => Ok(found.clone()),
                None => self.err(format!("module '{}' has no type '{}'", module, type_name)),
            };
        }
        match self.lookup_type(name) {
            Some(found) => Ok(found.clone()),
            None => self.err(format!("unknown type '{}'", name)),
        }
    }

    pub(super) fn resolve_fields(&self, fields: &[Field]) -> CheckResult<Vec<(String, Type)>> {
        fields
            .iter()
            .map(|f| Ok((f.name.clone(), self.resolve_type(&f.ty)?)))
            .collect()
    }

    pub(super) fn resolve_type_def(&self, name: &str, definition: &TypeDef) -> CheckResult<TypeKind> {
        match definition {
            TypeDef::Sum(variants) => {
                let mut defs = Vec::new();
                for variant in variants {
                    defs.push(VariantDef {
                        name: variant.name.clone(),
                        fields: self.resolve_fields(&variant.fields)?,
                    });
                }
                Ok(TypeKind::Sum(defs))
            }
            TypeDef::Shape(fields) => Ok(TypeKind::Shape(self.resolve_fields(fields)?)),
            TypeDef::Index { key, value } => {
                let key = self.resolve_type(key)?;
                if key != Type::String {
                    return self.err(format!(
                        "index signature keys must be string, found {}",
                        self.show(&key)
                    ));
                }
                Ok(TypeKind::Dict {
                    key,
                    value: self.resolve_type(value)?,
                })
            }
            TypeDef::Alias(expr) => {
                // Only reached inside an intersection.
                let ty = self.resolve_type(expr)?;
                match self.table.fields_of(&ty) {
                    Some(fields) => Ok(TypeKind::Shape(fields)),
                    None => self.err(format!(
                        "'{}' cannot be part of the intersection '{}': only types with fields can be merged",
                        self.show(&ty),
                        name
                    )),
                }
            }
            TypeDef::Intersection(parts) => {
                let mut merged: Vec<(String, Type)> = Vec::new();
                for part in parts {
                    let fields = match self.resolve_type_def(name, part)? {
                        TypeKind::Shape(fields) => fields,
                        _ => {
                            return self.err(format!(
                                "only types with fields can be merged with '&' (in '{}')",
                                name
                            ))
                        }
                    };
                    for (field, ty) in fields {
                        match merged.iter().find(|(n, _)| *n == field) {
                            Some((_, existing)) if *existing != ty => {
                                return self.err(format!(
                                    "field '{}' has conflicting types in '{}': {} and {}",
                                    field,
                                    name,
                                    self.show(existing),
                                    self.show(&ty)
                                ));
                            }
                            Some(_) => {}
                            None => merged.push((field, ty)),
                        }
                    }
                }
                Ok(TypeKind::Shape(merged))
            }
        }
    }

    pub(super) fn function_signature(
        &self,
        name: &str,
        params: &[Param],
        return_type: Option<&TypeExpr>,
        is_declaration: bool,
    ) -> CheckResult<Type> {
        let mut param_types = Vec::new();
        for param in params {
            match &param.ty {
                Some(ty) => param_types.push(self.resolve_type(ty)?),
                None => {
                    return self.err(format!(
                        "parameter '{}' of '{}' needs a type annotation",
                        param.name, name
                    ))
                }
            }
        }
        let ret = match return_type {
            Some(ty) => self.resolve_type(ty)?,
            None if is_declaration => Type::Void,
            None => Type::Unknown,
        };
        Ok(Type::Function {
            params: param_types,
            ret: Box::new(ret),
        })
    }

    pub(super) fn expect_type(&self, expected: &Type, actual: &Type, what: &str) -> CheckResult<()> {
        if self.assignable(expected, actual) {
            Ok(())
        } else {
            self.err(format!(
                "{} must be {}, found {}",
                what,
                self.show(expected),
                self.show(actual)
            ))
        }
    }

    /// Type of an `if`/`match` with several branches.
    pub(super) fn branch_types(&self, types: &[Type], expected: Option<&Type>, what: &str) -> CheckResult<Type> {
        if let Some(expected) = expected {
            for ty in types {
                if *ty != Type::Void && !self.assignable(expected, ty) {
                    return self.err(format!(
                        "{} branch has type {}, expected {}",
                        what,
                        self.show(ty),
                        self.show(expected)
                    ));
                }
            }
        }
        let mut result = Type::Never;
        for ty in types {
            result = match self.join(&result, ty) {
                Some(joined) => joined,
                // Branches with different types: fine as a statement, an error if used as a value.
                None => return Ok(Type::Void),
            };
        }
        Ok(result)
    }
}
