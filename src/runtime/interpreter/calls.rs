//! Calls: functions, methods, `mut` arguments and `display`.

use super::*;

impl Interpreter {
    pub(super) fn eval_call(
        &mut self,
        callee: &AstNode,
        args: &[AstNode],
        method: Option<MethodRef>,
        displays: &[Option<MethodRef>],
        env: &Rc<Env>,
    ) -> Exec<Value> {
        // A method (`person.greet()`) or an associated function (`Person.create()`).
        if let Some(method) = method {
            let closure = match self.methods.get(&(method.module.clone(), method.key.clone())) {
                Some(closure) => Rc::clone(closure),
                None => return Err(self.error(format!("method '{}' not found", method.key))),
            };
            let mut values = Vec::new();
            let mut places = Vec::new();
            if method.has_self {
                if let AstNode::Member { object, .. } = callee.unlocated() {
                    // `mut self`: the value is stored back where it came from, like a `mut`
                    // argument.
                    if closure.params[0].is_mut {
                        let (value, place) = self.eval_place(object, env)?;
                        values.push(value);
                        places.push((0, place));
                    } else {
                        values.push(self.eval(object, env)?);
                    }
                }
            }
            let offset = values.len();
            let (rest, arg_places) = self.eval_args(args, env)?;
            values.extend(rest);
            places.extend(arg_places.into_iter().map(|(i, place)| (i + offset, place)));
            return self.call_with_places(&Value::Function(closure), values, places, env);
        }
        if let AstNode::Member { object, name } = callee.unlocated() {
            let object = self.eval(object, env)?;
            let (args, places) = self.eval_args(args, env)?;
            if !places.is_empty() {
                let function = self.member(&object, name)?;
                return self.call_with_places(&function, args, places, env);
            }
            return self.call_method(&object, name, args);
        }
        let callee = self.eval(callee, env)?;
        let (mut args, places) = self.eval_args(args, env)?;
        // `print(person)`: values whose type declares `display` are shown with it.
        for (arg, display) in args.iter_mut().zip(displays) {
            if display.is_some() {
                let value = std::mem::replace(arg, Value::Void);
                *arg = Value::str(&self.display(value, display.as_ref())?);
            }
        }
        self.call_with_places(&callee, args, places, env)
    }

    /// Text of a value for `print` and interpolation: its type's `display` method, if declared.
    pub(super) fn display(&mut self, value: Value, display: Option<&MethodRef>) -> Exec<String> {
        let method = match display {
            Some(method) => method,
            None => return Ok(value.display()),
        };
        let closure = match self.methods.get(&(method.module.clone(), method.key.clone())) {
            Some(closure) => Rc::clone(closure),
            None => return Err(self.error(format!("method '{}' not found", method.key))),
        };
        match self.call_closure(&closure, vec![value])? {
            Value::Str(text) => Ok(text.to_string()),
            other => Ok(other.display()),
        }
    }

