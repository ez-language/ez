//! Modules: loading user modules, imports and the standard library modules.

use super::*;

impl Interpreter {
    pub(super) fn load_module(&mut self, path: &Path) -> Result<Rc<ModuleValue>, EzError> {
        if let Some(module) = self.modules.get(path) {
            return Ok(Rc::clone(module));
        }
        let program = match self.program.modules.get(path) {
            Some(program) => Rc::clone(program),
            None => {
                return Err(EzError::ImportError(format!(
                    "module '{}' was not checked",
                    path.display()
                )))
            }
        };
        let env = Env::module(&self.prelude);
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let module = Rc::new(ModuleValue {
            name,
            env: Rc::clone(&env),
            types: RefCell::new(Vec::new()),
        });

        let display: Rc<str> = if *path == self.program.entry {
            Rc::from(
                std::env::current_dir()
                    .ok()
                    .and_then(|cwd| path.strip_prefix(cwd).ok().map(|p| p.display().to_string()))
                    .unwrap_or_else(|| path.display().to_string()),
            )
        } else {
            Rc::from(path.display().to_string())
        };
        let saved = (
            std::mem::replace(&mut self.file, display),
            std::mem::replace(
                &mut self.dir,
                path.parent().map(Path::to_path_buf).unwrap_or_default(),
            ),
            self.line,
            self.current_module.replace(Rc::clone(&module)),
            std::mem::replace(&mut self.module_path, path.to_path_buf()),
        );

        let result = self.exec_statements(&program, &env);

        self.file = saved.0;
        self.dir = saved.1;
        self.line = saved.2;
        self.current_module = saved.3;
        self.module_path = saved.4;

        match result {
            Ok(_) => {}
            Err(Flow::Error(error)) => return Err(error),
            Err(_) => {}
        }
        self.modules.insert(path.to_path_buf(), Rc::clone(&module));
        Ok(module)
    }

    pub(super) fn import(&mut self, kind: &ImportKind, path: &str, env: &Rc<Env>) -> Exec<()> {
        let module = if is_stdlib_import(path) {
            self.stdlib_module(path)
        } else {
            let resolved = resolve_module_path(&self.dir, path);
            self.load_module(&resolved)?
        };
        match kind {
            ImportKind::Module(name) => env.define(name, Value::Module(module)),
            ImportKind::Names(names) => {
                for name in names {
                    // Imported names always read the module's variable (see `Env::alias`).
                    if module.env.get_local(name).is_some() {
                        env.alias(name, &module.env);
                    }
                    // Importing a sum type also imports its variants. Shapes and aliases have
                    // no runtime value.
                    let types = module.types.borrow();
                    if let Some(sum) = types.iter().find(|sum| sum.name == *name) {
                        for (variant, _) in &sum.variants {
                            env.alias(variant, &module.env);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn stdlib_module(&mut self, name: &str) -> Rc<ModuleValue> {
        if let Some(module) = self.stdlib.get(name) {
            return Rc::clone(module);
        }
        let env = Env::new(None);
        for (member, value) in stdlib::module_members(name) {
            env.define(&member, value);
        }
        let types = BUILTIN_SUMS
            .iter()
            .filter(|sum| sum.module == Some(name))
            .map(|sum| stdlib::sum(sum.name))
            .collect();
        let module = Rc::new(ModuleValue {
            name: name.to_string(),
            env,
            types: RefCell::new(types),
        });
        self.stdlib.insert(name.to_string(), Rc::clone(&module));
        module
    }
}
