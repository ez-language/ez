//! Built-in types, functions and methods, and the standard library modules.

use super::*;

impl Checker {
    pub fn new() -> Self {
        let mut table = TypeTable::default();

        // Built-in sum types: declared first, resolved once every name exists (`Json` refers to
        // `JsonObject`, which refers back to `Json`).
        let mut ids: HashMap<&'static str, TypeId> = HashMap::new();
        for sum in BUILTIN_SUMS {
            ids.insert(sum.name, table.add(sum.name, sum.params, TypeKind::Pending));
        }
        let json = ids["Json"];
        let json_object = table.add(
            "JsonObject",
            0,
            TypeKind::Dict {
                key: Type::String,
                value: Self::named(json),
            },
        );
        let field_type = |name: &str| -> Type {
            match name {
                "T0" => Type::Param(0),
                "T1" => Type::Param(1),
                "string" => Type::String,
                "int" => Type::Int,
                "float" => Type::Float,
                "boolean" => Type::Boolean,
                "Json[]" => Type::List(Box::new(Self::named(json))),
                "JsonObject" => Self::named(json_object),
                other => panic!("unknown field type '{}' in a built-in sum type", other),
            }
        };
        for sum in BUILTIN_SUMS {
            let variants = sum
                .variants
                .iter()
                .map(|(name, fields)| VariantDef {
                    name: name.to_string(),
                    fields: fields
                        .iter()
                        .map(|(field, ty)| (field.to_string(), field_type(ty)))
                        .collect(),
                })
                .collect();
            table.types[ids[sum.name]].kind = TypeKind::Sum(variants);
        }

        let result_of = |ok: Type, error: &str| Type::Named {
            id: ids["Result"],
            args: vec![ok, Self::named(ids[error])],
        };
        let file = table.add(
            "File",
            0,
            TypeKind::Builtin {
                fields: vec![],
                methods: vec![
                    ("read".to_string(), vec![], result_of(Type::String, "FileError")),
                    (
                        "write".to_string(),
                        vec![Type::String],
                        result_of(Type::Void, "FileError"),
                    ),
                    ("close".to_string(), vec![], Type::Void),
                ],
            },
        );
        let int_fields = |names: &[&str]| -> Vec<(String, Type)> {
            names.iter().map(|n| (n.to_string(), Type::Int)).collect()
        };
        let datetime = table.add(
            "DateTime",
            0,
            TypeKind::Builtin {
                fields: int_fields(&["year", "month", "day", "hour", "minute", "second"]),
                methods: vec![],
            },
        );
        let response = table.add(
            "Response",
            0,
            TypeKind::Builtin {
                fields: vec![
                    ("status".to_string(), Type::Int),
                    ("body".to_string(), Type::String),
                ],
                methods: vec![],
            },
        );
        let regex_match = table.add(
            "Match",
            0,
            TypeKind::Builtin {
                fields: vec![
                    ("value".to_string(), Type::String),
                    ("start".to_string(), Type::Int),
                    ("end".to_string(), Type::Int),
                ],
                methods: vec![],
            },
        );
        let s = || Type::String;
        let match_ty = Self::named(regex_match);
        let regex = table.add(
            "Regex",
            0,
            TypeKind::Builtin {
                fields: vec![],
                methods: vec![
                    ("test".to_string(), vec![s()], Type::Boolean),
                    (
                        "find".to_string(),
                        vec![s()],
                        Type::Named {
                            id: ids["Option"],
                            args: vec![match_ty.clone()],
                        },
                    ),
                    ("findAll".to_string(), vec![s()], Type::List(Box::new(match_ty))),
                    ("replace".to_string(), vec![s(), s()], s()),
                    ("replaceAll".to_string(), vec![s(), s()], s()),
                    ("split".to_string(), vec![s()], Type::List(Box::new(s()))),
                ],
            },
        );

        Checker {
            table,
            builtins: BuiltinTypes {
                option: ids["Option"],
                result: ids["Result"],
                parse_error: ids["ParseError"],
                math_error: ids["MathError"],
                file_error: ids["FileError"],
                file_mode: ids["Mode"],
                http_error: ids["HttpError"],
                json,
                json_object,
                json_error: ids["JsonError"],
                date_error: ids["DateError"],
                file,
                datetime,
                response,
                regex_match,
                regex,
                regex_error: ids["RegexError"],
            },
            modules: Vec::new(),
            module_by_path: HashMap::new(),
            stdlib: HashMap::new(),
            loading: Vec::new(),
            asts: HashMap::new(),
            methods: HashMap::new(),
            last_arg_types: Vec::new(),
            state: ModuleState::default(),
        }
    }

