//! `Num` — angka literal atau ekspresi yang merujuk `params`.
//!
//! Tata bahasa (recursive-descent, tanpa crate):
//! ```text
//! expr   := term (("+"|"-") term)*
//! term   := unary (("*"|"/") unary)*
//! unary  := "-" unary | atom
//! atom   := NUMBER | "$" IDENT | "(" expr ")"
//! IDENT  := [A-Za-z_][A-Za-z0-9_]*
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{OpError, OpErrorCode, OpResult};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Num {
    Value(f64),
    /// Expression, e.g. `"$w/2 - 3"`.
    Expr(String),
}

impl Default for Num {
    fn default() -> Self {
        Num::Value(0.0)
    }
}

impl From<f64> for Num {
    fn from(v: f64) -> Self {
        Num::Value(v)
    }
}

pub type Params = std::collections::BTreeMap<String, f64>;

/// Evaluasi `n` terhadap `params`.
pub fn eval(n: &Num, params: &Params) -> OpResult<f64> {
    let v = match n {
        Num::Value(v) => *v,
        Num::Expr(src) => {
            let mut p = Parser {
                src,
                pos: 0,
                params,
            };
            let v = p.expr()?;
            p.skip_ws();
            if p.pos < src.len() {
                return Err(p.syntax("karakter berlebih setelah ekspresi"));
            }
            v
        }
    };
    if !v.is_finite() {
        return Err(OpError::invalid(format!(
            "ekspresi {n:?} menghasilkan nilai tak hingga/NaN"
        )));
    }
    Ok(v)
}

