//! Blocks and statements: hoisting, scopes, loops and assignments.

use super::*;

impl Interpreter {
    /// Defines functions and constructors of a block before running it.
    pub(super) fn hoist(&mut self, statements: &[AstNode], env: &Rc<Env>) -> Exec<()> {
        // Imports first, as in the type checker: the whole block can use them.
        for statement in statements {
            if let AstNode::Located { line, .. } = statement {
                self.line = *line;
            }
            if let AstNode::Import { kind, path } = statement.unlocated() {
                self.import(kind, path, env)?;
            }
        }
        for statement in statements {
            match statement.unlocated() {
                AstNode::FunctionDecl {
                    name,
                    owner,
                    params,
                    return_type,
                    body,
                } => {
                    let full = match owner {
                        Some(owner) => format!("{}.{}", owner, name),
                        None => name.clone(),
                    };
                    let closure = Rc::new(Closure {
                        name: full.clone(),
                        params: params.clone(),
                        body: Rc::clone(body),
                        env: Rc::clone(env),
                        float_params: params.iter().map(|p| is_float_type(&p.ty)).collect(),
                        returns_float: is_float_type(return_type),
                        file: Rc::clone(&self.file),
                    });
                    match owner {
                        // Methods are not variables: calls find them by module and type.
                        Some(_) => {
                            self.methods.insert((self.module_path.clone(), full), closure);
                        }
                        None => env.define(name, Value::Function(closure)),
                    }
                }
                AstNode::TypeDecl {
                    name,
                    definition: TypeDef::Sum(variants),
                } => {
                    let sum = Rc::new(SumType {
                        name: name.clone(),
                        variants: variants
                            .iter()
                            .map(|v| {
                                (v.name.clone(), v.fields.iter().map(|f| f.name.clone()).collect())
                            })
                            .collect(),
                    });
                    for variant in variants {
                        env.define(&variant.name, sum.constructor(&variant.name));
                    }
                    if let Some(module) = &self.current_module {
                        if Rc::ptr_eq(&module.env, env) {
                            module.types.borrow_mut().push(sum);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(super) fn exec_statements(&mut self, statements: &[AstNode], env: &Rc<Env>) -> Exec<Value> {
        self.hoist(statements, env)?;
        let mut last = Value::Void;
        for statement in statements {
            last = self.eval(statement, env)?;
        }
        match statements.last().map(AstNode::unlocated) {
            Some(
                AstNode::While { .. }
                | AstNode::For { .. }
                | AstNode::Assign { .. }
                | AstNode::VarDecl { .. }
                | AstNode::FunctionDecl { .. }
                | AstNode::TypeDecl { .. }
                | AstNode::Import { .. },
            ) => Ok(Value::Void),
            _ => Ok(last),
        }
    }

    /// Runs a block (or a single expression, e.g. a match arm) in a new scope.
    pub(super) fn exec_block(&mut self, node: &AstNode, env: &Rc<Env>) -> Exec<Value> {
        let scope = Env::child(env);
        match node.unlocated() {
            AstNode::Block(statements) => self.exec_statements(statements, &scope),
            other => self.eval(other, &scope),
        }
    }

    pub(super) fn exec_loop_body(&mut self, body: &AstNode, env: &Rc<Env>) -> Exec<bool> {
        match self.exec_block(body, env) {
            Ok(_) | Err(Flow::Continue) => Ok(true),
            Err(Flow::Break) => Ok(false),
            Err(other) => Err(other),
        }
    }

    pub(super) fn eval_assign(
        &mut self,
        target: &AstNode,
        op: &AssignOperator,
        value: &AstNode,
        env: &Rc<Env>,
    ) -> Exec<()> {
        let new_value = |interpreter: &mut Self, current: Option<Value>| -> Exec<Value> {
            let value = interpreter.eval(value, env)?;
            match (op.binary(), current) {
                (None, Some(Value::Float(_))) => Ok(coerce(value, true)),
                (None, _) => Ok(value),
                (Some(binary), Some(current)) => {
                    let is_float = matches!(current, Value::Float(_));
                    Ok(coerce(interpreter.binary(binary, current, value)?, is_float))
                }
                (Some(_), None) => Err(interpreter.error("compound assignment to an undefined value")),
            }
        };

        match target.unlocated() {
            AstNode::Identifier(name) => {
                let current = env.get(name);
                let value = new_value(self, current)?;
                if !env.assign(name, value.clone()) {
                    env.define(name, value);
                }
                Ok(())
            }
            AstNode::Index { target, index } => {
                let container = self.eval(target, env)?;
                let index = self.eval(index, env)?;
                match &container {
                    Value::List(items) => {
                        let position = self.list_position(items.borrow().len(), &index)?;
                        let current = items.borrow()[position].clone();
                        let value = new_value(self, Some(current))?;
                        items.borrow_mut()[position] = value;
                        Ok(())
                    }
                    Value::Object(object) => {
                        let key = match &index {
                            Value::Str(key) => key.to_string(),
                            other => {
                                return Err(self.error(format!(
                                    "dictionary keys must be strings, found {}",
                                    other.type_name()
                                )))
                            }
                        };
                        let current = object.borrow().get(&key).cloned();
                        let value = if op.binary().is_some() && current.is_none() {
                            return Err(self.error(format!("key '{}' not found", key)));
                        } else {
                            new_value(self, current)?
                        };
                        object.borrow_mut().set(&key, value);
                        Ok(())
                    }
                    other => Err(self.error(format!(
                        "cannot assign by index to a value of type {}",
                        other.type_name()
                    ))),
                }
            }
            AstNode::Member { object, name } => {
                let object = self.eval(object, env)?;
                match &object {
                    Value::Object(fields) => {
                        let current = fields.borrow().get(name).cloned();
                        let value = new_value(self, current)?;
                        fields.borrow_mut().set(name, value);
                        Ok(())
                    }
                    other => Err(self.error(format!(
                        "cannot assign field '{}' on a value of type {}",
                        name,
                        other.type_name()
                    ))),
                }
            }
            _ => Err(self.error("invalid assignment target")),
        }
    }
}