    /// Id of a built-in sum type, by name.
    pub(super) fn builtin_sum_id(&self, name: &str) -> TypeId {
        let b = &self.builtins;
        match name {
            "Option" => b.option,
            "Result" => b.result,
            "ParseError" => b.parse_error,
            "MathError" => b.math_error,
            "FileError" => b.file_error,
            "Mode" => b.file_mode,
            "HttpError" => b.http_error,
            "Json" => b.json,
            "JsonError" => b.json_error,
            "DateError" => b.date_error,
            "RegexError" => b.regex_error,
            other => panic!("unknown built-in sum type '{}'", other),
        }
    }

    /// Scope with the built-in functions, types and constructors.
    pub(super) fn prelude(&self) -> Scope {
        let mut scope = Scope::default();
        for name in ["print", "input", "range", "int", "float", "string", "panic"] {
            scope.vars.insert(name.to_string(), Symbol::builtin(name));
        }
        let primitives = [
            ("int", Type::Int),
            ("float", Type::Float),
            ("string", Type::String),
            ("boolean", Type::Boolean),
            ("bool", Type::Boolean),
            ("void", Type::Void),
        ];
        for (name, ty) in primitives {
            scope.types.insert(name.to_string(), TypeRef::Type(ty));
        }
        for sum in BUILTIN_SUMS.iter().filter(|sum| sum.module.is_none()) {
            let id = self.builtin_sum_id(sum.name);
            let type_ref = if sum.params > 0 {
                TypeRef::Generic(id)
            } else {
                TypeRef::Type(Self::named(id))
            };
            scope.types.insert(sum.name.to_string(), type_ref);
            self.declare_constructors_into(&mut scope, id);
        }
        scope
    }

    pub(super) fn declare_constructors_into(&self, scope: &mut Scope, id: TypeId) {
        let info = self.table.get(id);
        let params = info.params;
        if let TypeKind::Sum(variants) = &info.kind {
            for variant in variants {
                let result = Type::Named {
                    id,
                    args: vec![Type::Unknown; params],
                };
                let ty = if variant.fields.is_empty() {
                    result
                } else {
                    Type::Function {
                        params: variant.fields.iter().map(|(_, t)| t.clone()).collect(),
                        ret: Box::new(result),
                    }
                };
                scope.vars.insert(
                    variant.name.clone(),
                    Symbol {
                        ty,
                        is_const: true,
                        is_imut: true,
                        kind: SymbolKind::Constructor {
                            id,
                            variant: variant.name.clone(),
                        },
                        owner: None,
                        mut_params: Vec::new(),
                        reason: None,
                    },
                );
            }
        }
    }

    pub(super) fn stdlib_module(&mut self, name: &str) -> ModuleId {
        if let Some(id) = self.stdlib.get(name) {
            return *id;
        }
        let mut module = ModuleType {
            display: name.to_string(),
            ..ModuleType::default()
        };
        for function in stdlib_functions(name) {
            module
                .members
                .insert(function.to_string(), Symbol::builtin(&format!("{}.{}", name, function)));
        }
        let b = self.builtins;
        if name == "math" {
            module.members.insert("pi".to_string(), Symbol::value(Type::Float, true));
            module.members.insert("e".to_string(), Symbol::value(Type::Float, true));
        }
        let object_types: &[(&str, TypeId)] = match name {
            "file" => &[("File", b.file)],
            "datetime" => &[("DateTime", b.datetime)],
            "http" => &[("Response", b.response)],
            "json" => &[("JsonObject", b.json_object)],
            "regex" => &[("Match", b.regex_match), ("Regex", b.regex)],
            _ => &[],
        };
        for (type_name, id) in object_types {
            module
                .types
                .insert(type_name.to_string(), TypeRef::Type(Self::named(*id)));
        }
        // Sum types of the module (`FileError`, `Json`...), with their constructors.
        let mut constructors = Scope::default();
        for sum in BUILTIN_SUMS.iter().filter(|sum| sum.module == Some(name)) {
            let id = self.builtin_sum_id(sum.name);
            module
                .types
                .insert(sum.name.to_string(), TypeRef::Type(Self::named(id)));
            self.declare_constructors_into(&mut constructors, id);
        }
        module.members.extend(constructors.vars);
        self.modules.push(module);
        let id = self.modules.len() - 1;
        self.stdlib.insert(name.to_string(), id);
        id
    }

    pub(super) fn arg_types(&mut self, args: &[AstNode]) -> CheckResult<Vec<Type>> {
        args.iter().map(|a| self.check_value(a, None)).collect()
    }

    pub(super) fn arity(&self, name: &str, args: &[Type], min: usize, max: usize) -> CheckResult<()> {
        if args.len() < min || args.len() > max {
            let expected = if min == max {
                min.to_string()
            } else {
                format!("{} to {}", min, max)
            };
            return self.err(format!(
                "'{}' expects {} argument(s), found {}",
                name,
                expected,
                args.len()
            ));
        }
        Ok(())
    }