/// Evaluasi larik `Num` sekaligus.
pub fn eval_arr<const N: usize>(a: &[Num; N], params: &Params) -> OpResult<[f64; N]> {
    let mut out = [0.0; N];
    for (o, n) in out.iter_mut().zip(a) {
        *o = eval(n, params)?;
    }
    Ok(out)
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
    params: &'a Params,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek().filter(|c| c.is_whitespace()) {
            self.pos += c.len_utf8();
        }
    }

    fn eat(&mut self, c: char) -> bool {
        self.skip_ws();
        if self.peek() == Some(c) {
            self.pos += c.len_utf8();
            true
        } else {
            false
        }
    }

    fn syntax(&self, what: &str) -> OpError {
        OpError::invalid(format!(
            "sintaks ekspresi \"{}\" salah di posisi {}: {what}",
            self.src, self.pos
        ))
        .with_context(serde_json::json!({ "expr": self.src, "pos": self.pos }))
    }

    fn expr(&mut self) -> OpResult<f64> {
        let mut v = self.term()?;
        loop {
            if self.eat('+') {
                v += self.term()?;
            } else if self.eat('-') {
                v -= self.term()?;
            } else {
                return Ok(v);
            }
        }
    }

    fn term(&mut self) -> OpResult<f64> {
        let mut v = self.unary()?;
        loop {
            if self.eat('*') {
                v *= self.unary()?;
            } else if self.eat('/') {
                let d = self.unary()?;
                if d == 0.0 {
                    return Err(OpError::invalid(format!(
                        "pembagian dengan nol dalam ekspresi \"{}\"",
                        self.src
                    )));
                }
                v /= d;
            } else {
                return Ok(v);
            }
        }
    }

    fn unary(&mut self) -> OpResult<f64> {
        if self.eat('-') {
            return Ok(-self.unary()?);
        }
        self.atom()
    }

    fn atom(&mut self) -> OpResult<f64> {
        self.skip_ws();
        if self.eat('(') {
            let v = self.expr()?;
            if !self.eat(')') {
                return Err(self.syntax("kurung tutup ')' tidak ditemukan"));
            }
            return Ok(v);
        }
        if self.eat('$') {
            let start = self.pos;
            let bytes = self.src.as_bytes();
            if self.pos < bytes.len()
                && (bytes[self.pos].is_ascii_alphabetic() || bytes[self.pos] == b'_')
            {
                self.pos += 1;
                while self.pos < bytes.len()
                    && (bytes[self.pos].is_ascii_alphanumeric() || bytes[self.pos] == b'_')
                {
                    self.pos += 1;
                }
            } else {
                return Err(self.syntax("nama param diharapkan setelah '$'"));
            }
            let name = &self.src[start..self.pos];
            return self.params.get(name).copied().ok_or_else(|| {
                let known: Vec<&str> = self.params.keys().map(String::as_str).collect();
                OpError::new(
                    OpErrorCode::UnknownRef,
                    format!("param '{name}' tidak dikenal (param yang ada: {known:?})"),
                )
                .with_context(serde_json::json!({ "param": name, "available": known }))
            });
        }
        let start = self.pos;
        let bytes = self.src.as_bytes();
        while self.pos < bytes.len()
            && (bytes[self.pos].is_ascii_digit() || bytes[self.pos] == b'.')
        {
            self.pos += 1;
        }
        // Eksponen opsional: 1e-3, 2.5E+2.
        if self.pos > start
            && self.pos < bytes.len()
            && (bytes[self.pos] == b'e' || bytes[self.pos] == b'E')
        {
            let save = self.pos;
            self.pos += 1;
            if self.pos < bytes.len() && (bytes[self.pos] == b'+' || bytes[self.pos] == b'-') {
                self.pos += 1;
            }
            let digits = self.pos;
            while self.pos < bytes.len() && bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
            if self.pos == digits {
                self.pos = save;
            }
        }
        if self.pos == start {
            return Err(self.syntax("angka, '$param', atau '(' diharapkan"));
        }
        self.src[start..self.pos]
            .parse::<f64>()
            .map_err(|_| self.syntax("angka tidak valid"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(kv: &[(&str, f64)]) -> Params {
        kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn ex(s: &str) -> Num {
        Num::Expr(s.to_string())
    }

    #[test]
    fn evaluates_param_expression() {
        assert_eq!(
            eval(&ex("$w/2 - 3"), &params(&[("w", 60.0)])).unwrap(),
            27.0
        );
    }

    #[test]
    fn unary_minus_and_parentheses() {
        assert_eq!(eval(&ex("-(2+3)*4"), &Params::new()).unwrap(), -20.0);
        assert_eq!(eval(&ex("2 * -3"), &Params::new()).unwrap(), -6.0);
        assert_eq!(eval(&ex("1.5e1 + 0.5"), &Params::new()).unwrap(), 15.5);
    }

    #[test]
    fn unknown_param_is_unknown_ref() {
        let e = eval(&ex("$x"), &params(&[("w", 1.0)])).unwrap_err();
        assert_eq!(e.code, OpErrorCode::UnknownRef);
        assert!(
            e.message.contains("'x'") && e.message.contains("\"w\""),
            "{}",
            e.message
        );
    }

    #[test]
    fn division_by_zero_and_syntax_are_invalid_param() {
        assert_eq!(
            eval(&ex("1/0"), &Params::new()).unwrap_err().code,
            OpErrorCode::InvalidParam
        );
        assert_eq!(
            eval(&ex("2 +"), &Params::new()).unwrap_err().code,
            OpErrorCode::InvalidParam
        );
        assert_eq!(
            eval(&ex("(1"), &Params::new()).unwrap_err().code,
            OpErrorCode::InvalidParam
        );
        assert_eq!(
            eval(&ex("3 4"), &Params::new()).unwrap_err().code,
            OpErrorCode::InvalidParam
        );
    }

    #[test]
    fn json_shape() {
        assert_eq!(serde_json::to_string(&Num::Value(5.0)).unwrap(), "5.0");
        assert_eq!(serde_json::to_string(&ex("$t")).unwrap(), "\"$t\"");
        let n: Num = serde_json::from_str("8").unwrap();
        assert_eq!(n, Num::Value(8.0));
        let n: Num = serde_json::from_str("\"$t\"").unwrap();
        assert_eq!(n, ex("$t"));
    }
}
