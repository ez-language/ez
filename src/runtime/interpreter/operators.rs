//! Unary and binary operators.

use super::*;

impl Interpreter {
    pub(super) fn unary(&self, op: &UnaryOperator, value: Value) -> Exec<Value> {
        match (op, value) {
            (UnaryOperator::Negate, Value::Int(n)) => match n.checked_neg() {
                Some(n) => Ok(Value::Int(n)),
                None => Err(self.error("integer overflow")),
            },
            (UnaryOperator::Negate, Value::Float(n)) => Ok(Value::Float(-n)),
            (UnaryOperator::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            (UnaryOperator::BitNot, Value::Int(n)) => Ok(Value::Int(!n)),
            (_, value) => Err(self.error(format!(
                "invalid operand of type {} for a unary operator",
                value.type_name()
            ))),
        }
    }

    pub(super) fn eval_binary(
        &mut self,
        left: &AstNode,
        op: BinaryOperator,
        right: &AstNode,
        env: &Rc<Env>,
    ) -> Exec<Value> {
        match op {
            BinaryOperator::And | BinaryOperator::Or => {
                let left_value = self.condition(left, env)?;
                if (op == BinaryOperator::And) != left_value {
                    return Ok(Value::Bool(left_value));
                }
                Ok(Value::Bool(self.condition(right, env)?))
            }
            BinaryOperator::Is => {
                let left_value = self.eval(left, env)?;
                let right_value = self.eval(right, env)?;
                // `value is Some`: variant test.
                if let AstNode::Identifier(name) = right.unlocated() {
                    let is_variant_name = match &right_value {
                        Value::Constructor(constructor) => constructor.variant == *name,
                        Value::Variant(variant) => variant.name == *name && variant.values.is_empty(),
                        _ => false,
                    };
                    if is_variant_name {
                        return Ok(Value::Bool(match &left_value {
                            Value::Variant(variant) => variant.name == *name,
                            _ => false,
                        }));
                    }
                }
                Ok(Value::Bool(left_value.identical(&right_value)))
            }
            _ => {
                let left = self.eval(left, env)?;
                let right = self.eval(right, env)?;
                self.binary(op, left, right)
            }
        }
    }

    pub(crate) fn binary(&self, op: BinaryOperator, left: Value, right: Value) -> Exec<Value> {
        use BinaryOperator::*;
        let overflow = || self.error("integer overflow");
        match (op, &left, &right) {
            (Equal, _, _) => Ok(Value::Bool(left.equals(&right))),
            (NotEqual, _, _) => Ok(Value::Bool(!left.equals(&right))),
            (Is, _, _) => Ok(Value::Bool(left.identical(&right))),
            (In, _, _) => self.contains(&right, &left).map(Value::Bool),

            (Add, Value::Int(a), Value::Int(b)) => a.checked_add(*b).map(Value::Int).ok_or_else(overflow),
            (Subtract, Value::Int(a), Value::Int(b)) => a.checked_sub(*b).map(Value::Int).ok_or_else(overflow),
            (Multiply, Value::Int(a), Value::Int(b)) => a.checked_mul(*b).map(Value::Int).ok_or_else(overflow),
            // Like Go and Rust: integer division truncates toward zero and the remainder has
            // the sign of the dividend, so `a == (a / b) * b + a % b`.
            (Divide | Modulo, Value::Int(a), Value::Int(b)) => {
                if *b == 0 {
                    return Err(self.error("division by zero"));
                }
                let result = if op == Divide {
                    a.checked_div(*b)
                } else {
                    a.checked_rem(*b)
                };
                result.map(Value::Int).ok_or_else(overflow)
            }
            (Power, Value::Int(a), Value::Int(b)) => {
                if *a == 0 && *b < 0 {
                    return Err(self.error("division by zero"));
                }
                if *b < 0 {
                    return Err(self.error("negative exponent with int operands; use a float base"));
                }
                let exponent = u32::try_from(*b).map_err(|_| overflow())?;
                a.checked_pow(exponent).map(Value::Int).ok_or_else(overflow)
            }
            (
                Add | Subtract | Multiply | Divide | Modulo | Power,
                Value::Int(_) | Value::Float(_),
                Value::Int(_) | Value::Float(_),
            ) => {
                let (a, b) = (as_float(&left), as_float(&right));
                let result = match op {
                    Add => a + b,
                    Subtract => a - b,
                    Multiply => a * b,
                    // Operators cannot return a Result: every failure panics.
                    Power => match stdlib::real_pow(a, b) {
                        Ok(result) => result,
                        Err(PowFailure::Complex) => {
                            return Err(self.error(format!(
                                "{} ** {} is not a real number (use math.pow to handle it)",
                                format_float(a),
                                format_float(b)
                            )))
                        }
                        // `0 ** -1` is `1 / 0`.
                        Err(PowFailure::Infinite) => return Err(self.error("division by zero")),
                        Err(PowFailure::Overflow) => return Err(self.error("float overflow")),
                    },
                    _ => {
                        if b == 0.0 {
                            return Err(self.error("division by zero"));
                        }
                        if op == Divide {
                            a / b
                        } else {
                            a % b
                        }
                    }
                };
                // `float` has no infinity: a result that does not fit is a representation limit.
                if !result.is_finite() {
                    return Err(self.error("float overflow"));
                }
                Ok(Value::Float(result))
            }
            (Add, Value::Str(a), Value::Str(b)) => Ok(Value::str(&format!("{}{}", a, b))),
            (Multiply, Value::Str(s), Value::Int(n)) | (Multiply, Value::Int(n), Value::Str(s)) => {
                Ok(Value::str(&s.repeat((*n).max(0) as usize)))
            }
            (Add, Value::List(a), Value::List(b)) => {
                let mut items = a.borrow().clone();
                items.extend(b.borrow().iter().cloned());
                Ok(Value::list(items))
            }

            (Greater | GreaterEqual | Less | LessEqual, _, _) => {
                let ordering = match (&left, &right) {
                    (Value::Int(a), Value::Int(b)) => a.partial_cmp(b),
                    (Value::Int(_) | Value::Float(_), Value::Int(_) | Value::Float(_)) => {
                        as_float(&left).partial_cmp(&as_float(&right))
                    }
                    (Value::Str(a), Value::Str(b)) => a.partial_cmp(b),
                    _ => {
                        return Err(self.error(format!(
                            "cannot compare {} with {}",
                            left.type_name(),
                            right.type_name()
                        )))
                    }
                };
                let result = match ordering {
                    Some(ordering) => match op {
                        Greater => ordering.is_gt(),
                        GreaterEqual => ordering.is_ge(),
                        Less => ordering.is_lt(),
                        _ => ordering.is_le(),
                    },
                    None => false,
                };
                Ok(Value::Bool(result))
            }

            (BitAnd, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a & b)),
            (BitOr, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a | b)),
            (BitXor, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a ^ b)),
            (ShiftLeft | ShiftRight, Value::Int(a), Value::Int(b)) => {
                if !(0..64).contains(b) {
                    return Err(self.error(format!("invalid shift amount {}", b)));
                }
                Ok(Value::Int(if op == ShiftLeft { a << b } else { a >> b }))
            }
            (And | Or, Value::Bool(a), Value::Bool(b)) => {
                Ok(Value::Bool(if op == And { *a && *b } else { *a || *b }))
            }
            _ => Err(self.error(format!(
                "operator '{}' cannot be applied to {} and {}",
                op.symbol(),
                left.type_name(),
                right.type_name()
            ))),
        }
    }

    /// `item in collection`
    pub(super) fn contains(&self, collection: &Value, item: &Value) -> Exec<bool> {
        match (collection, item) {
            (Value::Str(haystack), Value::Str(needle)) => Ok(haystack.contains(&**needle)),
            (Value::List(items), _) => Ok(items.borrow().iter().any(|v| v.equals(item))),
            (Value::Object(object), Value::Str(key)) => Ok(object.borrow().get(key).is_some()),
            _ => Err(self.error(format!(
                "'in' cannot check a {} inside a {}",
                item.type_name(),
                collection.type_name()
            ))),
        }
    }
}