    pub(super) fn expect_arg(&self, name: &str, index: usize, expected: &Type, actual: &Type) -> CheckResult<()> {
        if self.assignable(expected, actual) {
            Ok(())
        } else {
            self.err(format!(
                "argument {} of '{}' must be {}, found {}",
                index + 1,
                name,
                self.show(expected),
                self.show(actual)
            ))
        }
    }

    pub(super) fn expect_numeric_arg(&self, name: &str, index: usize, actual: &Type) -> CheckResult<()> {
        if actual.is_numeric() || actual.is_dynamic() {
            Ok(())
        } else {
            self.err(format!(
                "argument {} of '{}' must be a number, found {}",
                index + 1,
                name,
                self.show(actual)
            ))
        }
    }

    /// Checks a call to a built-in function and returns its result type.
    pub(super) fn check_builtin_call(&mut self, name: &str, args: &[AstNode]) -> CheckResult<Type> {
        let types = self.arg_types(args)?;
        self.last_arg_types = types.clone();
        let b = self.builtins;
        let s = Type::String;
        let check = |checker: &Self, expected: &[Type]| -> CheckResult<()> {
            checker.arity(name, &types, expected.len(), expected.len())?;
            for (i, (e, a)) in expected.iter().zip(&types).enumerate() {
                checker.expect_arg(name, i, e, a)?;
            }
            Ok(())
        };
        let result_with = |ok: Type, error: TypeId| Type::Named {
            id: b.result,
            args: vec![ok, Self::named(error)],
        };
        let json = Self::named(b.json);
        match name {
            "print" => Ok(Type::Void),
            "panic" => {
                check(self, &[s])?;
                Ok(Type::Never)
            }
            "input" => {
                self.arity(name, &types, 0, 1)?;
                if let Some(prompt) = types.first() {
                    self.expect_arg(name, 0, &s, prompt)?;
                }
                Ok(self.option_of(Type::String))
            }
            "range" => {
                self.arity(name, &types, 1, 3)?;
                for (i, ty) in types.iter().enumerate() {
                    self.expect_arg(name, i, &Type::Int, ty)?;
                }
                Ok(Type::List(Box::new(Type::Int)))
            }
            "int" | "float" => {
                self.arity(name, &types, 1, 1)?;
                let target = if name == "int" { Type::Int } else { Type::Float };
                match &types[0] {
                    Type::Int | Type::Float | Type::Boolean => Ok(target),
                    Type::String => Ok(result_with(target, b.parse_error)),
                    Type::Unknown => Ok(Type::Unknown),
                    other => self.err(format!("cannot convert {} to {}", self.show(other), name)),
                }
            }
            "string" => {
                self.arity(name, &types, 1, 1)?;
                Ok(Type::String)
            }

            "time.sleep" => {
                self.arity(name, &types, 1, 1)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                Ok(Type::Void)
            }
            "time.now" | "time.monotonic" => {
                check(self, &[])?;
                Ok(Type::Float)
            }

            "file.open" => {
                check(self, &[s, Self::named(b.file_mode)])?;
                Ok(result_with(Self::named(b.file), b.file_error))
            }

            "datetime.now" => {
                check(self, &[])?;
                Ok(Self::named(b.datetime))
            }
            "datetime.today" => {
                check(self, &[])?;
                Ok(Type::String)
            }
            "datetime.format" => {
                self.arity(name, &types, 2, 2)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                self.expect_arg(name, 1, &s, &types[1])?;
                Ok(Type::String)
            }
            "datetime.parse" => {
                check(self, &[s.clone(), s])?;
                Ok(result_with(Type::Int, b.date_error))
            }

            "math.abs" => {
                self.arity(name, &types, 1, 1)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                Ok(types[0].clone())
            }
            "math.sqrt" => {
                self.arity(name, &types, 1, 1)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                Ok(result_with(Type::Float, b.math_error))
            }
            "math.pow" => {
                self.arity(name, &types, 2, 2)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                self.expect_numeric_arg(name, 1, &types[1])?;
                Ok(result_with(Type::Float, b.math_error))
            }
            "math.floor" | "math.ceil" | "math.round" => {
                self.arity(name, &types, 1, 1)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                Ok(Type::Int)
            }

            "http.get" | "http.delete" => {
                check(self, &[s])?;
                Ok(result_with(Self::named(b.response), b.http_error))
            }
            "http.post" | "http.put" => {
                check(self, &[s.clone(), s])?;
                Ok(result_with(Self::named(b.response), b.http_error))
            }

            "json.parse" => {
                check(self, &[s])?;
                Ok(result_with(json, b.json_error))
            }
            "json.stringify" => {
                check(self, &[json])?;
                Ok(Type::String)
            }
            "json.get" => {
                check(self, &[json.clone(), s])?;
                Ok(self.option_of(json))
            }
            "json.asString" | "json.asInt" | "json.asFloat" | "json.asBool" => {
                check(self, &[json])?;
                let value = match name {
                    "json.asString" => Type::String,
                    "json.asInt" => Type::Int,
                    "json.asFloat" => Type::Float,
                    _ => Type::Boolean,
                };
                Ok(self.option_of(value))
            }

            "regex.compile" => {
                check(self, &[s])?;
                Ok(result_with(Self::named(b.regex), b.regex_error))
            }
            other => self.err(format!("unknown built-in function '{}'", other)),
        }
    }

