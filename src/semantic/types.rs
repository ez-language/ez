use std::fmt;

pub type TypeId = usize;
pub type ModuleId = usize;

/// Static type of an expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    String,
    Boolean,
    Void,
    /// Dynamic value (e.g. the result of `json.parse`): operations on it are checked at runtime.
    Any,
    /// Not known yet (`None`, `[]`): compatible with anything and refined on later assignments.
    Unknown,
    /// Expression that never produces a value (`return`, `break`, `continue`).
    Never,
    List(Box<Type>),
    Function {
        params: Vec<Type>,
        ret: Box<Type>,
    },
    /// Declared type (sum type, shape, interface, index signature or built-in object).
    Named {
        id: TypeId,
        args: Vec<Type>,
    },
    /// Value of a sum type known to be one specific variant (inside `if (x is Some) { ... }`).
    Variant {
        id: TypeId,
        args: Vec<Type>,
        variant: String,
    },
    /// Object literal without a declared type.
    Record(Vec<(String, Type)>),
    Module(ModuleId),
    /// Generic parameter of a built-in type (`T` in `Option[T]`).
    Param(usize),
}

impl Type {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Float)
    }

    pub fn is_dynamic(&self) -> bool {
        matches!(self, Type::Any | Type::Unknown)
    }

    pub fn contains_unknown(&self) -> bool {
        match self {
            Type::Unknown => true,
            Type::List(inner) => inner.contains_unknown(),
            Type::Function { params, ret } => {
                params.iter().any(Type::contains_unknown) || ret.contains_unknown()
            }
            Type::Named { args, .. } | Type::Variant { args, .. } => {
                args.iter().any(Type::contains_unknown)
            }
            Type::Record(fields) => fields.iter().any(|(_, t)| t.contains_unknown()),
            _ => false,
        }
    }

    /// Replaces the generic parameters with `args`.
    pub fn substitute(&self, args: &[Type]) -> Type {
        match self {
            Type::Param(i) => args.get(*i).cloned().unwrap_or(Type::Unknown),
            Type::List(inner) => Type::List(Box::new(inner.substitute(args))),
            Type::Function { params, ret } => Type::Function {
                params: params.iter().map(|p| p.substitute(args)).collect(),
                ret: Box::new(ret.substitute(args)),
            },
            Type::Named { id, args: inner } => Type::Named {
                id: *id,
                args: inner.iter().map(|a| a.substitute(args)).collect(),
            },
            Type::Variant {
                id,
                args: inner,
                variant,
            } => Type::Variant {
                id: *id,
                args: inner.iter().map(|a| a.substitute(args)).collect(),
                variant: variant.clone(),
            },
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|(n, t)| (n.clone(), t.substitute(args)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VariantDef {
    pub name: String,
    pub fields: Vec<(String, Type)>,
}

#[derive(Debug, Clone)]
pub enum TypeKind {
    /// Declared but not resolved yet (between hoisting and resolution).
    Pending,
    Sum(Vec<VariantDef>),
    /// Nominal type with fixed fields.
    Shape(Vec<(String, Type)>),
    /// Structural type: any value with exactly the same fields is accepted.
    Interface(Vec<(String, Type)>),
    /// Index signature: `{ [key: string]: T }`.
    Dict { key: Type, value: Type },
    /// Built-in object (`File`, `DateTime`...): read-only fields and methods.
    Builtin {
        fields: Vec<(String, Type)>,
        methods: Vec<(String, Vec<Type>, Type)>,
    },
}

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub name: String,
    /// Number of generic parameters (only built-in types are generic).
    pub params: usize,
    pub kind: TypeKind,
}

/// All declared types of a program (shared by every module).
#[derive(Debug, Default)]
pub struct TypeTable {
    pub types: Vec<TypeInfo>,
}

impl TypeTable {
    pub fn add(&mut self, name: &str, params: usize, kind: TypeKind) -> TypeId {
        self.types.push(TypeInfo {
            name: name.to_string(),
            params,
            kind,
        });
        self.types.len() - 1
    }

    pub fn get(&self, id: TypeId) -> &TypeInfo {
        &self.types[id]
    }

    /// Fields of a value of this type, with generic arguments applied.
    pub fn fields_of(&self, ty: &Type) -> Option<Vec<(String, Type)>> {
        match ty {
            Type::Record(fields) => Some(fields.clone()),
            Type::Named { id, args } => match &self.get(*id).kind {
                TypeKind::Shape(fields)
                | TypeKind::Interface(fields)
                | TypeKind::Builtin { fields, .. } => Some(
                    fields
                        .iter()
                        .map(|(n, t)| (n.clone(), t.substitute(args)))
                        .collect(),
                ),
                _ => None,
            },
            Type::Variant { id, args, variant } => match &self.get(*id).kind {
                TypeKind::Sum(variants) => variants.iter().find(|v| &v.name == variant).map(|v| {
                    v.fields
                        .iter()
                        .map(|(n, t)| (n.clone(), t.substitute(args)))
                        .collect()
                }),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn variants_of(&self, id: TypeId) -> Option<&Vec<VariantDef>> {
        match &self.get(id).kind {
            TypeKind::Sum(variants) => Some(variants),
            _ => None,
        }
    }

    pub fn display(&self, ty: &Type) -> String {
        TypeDisplay { table: self, ty }.to_string()
    }
}

struct TypeDisplay<'a> {
    table: &'a TypeTable,
    ty: &'a Type,
}

impl fmt::Display for TypeDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let show = |ty: &Type| self.table.display(ty);
        match self.ty {
            Type::Int => write!(f, "int"),
            Type::Float => write!(f, "float"),
            Type::String => write!(f, "string"),
            Type::Boolean => write!(f, "boolean"),
            Type::Void => write!(f, "void"),
            Type::Any => write!(f, "any"),
            Type::Unknown => write!(f, "?"),
            Type::Never => write!(f, "never"),
            Type::List(inner) => write!(f, "{}[]", show(inner)),
            Type::Function { params, ret } => {
                let params: Vec<String> = params.iter().map(show).collect();
                write!(f, "({}) => {}", params.join(", "), show(ret))
            }
            Type::Named { id, args } | Type::Variant { id, args, .. } => {
                write!(f, "{}", self.table.get(*id).name)?;
                if !args.is_empty() {
                    let args: Vec<String> = args.iter().map(show).collect();
                    write!(f, "[{}]", args.join(", "))?;
                }
                if let Type::Variant { variant, .. } = self.ty {
                    write!(f, ".{}", variant)?;
                }
                Ok(())
            }
            Type::Record(fields) => {
                let fields: Vec<String> = fields
                    .iter()
                    .map(|(n, t)| format!("{}: {}", n, show(t)))
                    .collect();
                write!(f, "{{ {} }}", fields.join(", "))
            }
            Type::Module(_) => write!(f, "module"),
            Type::Param(i) => write!(f, "T{}", i),
        }
    }
}
