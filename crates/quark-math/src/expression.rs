use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use mlua::prelude::*;
use quark_core::create_userdata;

const MAX_DEPTH: usize = 128;
const MAX_LENGTH: usize = 10_000;

#[derive(Debug, Clone)]
enum Node {
    Number(f64),
    Variable(String),
    Negate(Box<Node>),
    Not(Box<Node>),
    Binary(Operator, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Power,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    Symbol(&'static str),
    Open,
    Close,
    Comma,
}

fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    if text.len() > MAX_LENGTH {
        return Err(format!(
            "expressions can have at most {MAX_LENGTH} characters"
        ));
    }
    let characters: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if character.is_whitespace() {
            index += 1;
            continue;
        }
        if character.is_ascii_digit()
            || (character == '.' && characters.get(index + 1).is_some_and(char::is_ascii_digit))
        {
            let start = index;
            while index < characters.len()
                && (characters[index].is_ascii_digit() || characters[index] == '.')
            {
                index += 1;
            }
            if index < characters.len() && matches!(characters[index], 'e' | 'E') {
                let mut lookahead = index + 1;
                if lookahead < characters.len() && matches!(characters[lookahead], '+' | '-') {
                    lookahead += 1;
                }
                if lookahead < characters.len() && characters[lookahead].is_ascii_digit() {
                    index = lookahead;
                    while index < characters.len() && characters[index].is_ascii_digit() {
                        index += 1;
                    }
                }
            }
            let literal: String = characters[start..index].iter().collect();
            let number = literal
                .parse()
                .map_err(|_| format!("invalid number '{literal}'"))?;
            tokens.push(Token::Number(number));
            continue;
        }
        if character.is_alphabetic() || character == '_' {
            let start = index;
            while index < characters.len()
                && (characters[index].is_alphanumeric() || characters[index] == '_')
            {
                index += 1;
            }
            tokens.push(Token::Name(characters[start..index].iter().collect()));
            continue;
        }
        let two: String = characters[index..(index + 2).min(characters.len())]
            .iter()
            .collect();
        let symbol = match two.as_str() {
            "<=" => Some("<="),
            ">=" => Some(">="),
            "==" => Some("=="),
            "!=" | "~=" => Some("!="),
            "&&" => Some("and"),
            "||" => Some("or"),
            "**" => Some("^"),
            _ => None,
        };
        if let Some(symbol) = symbol {
            tokens.push(Token::Symbol(symbol));
            index += 2;
            continue;
        }
        tokens.push(match character {
            '+' => Token::Symbol("+"),
            '-' => Token::Symbol("-"),
            '*' => Token::Symbol("*"),
            '/' => Token::Symbol("/"),
            '%' => Token::Symbol("%"),
            '^' => Token::Symbol("^"),
            '<' => Token::Symbol("<"),
            '>' => Token::Symbol(">"),
            '!' => Token::Symbol("not"),
            '(' => Token::Open,
            ')' => Token::Close,
            ',' => Token::Comma,
            other => return Err(format!("unexpected character '{other}'")),
        });
        index += 1;
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).cloned();
        self.position += 1;
        token
    }

    fn operator(&self) -> Option<(Operator, u8, bool)> {
        let operator = match self.peek()? {
            Token::Symbol("or") => (Operator::Or, 1, false),
            Token::Name(name) if name == "or" => (Operator::Or, 1, false),
            Token::Symbol("and") => (Operator::And, 2, false),
            Token::Name(name) if name == "and" => (Operator::And, 2, false),
            Token::Symbol("==") => (Operator::Equal, 3, false),
            Token::Symbol("!=") => (Operator::NotEqual, 3, false),
            Token::Symbol("<") => (Operator::Less, 4, false),
            Token::Symbol("<=") => (Operator::LessEqual, 4, false),
            Token::Symbol(">") => (Operator::Greater, 4, false),
            Token::Symbol(">=") => (Operator::GreaterEqual, 4, false),
            Token::Symbol("+") => (Operator::Add, 5, false),
            Token::Symbol("-") => (Operator::Subtract, 5, false),
            Token::Symbol("*") => (Operator::Multiply, 6, false),
            Token::Symbol("/") => (Operator::Divide, 6, false),
            Token::Symbol("%") => (Operator::Modulo, 6, false),
            Token::Symbol("^") => (Operator::Power, 8, true),
            _ => return None,
        };
        Some(operator)
    }

    fn expression(&mut self, minimum: u8) -> Result<Node, String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err("the expression is nested too deeply".to_owned());
        }
        let mut left = self.unary()?;
        while let Some((operator, precedence, right_associative)) = self.operator() {
            if precedence < minimum {
                break;
            }
            self.next();
            let right = self.expression(if right_associative {
                precedence
            } else {
                precedence + 1
            })?;
            left = Node::Binary(operator, Box::new(left), Box::new(right));
        }
        self.depth -= 1;
        Ok(left)
    }

    fn unary(&mut self) -> Result<Node, String> {
        match self.peek() {
            Some(Token::Symbol("-")) => {
                self.next();
                Ok(Node::Negate(Box::new(self.expression(7)?)))
            }
            Some(Token::Symbol("+")) => {
                self.next();
                self.expression(7)
            }
            Some(Token::Symbol("not")) => {
                self.next();
                Ok(Node::Not(Box::new(self.expression(3)?)))
            }
            Some(Token::Name(name)) if name == "not" => {
                self.next();
                Ok(Node::Not(Box::new(self.expression(3)?)))
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Result<Node, String> {
        match self.next() {
            Some(Token::Number(number)) => Ok(Node::Number(number)),
            Some(Token::Open) => {
                let inner = self.expression(0)?;
                match self.next() {
                    Some(Token::Close) => Ok(inner),
                    _ => Err("missing ')'".to_owned()),
                }
            }
            Some(Token::Name(name)) => {
                if self.peek() != Some(&Token::Open) {
                    return Ok(Node::Variable(name));
                }
                self.next();
                let mut arguments = Vec::new();
                if self.peek() == Some(&Token::Close) {
                    self.next();
                } else {
                    loop {
                        arguments.push(self.expression(0)?);
                        match self.next() {
                            Some(Token::Comma) => continue,
                            Some(Token::Close) => break,
                            _ => return Err(format!("missing ')' after the arguments of {name}")),
                        }
                    }
                }
                Ok(Node::Call(name, arguments))
            }
            Some(token) => Err(format!("unexpected {}", describe(&token))),
            None => Err("the expression ended unexpectedly".to_owned()),
        }
    }
}

fn describe(token: &Token) -> String {
    match token {
        Token::Number(number) => format!("number {number}"),
        Token::Name(name) => format!("'{name}'"),
        Token::Symbol(symbol) => format!("'{symbol}'"),
        Token::Open => "'('".to_owned(),
        Token::Close => "')'".to_owned(),
        Token::Comma => "','".to_owned(),
    }
}

const FUNCTIONS: &[(&str, usize, usize)] = &[
    ("abs", 1, 1),
    ("sqrt", 1, 1),
    ("cbrt", 1, 1),
    ("exp", 1, 1),
    ("ln", 1, 1),
    ("log", 1, 2),
    ("log10", 1, 1),
    ("log2", 1, 1),
    ("sin", 1, 1),
    ("cos", 1, 1),
    ("tan", 1, 1),
    ("asin", 1, 1),
    ("acos", 1, 1),
    ("atan", 1, 1),
    ("atan2", 2, 2),
    ("floor", 1, 1),
    ("ceil", 1, 1),
    ("round", 1, 2),
    ("trunc", 1, 1),
    ("sign", 1, 1),
    ("min", 1, 64),
    ("max", 1, 64),
    ("clamp", 3, 3),
    ("lerp", 3, 3),
    ("pow", 2, 2),
    ("hypot", 2, 2),
    ("if", 3, 3),
    ("rad", 1, 1),
    ("deg", 1, 1),
];

fn check(node: &Node, variables: &mut BTreeSet<String>) -> Result<(), String> {
    match node {
        Node::Number(_) => Ok(()),
        Node::Variable(name) => {
            if !matches!(name.as_str(), "pi" | "e" | "inf" | "true" | "false") {
                variables.insert(name.clone());
            }
            Ok(())
        }
        Node::Negate(inner) | Node::Not(inner) => check(inner, variables),
        Node::Binary(_, left, right) => {
            check(left, variables)?;
            check(right, variables)
        }
        Node::Call(name, arguments) => {
            let (_, minimum, maximum) = FUNCTIONS
                .iter()
                .find(|(function, _, _)| function == name)
                .ok_or_else(|| format!("unknown function '{name}'"))?;
            if arguments.len() < *minimum || arguments.len() > *maximum {
                return Err(if minimum == maximum {
                    format!("{name} takes {minimum} argument(s)")
                } else {
                    format!("{name} takes {minimum} to {maximum} arguments")
                });
            }
            arguments
                .iter()
                .try_for_each(|argument| check(argument, variables))
        }
    }
}

fn truth(value: f64) -> bool {
    value != 0.0 && !value.is_nan()
}

fn evaluate(node: &Node, variables: &HashMap<String, f64>) -> Result<f64, String> {
    Ok(match node {
        Node::Number(number) => *number,
        Node::Variable(name) => match variables.get(name) {
            Some(value) => *value,
            None => match name.as_str() {
                "pi" => std::f64::consts::PI,
                "e" => std::f64::consts::E,
                "inf" => f64::INFINITY,
                "true" => 1.0,
                "false" => 0.0,
                _ => return Err(format!("the variable '{name}' has no value")),
            },
        },
        Node::Negate(inner) => -evaluate(inner, variables)?,
        Node::Not(inner) => f64::from(!truth(evaluate(inner, variables)?)),
        Node::Binary(Operator::And, left, right) => {
            let left = evaluate(left, variables)?;
            if truth(left) {
                evaluate(right, variables)?
            } else {
                left
            }
        }
        Node::Binary(Operator::Or, left, right) => {
            let left = evaluate(left, variables)?;
            if truth(left) {
                left
            } else {
                evaluate(right, variables)?
            }
        }
        Node::Binary(operator, left, right) => {
            let (left, right) = (evaluate(left, variables)?, evaluate(right, variables)?);
            match operator {
                Operator::Add => left + right,
                Operator::Subtract => left - right,
                Operator::Multiply => left * right,
                Operator::Divide => left / right,
                Operator::Modulo => left - (left / right).floor() * right,
                Operator::Power => left.powf(right),
                Operator::Less => f64::from(left < right),
                Operator::LessEqual => f64::from(left <= right),
                Operator::Greater => f64::from(left > right),
                Operator::GreaterEqual => f64::from(left >= right),
                Operator::Equal => f64::from(left == right),
                Operator::NotEqual => f64::from(left != right),
                Operator::And | Operator::Or => unreachable!(),
            }
        }
        Node::Call(name, arguments) => {
            if name == "if" {
                return if truth(evaluate(&arguments[0], variables)?) {
                    evaluate(&arguments[1], variables)
                } else {
                    evaluate(&arguments[2], variables)
                };
            }
            let values = arguments
                .iter()
                .map(|argument| evaluate(argument, variables))
                .collect::<Result<Vec<_>, _>>()?;
            let first = values[0];
            match name.as_str() {
                "abs" => first.abs(),
                "sqrt" => first.sqrt(),
                "cbrt" => first.cbrt(),
                "exp" => first.exp(),
                "ln" => first.ln(),
                "log" => match values.get(1) {
                    Some(base) => first.log(*base),
                    None => first.ln(),
                },
                "log10" => first.log10(),
                "log2" => first.log2(),
                "sin" => first.sin(),
                "cos" => first.cos(),
                "tan" => first.tan(),
                "asin" => first.asin(),
                "acos" => first.acos(),
                "atan" => first.atan(),
                "atan2" => first.atan2(values[1]),
                "floor" => first.floor(),
                "ceil" => first.ceil(),
                "round" => {
                    let factor = 10f64.powi(values.get(1).copied().unwrap_or(0.0) as i32);
                    (first * factor).round() / factor
                }
                "trunc" => first.trunc(),
                "sign" => {
                    if first > 0.0 {
                        1.0
                    } else if first < 0.0 {
                        -1.0
                    } else {
                        0.0
                    }
                }
                "min" => values.iter().copied().fold(f64::INFINITY, f64::min),
                "max" => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                "clamp" => first.clamp(values[1].min(values[2]), values[1].max(values[2])),
                "lerp" => first + (values[1] - first) * values[2],
                "pow" => first.powf(values[1]),
                "hypot" => first.hypot(values[1]),
                "rad" => first.to_radians(),
                "deg" => first.to_degrees(),
                _ => return Err(format!("unknown function '{name}'")),
            }
        }
    })
}

pub struct Expression {
    root: Arc<Node>,
    source: String,
    variables: BTreeSet<String>,
}

fn compile(source: &str) -> Result<Expression, String> {
    let mut parser = Parser {
        tokens: tokenize(source)?,
        position: 0,
        depth: 0,
    };
    if parser.tokens.is_empty() {
        return Err("the expression is empty".to_owned());
    }
    let root = parser.expression(0)?;
    if let Some(token) = parser.peek() {
        return Err(format!("unexpected {}", describe(token)));
    }
    let mut variables = BTreeSet::new();
    check(&root, &mut variables)?;
    Ok(Expression {
        root: Arc::new(root),
        source: source.to_owned(),
        variables,
    })
}

fn read_variables(table: Option<LuaTable>) -> LuaResult<HashMap<String, f64>> {
    let mut variables = HashMap::new();
    if let Some(table) = table {
        for pair in table.pairs::<String, LuaValue>() {
            let (name, value) = pair?;
            let value = match value {
                LuaValue::Integer(integer) => integer as f64,
                LuaValue::Number(number) => number,
                LuaValue::Boolean(flag) => f64::from(flag),
                _ => continue,
            };
            variables.insert(name, value);
        }
    }
    Ok(variables)
}

fn result(lua: &Lua, value: Result<f64, String>) -> LuaResult<LuaMultiValue> {
    match value {
        Ok(number) if number.fract() == 0.0 && number.abs() < 9.0e15 => {
            (number as i64).into_lua_multi(lua)
        }
        Ok(number) => number.into_lua_multi(lua),
        Err(message) => (LuaNil, message).into_lua_multi(lua),
    }
}

impl LuaUserData for Expression {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "Evaluate",
            |lua, expression, variables: Option<LuaTable>| {
                result(lua, evaluate(&expression.root, &read_variables(variables)?))
            },
        );
        methods.add_method("GetVariables", |lua, expression, ()| {
            lua.create_sequence_from(expression.variables.iter().cloned())
        });
        methods.add_method("GetSource", |_, expression, ()| {
            Ok(expression.source.clone())
        });
        methods.add_meta_method(
            LuaMetaMethod::Call,
            |lua, expression, variables: Option<LuaTable>| {
                result(lua, evaluate(&expression.root, &read_variables(variables)?))
            },
        );
        methods.add_meta_method(LuaMetaMethod::ToString, |_, expression, ()| {
            Ok(format!("Expression({})", expression.source))
        });
    }
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "Compile",
        lua.create_function(|lua, source: String| match compile(&source) {
            Ok(expression) => create_userdata(lua, expression)?.into_lua_multi(lua),
            Err(message) => (LuaNil, format!("invalid expression: {message}")).into_lua_multi(lua),
        })?,
    )?;
    module.set(
        "Evaluate",
        lua.create_function(
            |lua, (source, variables): (String, Option<LuaTable>)| match compile(&source) {
                Ok(expression) => {
                    result(lua, evaluate(&expression.root, &read_variables(variables)?))
                }
                Err(message) => {
                    (LuaNil, format!("invalid expression: {message}")).into_lua_multi(lua)
                }
            },
        )?,
    )?;
    Ok(())
}
