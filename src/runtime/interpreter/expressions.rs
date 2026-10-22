//! Evaluation of expressions and statements, `?`, indexing and members.

use super::*;

impl Interpreter {
    pub fn eval(&mut self, node: &AstNode, env: &Rc<Env>) -> Exec<Value> {
        match node {
            AstNode::Located { line, node } => {
                self.line = *line;
                self.eval(node, env)
            }
            AstNode::Export(node) => self.eval(node, env),
            AstNode::Try { value, wrap } => self.eval_try(value, wrap, env),
            AstNode::IntLiteral(n) => Ok(Value::Int(*n)),
            AstNode::FloatLiteral(n) => Ok(Value::Float(*n)),
            AstNode::StringLiteral(s) => Ok(Value::str(s)),
            AstNode::BooleanLiteral(b) => Ok(Value::Bool(*b)),
            AstNode::TemplateString(parts) => {
                let mut text = String::new();
                for part in parts {
                    match part {
                        TemplatePart::Text(s) => text.push_str(s),
                        TemplatePart::Expression(expression, display) => {
                            let value = self.eval(expression, env)?;
                            let display = display.borrow().clone();
                            text.push_str(&self.display(value, display.as_ref())?)
                        }
                    }
                }
                Ok(Value::str(&text))
            }
            AstNode::ArrayLiteral(elements) => {
                let mut items = Vec::with_capacity(elements.len());
                for element in elements {
                    items.push(self.eval(element, env)?);
                }
                Ok(Value::list(items))
            }
            AstNode::ObjectLiteral(entries) => {
                let mut fields = Vec::with_capacity(entries.len());
                for entry in entries {
                    fields.push((entry.key.clone(), self.eval(&entry.value, env)?));
                }
                Ok(Value::object(fields))
            }
            AstNode::Identifier(name) => match env.get(name) {
                Some(value) => Ok(value),
                None => Err(self.error(format!("undefined variable '{}'", name))),
            },
            AstNode::Unary { op, operand } => {
                let value = self.eval(operand, env)?;
                self.unary(op, value)
            }
            AstNode::BinaryOp { left, op, right } => self.eval_binary(left, *op, right, env),
            AstNode::Call {
                callee,
                args,
                method,
                displays,
            } => {
                let method = method.borrow().clone();
                let displays = displays.borrow().clone();
                self.eval_call(callee, args, method, &displays, env)
            }
            AstNode::MutArg(_) => Err(self.error("'mut' is only used before a call argument")),
            AstNode::Index { target, index } => {
                let target = self.eval(target, env)?;
                let index = self.eval(index, env)?;
                self.index(&target, &index)
            }
            AstNode::Member { object, name } => {
                let object = self.eval(object, env)?;
                self.member(&object, name)
            }
            AstNode::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if self.condition(condition, env)? {
                    let value = self.exec_block(then_branch, env)?;
                    Ok(if else_branch.is_some() { value } else { Value::Void })
                } else if let Some(else_branch) = else_branch {
                    self.exec_block(else_branch, env)
                } else {
                    Ok(Value::Void)
                }
            }
            AstNode::Match { scrutinee, arms } => {
                let value = self.eval(scrutinee, env)?;
                self.eval_match(&value, arms, env)
            }
            AstNode::Lambda {
                params,
                return_type,
                body,
            } => Ok(Value::Function(Rc::new(Closure {
                name: "anonymous function".to_string(),
                params: params.clone(),
                body: Rc::clone(body),
                env: Env::snapshot(env),
                float_params: params.iter().map(|p| is_float_type(&p.ty)).collect(),
                returns_float: is_float_type(return_type),
                file: Rc::clone(&self.file),
            }))),
            AstNode::Block(_) => self.exec_block(node, env),

            AstNode::While { condition, body } => {
                while self.condition(condition, env)? {
                    if !self.exec_loop_body(body, env)? {
                        break;
                    }
                }
                Ok(Value::Void)
            }
            AstNode::For {
                variable,
                is_mut,
                iterable,
                body,
            } => {
                let iterable = self.eval(iterable, env)?;
                let items: Vec<Value> = match &iterable {
                    Value::List(items) => items.borrow().clone(),
                    Value::Str(s) => s.chars().map(|c| Value::str(&c.to_string())).collect(),
                    Value::Object(object) => object
                        .borrow()
                        .fields
                        .iter()
                        .map(|(k, _)| Value::str(k))
                        .collect(),
                    other => {
                        return Err(self.error(format!(
                            "cannot iterate over a value of type {}",
                            other.type_name()
                        )))
                    }
                };
                for item in items {
                    let scope = Env::child(env);
                    // A `for mut` variable is a copy: changing it does not change the list.
                    scope.define(variable, if *is_mut { item.deep_copy() } else { item });
                    if !self.exec_loop_body(body, &scope)? {
                        break;
                    }
                }
                Ok(Value::Void)
            }
            AstNode::Break => Err(Flow::Break),
            AstNode::Continue => Err(Flow::Continue),
            AstNode::Return(value) => {
                let value = match value {
                    Some(value) => self.eval(value, env)?,
                    None => Value::Void,
                };
                Err(Flow::Return(value))
            }
            AstNode::Assign { target, op, value } => {
                self.eval_assign(target, op, value, env)?;
                Ok(Value::Void)
            }
            AstNode::VarDecl { name, ty, value, .. } => {
                let value = self.eval(value, env)?;
                env.define(name, coerce(value, is_float_type(ty)));
                Ok(Value::Void)
            }
            // A function declared inside another function captures a copy of the variables
            // around it where it is declared (it was hoisted with access to the whole block).
            AstNode::FunctionDecl {
                name,
                owner: None,
                params,
                return_type,
                body,
            } => {
                let captured = Env::snapshot(env);
                if !Rc::ptr_eq(&captured, env) {
                    env.define(
                        name,
                        Value::Function(Rc::new(Closure {
                            name: name.clone(),
                            params: params.clone(),
                            body: Rc::clone(body),
                            env: captured,
                            float_params: params.iter().map(|p| is_float_type(&p.ty)).collect(),
                            returns_float: is_float_type(return_type),
                            file: Rc::clone(&self.file),
                        })),
                    );
                }
                Ok(Value::Void)
            }
            AstNode::FunctionDecl { .. } | AstNode::TypeDecl { .. } => Ok(Value::Void),
            // Imports run when the block is hoisted.
            AstNode::Import { .. } => Ok(Value::Void),
        }
    }

    /// `value?`: the `Ok`/`Some` value, or an early return of the `Err`/`None`. At the top level
    /// of the main file, the failure stops the program.
    pub(super) fn eval_try(&mut self, value: &AstNode, wrap: &RefCell<Option<TryWrap>>, env: &Rc<Env>) -> Exec<Value> {
        let variant = match self.eval(value, env)? {
            Value::Variant(variant) => variant,
            other => {
                return Err(self.error(format!(
                    "'?' expects a Result or an Option, found {}",
                    other.type_name()
                )))
            }
        };
        let failure = match variant.name.as_str() {
            "Ok" | "Some" => return Ok(variant.values[0].clone()),
            "None" => stdlib::none(),
            _ => {
                let error = variant.values[0].clone();
                let error = match &*wrap.borrow() {
                    // Automatic conversion: wrap the error in the variant the checker chose.
                    Some(wrap) => {
                        let constructor = match &wrap.module {
                            None => env.get(&wrap.variant),
                            Some(module) => match env.get(module) {
                                Some(Value::Module(module)) => module.env.get_local(&wrap.variant),
                                _ => None,
                            },
                        };
                        match constructor {
                            Some(constructor) => self.call_value(&constructor, vec![error])?,
                            None => {
                                return Err(self.error(format!("'{}' is not in scope", wrap.variant)))
                            }
                        }
                    }
                    None => error,
                };
                stdlib::err(error)
            }
        };
        if self.function_depth == 0 {
            // The error itself, readable (`file 'a.txt' not found`), or `None`.
            let shown = match &failure {
                Value::Variant(variant) if variant.name == "Err" => variant.values[0].display(),
                other => other.display(),
            };
            return Err(Flow::Error(EzError::UnhandledError(format!(
                "{}: {}",
                self.location(),
                shown
            ))));
        }
        Err(Flow::Return(failure))
    }

    pub(super) fn condition(&mut self, condition: &AstNode, env: &Rc<Env>) -> Exec<bool> {
        match self.eval(condition, env)? {
            Value::Bool(b) => Ok(b),
            other => Err(self.error(format!(
                "condition must be boolean, found {}",
                other.type_name()
            ))),
        }
    }

    pub(super) fn list_position(&self, len: usize, index: &Value) -> Exec<usize> {
        match index {
            Value::Int(i) if *i >= 0 && (*i as usize) < len => Ok(*i as usize),
            Value::Int(i) => Err(self.error(format!(
                "index {} out of range (length {})",
                i, len
            ))),
            other => Err(self.error(format!("index must be int, found {}", other.type_name()))),
        }
    }

    pub(super) fn index(&self, target: &Value, index: &Value) -> Exec<Value> {
        match target {
            Value::List(items) => {
                let items = items.borrow();
                let position = self.list_position(items.len(), index)?;
                Ok(items[position].clone())
            }
            Value::Str(s) => {
                let chars: Vec<char> = s.chars().collect();
                let position = self.list_position(chars.len(), index)?;
                Ok(Value::str(&chars[position].to_string()))
            }
            Value::Object(object) => match index {
                Value::Str(key) => match object.borrow().get(key) {
                    Some(value) => Ok(value.clone()),
                    None => Err(self.error(format!("key '{}' not found", key))),
                },
                Value::Int(_) => Err(self.error("objects cannot be indexed by int")),
                other => Err(self.error(format!(
                    "dictionary keys must be strings, found {}",
                    other.type_name()
                ))),
            },
            other => Err(self.error(format!("cannot index a value of type {}", other.type_name()))),
        }
    }

    pub(super) fn member(&self, object: &Value, name: &str) -> Exec<Value> {
        match object {
            Value::Object(fields) => match fields.borrow().get(name) {
                Some(value) => Ok(value.clone()),
                None => Err(self.error(format!("field '{}' not found", name))),
            },
            Value::Variant(variant) => match variant.field(name) {
                Some(value) => Ok(value.clone()),
                None => Err(self.error(format!(
                    "variant '{}' has no field '{}'",
                    variant.name, name
                ))),
            },
            Value::Module(module) => match module.env.get_local(name) {
                Some(value) => Ok(value),
                None => Err(self.error(format!("module '{}' has no member '{}'", module.name, name))),
            },
            Value::Native(native) => match &*native.borrow() {
                crate::runtime::value::Native::DateTime(datetime) => match datetime.field(name) {
                    Some(n) => Ok(Value::Int(n)),
                    None => Err(self.error(format!("DateTime has no field '{}'", name))),
                },
                _ => Err(self.error(format!("'{}' must be called", name))),
            },
            other => Err(self.error(format!(
                "a value of type {} has no field '{}'",
                other.type_name(),
                name
            ))),
        }
    }
}
