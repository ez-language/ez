//! `match`: arms and patterns.

use super::*;

impl Interpreter {
    pub(super) fn eval_match(&mut self, value: &Value, arms: &[MatchArm], env: &Rc<Env>) -> Exec<Value> {
        for arm in arms {
            let scope = Env::child(env);
            if !self.match_pattern(&arm.pattern, value, &scope)? {
                continue;
            }
            if let Some(guard) = &arm.guard {
                if !self.condition(guard, &scope)? {
                    continue;
                }
            }
            return self.exec_block(&arm.body, &scope);
        }
        Err(self.error(format!("no match arm matches the value {}", value.repr())))
    }

    /// Whether `name` in a pattern names a variant (rather than introducing a binding).
    pub(super) fn is_variant_name(env: &Rc<Env>, name: &str) -> bool {
        match env.get(name) {
            Some(Value::Constructor(constructor)) => constructor.variant == name,
            Some(Value::Variant(variant)) => variant.name == name && variant.values.is_empty(),
            _ => false,
        }
    }

    pub(super) fn match_pattern(&mut self, pattern: &Pattern, value: &Value, env: &Rc<Env>) -> Exec<bool> {
        match pattern {
            Pattern::Wildcard => Ok(true),
            Pattern::Identifier(name) => {
                // A name is a variant when the matched value's type has it (`Null` against a
                // `Json`), even if the variant is not in scope.
                let is_variant = match value {
                    Value::Variant(variant) => variant.sum.has_variant(name),
                    _ => false,
                } || Self::is_variant_name(env, name);
                if is_variant {
                    return Ok(matches!(value, Value::Variant(v) if v.name == *name));
                }
                env.define(name, value.clone());
                Ok(true)
            }
            Pattern::Variant { name, args } => match value {
                Value::Variant(variant) if variant.name == *name && variant.values.len() == args.len() => {
                    for (sub_pattern, sub_value) in args.iter().zip(&variant.values) {
                        if !self.match_pattern(sub_pattern, sub_value, env)? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                _ => Ok(false),
            },
            Pattern::Literal(literal) => {
                let literal = self.eval(literal, env)?;
                Ok(literal.equals(value))
            }
        }
    }
}