    pub(super) fn check_method_call(
        &mut self,
        object_ty: &Type,
        name: &str,
        args: &[AstNode],
        expected: Option<&Type>,
    ) -> CheckResult<Type> {
        if let Type::Module(id) = object_ty {
            let symbol = match self.modules[*id].members.get(name) {
                Some(symbol) => symbol.clone(),
                None => return self.missing_member(*id, name),
            };
            return match symbol.kind {
                SymbolKind::Builtin(builtin) => self.check_builtin_call(&builtin, args),
                SymbolKind::Constructor { id, variant } => {
                    self.check_constructor(id, &variant, args, expected)
                }
                SymbolKind::Value => self.check_function_call_mut(&symbol.ty, &symbol.mut_params, args),
            };
        }

        // `result.mapErr(f)`: converts the error with `f` (a function or a variant constructor).
        if let Type::Named { id, args: type_args } = object_ty {
            if *id == self.builtins.result && name == "mapErr" {
                return self.check_map_err(type_args, args);
            }
        }
        let s = Type::String;
        let signature: Option<(Vec<Type>, Type)> = match object_ty {
            Type::String => match name {
                "length" => Some((vec![], Type::Int)),
                "trim" | "upper" | "lower" => Some((vec![], Type::String)),
                "isSpace" | "isNumeric" | "isAlpha" | "isAlphaNumeric" | "isUpper" | "isLower"
                | "isTitle" => Some((vec![], Type::Boolean)),
                "split" => Some((vec![s.clone()], Type::List(Box::new(Type::String)))),
                "replace" => Some((vec![s.clone(), s], Type::String)),
                _ => None,
            },
            Type::List(_) => match name {
                "length" => Some((vec![], Type::Int)),
                _ => None,
            },
            Type::Unknown => {
                for arg in args {
                    self.check_value(arg, None)?;
                }
                return Ok(Type::Unknown);
            }
            Type::Named { id, .. } => match &self.table.get(*id).kind {
                TypeKind::Dict { .. } if name == "length" => Some((vec![], Type::Int)),
                TypeKind::Builtin { methods, .. } => methods
                    .iter()
                    .find(|(n, _, _)| n == name)
                    .map(|(_, params, ret)| (params.clone(), ret.clone())),
                _ => None,
            },
            _ => None,
        };
        if let Some((params, ret)) = signature {
            self.check_args(&params, args)?;
            return Ok(ret);
        }

        // A field holding a function: `obj.callback(1)`.
        if let Some(fields) = self.table.fields_of(object_ty) {
            if let Some((_, field_ty)) = fields.into_iter().find(|(n, _)| n == name) {
                return self.check_function_call(&field_ty, args);
            }
        }
        // Every value has `display()` (what `print` shows: readable) and `debug()` (its
        // structure).
        if name == "display" || name == "debug" {
            self.check_args(&[], args)?;
            return Ok(Type::String);
        }
        self.err(format!(
            "a value of type {} has no method '{}'",
            self.show(object_ty),
            name
        ))
    }

    /// `result.mapErr(f)` with `f: (E) => F` gives a `Result[T, F]`.
    pub(super) fn check_map_err(&mut self, type_args: &[Type], args: &[AstNode]) -> CheckResult<Type> {
        let ok = type_args.first().cloned().unwrap_or(Type::Unknown);
        let error = type_args.get(1).cloned().unwrap_or(Type::Unknown);
        if args.len() != 1 {
            return self.err(format!("'mapErr' expects 1 argument, found {}", args.len()));
        }
        let converter = self.check_value(&args[0], None)?;
        match converter {
            Type::Function { params, ret } if params.len() == 1 => {
                if !self.assignable(&params[0], &error) {
                    return self.err(format!(
                        "the function given to 'mapErr' must accept {}, but it accepts {}",
                        self.show(&error),
                        self.show(&params[0])
                    ));
                }
                Ok(Type::Named {
                    id: self.builtins.result,
                    args: vec![ok, *ret],
                })
            }
            other => self.err(format!(
                "'mapErr' expects a function with one parameter (or a variant with one field), found {}",
                self.show(&other)
            )),
        }
    }
}
