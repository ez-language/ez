//! Modules: checking a file, what it exports and imports.

use super::*;

impl Checker {
    pub(super) fn check_module(&mut self, path: &Path, display: &str, source: &str) -> CheckResult<ModuleId> {
        let program = parse_source(source).map_err(|e| prefix_error(display, e))?;
        let program = Rc::new(program);
        self.asts.insert(path.to_path_buf(), Rc::clone(&program));
        self.loading.push(path.to_path_buf());

        let dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let prelude = self.prelude();
        let saved = std::mem::replace(
            &mut self.state,
            ModuleState {
                is_entry: self.loading.len() == 1,
                scopes: vec![prelude, Scope::default()],
                function_scopes: Vec::new(),
                returns: Vec::new(),
                file: display.to_string(),
                path: path.to_path_buf(),
                dir,
                line: 0,
            },
        );

        let result = self.check_statements(&program, None).and_then(|_| {
            let module_scope = self.state.scopes.get(1).cloned().unwrap_or_default();
            self.module_exports(&program, &module_scope, display)
        });
        self.state = saved;
        self.loading.pop();
        let module = result?;

        self.modules.push(module);
        let id = self.modules.len() - 1;
        self.module_by_path.insert(path.to_path_buf(), id);
        Ok(id)
    }

    /// What other modules can import: only the declarations marked with `export`. Exporting a
    /// sum type also exports its variants; an exported `var` is read-only outside the module.
    pub(super) fn module_exports(&mut self, program: &[AstNode], scope: &Scope, display: &str) -> CheckResult<ModuleType> {
        let mut module = ModuleType {
            display: display.to_string(),
            ..ModuleType::default()
        };
        let mut exported_ids: HashSet<TypeId> = HashSet::new();
        let mut private_ids: HashSet<TypeId> = HashSet::new();
        for statement in program {
            let name = match statement.declared_name() {
                Some(name) => name,
                None => continue,
            };
            let is_type = matches!(
                statement.unlocated(),
                AstNode::TypeDecl { .. }
            );
            if !statement.is_exported() {
                module.private.insert(name.to_string());
                if let Some(TypeRef::Type(Type::Named { id, .. })) = scope.types.get(name).filter(|_| is_type) {
                    private_ids.insert(*id);
                }
                continue;
            }
            if is_type {
                if let Some(type_ref) = scope.types.get(name) {
                    module.types.insert(name.to_string(), type_ref.clone());
                    if let TypeRef::Type(Type::Named { id, .. }) = type_ref {
                        exported_ids.insert(*id);
                        for variant in self.table.variants_of(*id).cloned().unwrap_or_default() {
                            if let Some(symbol) = scope.vars.get(&variant.name) {
                                module.members.insert(variant.name.clone(), symbol.clone());
                            }
                        }
                    }
                }
            } else if let Some(symbol) = scope.vars.get(name) {
                let mut symbol = symbol.clone();
                symbol.owner = Some(display.to_string());
                module.members.insert(name.to_string(), symbol);
            }
        }
        private_ids.retain(|id| !exported_ids.contains(id));
        // Implicit declarations (`x = 1`) and imported names are never exported either.
        for name in scope.vars.keys().chain(scope.types.keys()) {
            if !module.members.contains_key(name) && !module.types.contains_key(name) {
                module.private.insert(name.clone());
            }
        }

        // A public signature cannot mention a private type: importers could not name it.
        for statement in program.iter().filter(|s| s.is_exported()) {
            self.set_line(statement);
            let name = statement.declared_name().unwrap_or_default();
            let mut types: Vec<Type> = Vec::new();
            match scope.types.get(name) {
                Some(TypeRef::Type(Type::Named { id, args })) if module.types.contains_key(name) => {
                    match &self.table.get(*id).kind {
                        TypeKind::Sum(variants) => types.extend(
                            variants.iter().flat_map(|v| v.fields.iter().map(|(_, t)| t.clone())),
                        ),
                        TypeKind::Shape(fields) => {
                            types.extend(fields.iter().map(|(_, t)| t.clone()))
                        }
                        TypeKind::Dict { value, .. } => types.push(value.clone()),
                        _ => {}
                    }
                    types.extend(args.iter().cloned());
                }
                Some(TypeRef::Type(alias)) if module.types.contains_key(name) => types.push(alias.clone()),
                _ => {}
            }
            if let Some(symbol) = module.members.get(name) {
                types.push(symbol.ty.clone());
            }
            for ty in &types {
                if let Some(id) = Self::find_type_id(ty, &private_ids) {
                    return self.err(format!(
                        "'{}' is exported, but its type uses the private type '{}' (export '{}' too)",
                        name,
                        self.table.get(id).name,
                        self.table.get(id).name
                    ));
                }
            }
        }
        Ok(module)
    }

    pub(super) fn missing_member<T>(&self, module: ModuleId, name: &str) -> CheckResult<T> {
        let module = &self.modules[module];
        if module.private.contains(name) {
            self.err(format!("'{}' is not exported by '{}'", name, module.display))
        } else {
            self.err(format!("module '{}' has no member '{}'", module.display, name))
        }
    }

    /// First type of `ids` that appears in `ty`.
    pub(super) fn find_type_id(ty: &Type, ids: &HashSet<TypeId>) -> Option<TypeId> {
        match ty {
            Type::Named { id, args } | Type::Variant { id, args, .. } => {
                if ids.contains(id) {
                    return Some(*id);
                }
                args.iter().find_map(|a| Self::find_type_id(a, ids))
            }
            Type::List(inner) => Self::find_type_id(inner, ids),
            Type::Function { params, ret } => params
                .iter()
                .chain(std::iter::once(&**ret))
                .find_map(|t| Self::find_type_id(t, ids)),
            Type::Record(fields) => fields.iter().find_map(|(_, t)| Self::find_type_id(t, ids)),
            _ => None,
        }
    }

    pub(super) fn check_import(&mut self, kind: &ImportKind, path: &str) -> CheckResult<()> {
        let module_id = if is_stdlib_import(path) {
            self.stdlib_module(path)
        } else {
            let resolved = resolve_module_path(&self.state.dir, path);
            match self.module_by_path.get(&resolved) {
                Some(id) => *id,
                None => {
                    if self.loading.contains(&resolved) {
                        return self.err(format!("circular import of '{}'", path));
                    }
                    let source = match std::fs::read_to_string(&resolved) {
                        Ok(source) => source,
                        Err(e) => return self.err(format!("cannot import '{}': {}", path, e)),
                    };
                    let display = path.to_string();
                    self.check_module(&resolved, &display, &source)?
                }
            }
        };

        match kind {
            ImportKind::Module(name) => {
                self.declare(name, Symbol::value(Type::Module(module_id), true))?;
            }
            ImportKind::Names(names) => {
                let module = self.modules[module_id].clone();
                for name in names {
                    let mut found = false;
                    if let Some(symbol) = module.members.get(name) {
                        self.declare(name, symbol.clone())?;
                        found = true;
                    }
                    if let Some(type_ref) = module.types.get(name) {
                        self.current_scope().types.insert(name.clone(), type_ref.clone());
                        // Importing a sum type also imports its constructors.
                        if let TypeRef::Type(Type::Named { id, .. }) = type_ref {
                            if self.table.variants_of(*id).is_some() {
                                let mut scope = std::mem::take(self.current_scope());
                                self.declare_constructors_into(&mut scope, *id);
                                *self.current_scope() = scope;
                            }
                        }
                        found = true;
                    }
                    if !found {
                        return self.missing_member(module_id, name);
                    }
                }
            }
        }
        Ok(())
    }
}
