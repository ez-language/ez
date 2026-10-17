use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::runtime::value::Value;

/// A scope of variables. Blocks and function calls create child scopes.
#[derive(Debug, Default)]
pub struct Env {
    vars: RefCell<HashMap<String, Value>>,
    /// Names imported from another module: always read from that module, so the
    /// importer sees the current value of an exported `var`.
    aliases: RefCell<HashMap<String, Rc<Env>>>,
    parent: Option<Rc<Env>>,
}

impl Env {
    pub fn new(parent: Option<Rc<Env>>) -> Rc<Env> {
        Rc::new(Env {
            vars: RefCell::new(HashMap::new()),
            aliases: RefCell::new(HashMap::new()),
            parent,
        })
    }

    pub fn child(parent: &Rc<Env>) -> Rc<Env> {
        Env::new(Some(Rc::clone(parent)))
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(value) = self.vars.borrow().get(name) {
            return Some(value.clone());
        }
        if let Some(module) = self.aliases.borrow().get(name) {
            return module.get_local(name);
        }
        self.parent.as_ref().and_then(|parent| parent.get(name))
    }

    /// Makes `name` refer to the variable of the same name in `module`.
    pub fn alias(&self, name: &str, module: &Rc<Env>) {
        self.aliases
            .borrow_mut()
            .insert(name.to_string(), Rc::clone(module));
    }

    /// Value defined in this scope only (used for module members).
    pub fn get_local(&self, name: &str) -> Option<Value> {
        self.vars.borrow().get(name).cloned()
    }

    pub fn define(&self, name: &str, value: Value) {
        self.vars.borrow_mut().insert(name.to_string(), value);
    }

    /// Updates an existing variable in this scope or an enclosing one; `false` if not found.
    pub fn assign(&self, name: &str, value: Value) -> bool {
        if let Some(slot) = self.vars.borrow_mut().get_mut(name) {
            *slot = value;
            return true;
        }
        match &self.parent {
            Some(parent) => parent.assign(name, value),
            None => false,
        }
    }
}