    /// The argument values, and where each `mut` argument came from (by argument position).
    pub(super) fn eval_args(&mut self, args: &[AstNode], env: &Rc<Env>) -> Exec<Args> {
        let mut values = Vec::with_capacity(args.len());
        let mut places = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            match arg.unlocated() {
                AstNode::MutArg(place) => {
                    let (value, place) = self.eval_place(place, env)?;
                    values.push(value);
                    places.push((i, place));
                }
                other => values.push(self.eval(other, env)?),
            }
        }
        Ok((values, places))
    }

    pub(super) fn eval_place(&mut self, node: &AstNode, env: &Rc<Env>) -> Exec<(Value, Place)> {
        match node.unlocated() {
            AstNode::Identifier(name) => match env.get(name) {
                Some(value) => Ok((value, Place::Variable(name.clone()))),
                None => Err(self.error(format!("undefined variable '{}'", name))),
            },
            AstNode::Member { object, name } => {
                let object = self.eval(object, env)?;
                let value = self.member(&object, name)?;
                Ok((value, Place::Field(object, name.clone())))
            }
            AstNode::Index { target, index } => {
                let target = self.eval(target, env)?;
                let index = self.eval(index, env)?;
                let value = self.index(&target, &index)?;
                Ok((value, Place::Element(target, index)))
            }
            _ => Err(self.error("only a variable, a field or an element can be passed as 'mut'")),
        }
    }

    pub(super) fn set_place(&mut self, place: Place, value: Value, env: &Rc<Env>) -> Exec<()> {
        match place {
            Place::Variable(name) => {
                env.assign(&name, value);
            }
            Place::Field(Value::Object(fields), name) => fields.borrow_mut().set(&name, value),
            Place::Element(Value::List(items), index) => {
                let position = self.list_position(items.borrow().len(), &index)?;
                items.borrow_mut()[position] = value;
            }
            Place::Element(Value::Object(object), Value::Str(key)) => object.borrow_mut().set(&key, value),
            _ => return Err(self.error("invalid 'mut' argument")),
        }
        Ok(())
    }

    /// Calls `callee`; then stores the final value of each `mut` parameter (by position) back
    /// where its argument came from.
    pub(super) fn call_with_places(
        &mut self,
        callee: &Value,
        args: Vec<Value>,
        places: Vec<(usize, Place)>,
        env: &Rc<Env>,
    ) -> Exec<Value> {
        let closure = match callee {
            Value::Function(closure) => closure,
            other => return self.call_value(other, args),
        };
        let (value, scope) = self.run_closure(closure, args)?;
        for (i, place) in places {
            let param = &closure.params[i].name;
            if let Some(final_value) = scope.get_local(param) {
                self.set_place(place, final_value, env)?;
            }
        }
        Ok(value)
    }

    pub(crate) fn call_value(&mut self, callee: &Value, args: Vec<Value>) -> Exec<Value> {
        match callee {
            Value::Function(closure) => self.call_closure(closure, args),
            Value::Builtin(name) => stdlib::call(self, name, args),
            Value::Constructor(constructor) => {
                if args.len() != constructor.arity() {
                    return Err(self.error(format!(
                        "'{}' expects {} argument(s), found {}",
                        constructor.variant,
                        constructor.arity(),
                        args.len()
                    )));
                }
                Ok(constructor.sum.make(&constructor.variant, args))
            }
            other => Err(self.error(format!("a value of type {} cannot be called", other.type_name()))),
        }
    }

    pub(super) fn call_closure(&mut self, closure: &Rc<Closure>, args: Vec<Value>) -> Exec<Value> {
        self.run_closure(closure, args).map(|(value, _)| value)
    }

    /// Runs a function; also returns the scope of its parameters.
    pub(super) fn run_closure(&mut self, closure: &Rc<Closure>, args: Vec<Value>) -> Exec<(Value, Rc<Env>)> {
        if args.len() != closure.params.len() {
            return Err(self.error(format!(
                "'{}' expects {} argument(s), found {}",
                closure.name,
                closure.params.len(),
                args.len()
            )));
        }
        let scope = Env::child(&closure.env);
        for ((param, value), to_float) in closure.params.iter().zip(args).zip(&closure.float_params) {
            scope.define(&param.name, coerce(value, *to_float));
        }

        let saved_file = std::mem::replace(&mut self.file, Rc::clone(&closure.file));
        let saved_line = self.line;
        self.function_depth += 1;
        let result = match closure.body.unlocated() {
            AstNode::Block(statements) => self.exec_statements(statements, &scope),
            expression => self.eval(expression, &scope),
        };
        self.function_depth -= 1;
        self.file = saved_file;
        self.line = saved_line;

        let value = match result {
            Ok(value) | Err(Flow::Return(value)) => value,
            Err(Flow::Break) | Err(Flow::Continue) => Value::Void,
            Err(error) => return Err(error),
        };
        Ok((coerce(value, closure.returns_float), scope))
    }

    pub(super) fn call_method(&mut self, object: &Value, name: &str, args: Vec<Value>) -> Exec<Value> {
        // Every value has `display()` and `debug()`, unless a field of an object has the name.
        let is_field = matches!(object, Value::Object(fields) if fields.borrow().get(name).is_some());
        if !is_field && !matches!(object, Value::Module(_)) {
            match name {
                "display" => return Ok(Value::str(&object.display())),
                "debug" => return Ok(Value::str(&object.repr())),
                _ => {}
            }
        }
        match object {
            Value::Module(_) => {
                let member = self.member(object, name)?;
                self.call_value(&member, args)
            }
            Value::Str(s) => stdlib::string_method(self, s, name, args),
            Value::List(items) => match name {
                "length" => Ok(Value::Int(items.borrow().len() as i64)),
                _ => Err(self.error(format!("lists have no method '{}'", name))),
            },
            Value::Object(fields) => {
                if name == "length" && fields.borrow().get("length").is_none() {
                    return Ok(Value::Int(fields.borrow().fields.len() as i64));
                }
                let member = self.member(object, name)?;
                self.call_value(&member, args)
            }
            Value::Native(native) => stdlib::native_method(self, native, name, args),
            Value::Variant(variant) if variant.sum.name == "Result" && name == "mapErr" => {
                if variant.name != "Err" {
                    return Ok(object.clone());
                }
                let converter = args.into_iter().next().unwrap_or(Value::Void);
                let error = self.call_value(&converter, vec![variant.values[0].clone()])?;
                Ok(stdlib::err(error))
            }
            other => Err(self.error(format!(
                "a value of type {} has no method '{}'",
                other.type_name(),
                name
            ))),
        }
    }
}
