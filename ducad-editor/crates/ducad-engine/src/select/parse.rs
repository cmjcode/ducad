//! Parser selector (recursive-descent, tidak peka huruf besar).
//!
//! ```text
//! selector := term ( ("or" | "and" | "except") term )*   // asosiatif kiri
//! term     := base filter*
//! base     := "all" | "largest" | "smallest" | "longest" | "shortest"
//!           | (">" | "<") AXIS | ("+" | "-") AXIS | "|" AXIS | "#" AXIS
//!           | "idx:" INT ("," INT)* | "of(" selector ")" | "(" selector ")"
//! filter   := "[" KEY CMP VALUE "]"
//! ```

use crate::error::{OpError, OpErrorCode, OpResult};

/// Konteks evaluasi: himpunan face atau himpunan tepi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelCtx {
    Face,
    Edge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub fn index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }

    pub fn unit(self) -> [f64; 3] {
        let mut u = [0.0; 3];
        u[self.index()] = 1.0;
        u
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Base {
    All,
    Largest,
    Smallest,
    Longest,
    Shortest,
    /// `>A` (true) / `<A` (false).
    Extreme(Axis, bool),
    /// `+A` (true) / `-A` (false) — hanya face.
    Facing(Axis, bool),
    Parallel(Axis),
    Perpendicular(Axis),
    Idx(Vec<usize>),
    /// `of(S)` — S dievaluasi dalam konteks face.
    Of(Box<SelExpr>),
    Group(Box<SelExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Kind,
    Area,
    Len,
    R,
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmp {
    Eq,
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(f64),
    Ident(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    pub key: Key,
    pub cmp: Cmp,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOp {
    Or,
    And,
    Except,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelExpr {
    Term {
        base: Base,
        filters: Vec<Filter>,
    },
    Binary {
        op: SetOp,
        lhs: Box<SelExpr>,
        rhs: Box<SelExpr>,
    },
}

pub const FACE_KINDS: [&str; 6] = ["plane", "cylinder", "cone", "sphere", "torus", "other"];
pub const EDGE_KINDS: [&str; 3] = ["line", "circle", "other"];

/// Parse `src` untuk konteks `ctx`. Error → `SelectorSyntax` dengan
/// `context.pos` = indeks karakter penyebab.
pub fn parse(src: &str, ctx: SelCtx) -> OpResult<SelExpr> {
    let chars: Vec<char> = src.chars().collect();
    let mut p = Parser {
        src,
        chars: &chars,
        pos: 0,
    };
    let expr = p.selector(ctx)?;
    p.ws();
    if p.pos < chars.len() {
        return Err(p.err("sisa teks tidak dikenali"));
    }
    Ok(expr)
}

struct Parser<'a> {
    src: &'a str,
    chars: &'a [char],
    pos: usize,
}

impl Parser<'_> {
    fn err(&self, what: &str) -> OpError {
        self.err_at(self.pos, what)
    }

    fn err_at(&self, pos: usize, what: &str) -> OpError {
        OpError::new(
            OpErrorCode::SelectorSyntax,
            format!("selector \"{}\" salah di posisi {pos}: {what}", self.src),
        )
        .with_hint("contoh: \">Z\", \"|Z\", \"of(>Z) and |X\", \"all[kind=cylinder][r=2.75]\"")
        .with_context(serde_json::json!({ "selector": self.src, "pos": pos }))
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).map(|c| c.to_ascii_lowercase())
    }

    fn ws(&mut self) {
        while self.chars.get(self.pos).is_some_and(|c| c.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, c: char) -> bool {
        self.ws();
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Kata kunci tidak peka huruf besar, harus diakhiri non-alfanumerik.
    fn keyword(&mut self, kw: &str) -> bool {
        self.ws();
        let n = kw.chars().count();
        if self.pos + n > self.chars.len() {
            return false;
        }
        let matches = self.chars[self.pos..self.pos + n]
            .iter()
            .zip(kw.chars())
            .all(|(a, b)| a.to_ascii_lowercase() == b);
        let last_alpha = kw.chars().last().is_some_and(|c| c.is_ascii_alphanumeric());
        let boundary = !last_alpha
            || self
                .chars
                .get(self.pos + n)
                .is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == '_'));
        if matches && boundary {
            self.pos += n;
            true
        } else {
            false
        }
    }

    fn selector(&mut self, ctx: SelCtx) -> OpResult<SelExpr> {
        let mut lhs = self.term(ctx)?;
        loop {
            let op = if self.keyword("or") {
                SetOp::Or
            } else if self.keyword("and") {
                SetOp::And
            } else if self.keyword("except") {
                SetOp::Except
            } else {
                return Ok(lhs);
            };
            let rhs = self.term(ctx)?;
            lhs = SelExpr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
    }

    fn term(&mut self, ctx: SelCtx) -> OpResult<SelExpr> {
        let base = self.base(ctx)?;
        let mut filters = Vec::new();
        while self.eat('[') {
            filters.push(self.filter(ctx)?);
        }
        Ok(SelExpr::Term { base, filters })
    }

    fn axis(&mut self) -> OpResult<Axis> {
        // Sumbu harus menempel pada operatornya (">Z", bukan "> Z").
        let a = match self.peek() {
            Some('x') => Axis::X,
            Some('y') => Axis::Y,
            Some('z') => Axis::Z,
            _ => return Err(self.err("sumbu X, Y, atau Z diharapkan")),
        };
        self.pos += 1;
        Ok(a)
    }

    fn base(&mut self, ctx: SelCtx) -> OpResult<Base> {
        self.ws();
        let start = self.pos;
        let only = |p: &Self, want: SelCtx, what: &str| -> OpResult<()> {
            if ctx != want {
                let ctx_name = if ctx == SelCtx::Face { "face" } else { "tepi" };
                return Err(p.err_at(
                    start,
                    &format!("'{what}' tidak berlaku untuk konteks {ctx_name}"),
                ));
            }
            Ok(())
        };
        if self.keyword("all") {
            return Ok(Base::All);
        }
        if self.keyword("largest") {
            only(self, SelCtx::Face, "largest")?;
            return Ok(Base::Largest);
        }
        if self.keyword("smallest") {
            only(self, SelCtx::Face, "smallest")?;
            return Ok(Base::Smallest);
        }
        if self.keyword("longest") {
            only(self, SelCtx::Edge, "longest")?;
            return Ok(Base::Longest);
        }
        if self.keyword("shortest") {
            only(self, SelCtx::Edge, "shortest")?;
            return Ok(Base::Shortest);
        }
        if self.keyword("idx:") {
            let mut list = vec![self.int()?];
            while self.eat(',') {
                list.push(self.int()?);
            }
            return Ok(Base::Idx(list));
        }
        if self.keyword("of") {
            if !self.eat('(') {
                return Err(self.err("'(' diharapkan setelah 'of'"));
            }
            only(self, SelCtx::Edge, "of(...)")?;
            let inner = self.selector(SelCtx::Face)?;
            if !self.eat(')') {
                return Err(self.err("')' diharapkan"));
            }
            return Ok(Base::Of(Box::new(inner)));
        }
        match self.peek() {
            Some('(') => {
                self.pos += 1;
                let inner = self.selector(ctx)?;
                if !self.eat(')') {
                    return Err(self.err("')' diharapkan"));
                }
                Ok(Base::Group(Box::new(inner)))
            }
            Some(c @ ('>' | '<')) => {
                self.pos += 1;
                Ok(Base::Extreme(self.axis()?, c == '>'))
            }
            Some(c @ ('+' | '-')) => {
                self.pos += 1;
                let a = self.axis()?;
                only(self, SelCtx::Face, &format!("{c}sumbu"))?;
                Ok(Base::Facing(a, c == '+'))
            }
            Some('|') => {
                self.pos += 1;
                Ok(Base::Parallel(self.axis()?))
            }
            Some('#') => {
                self.pos += 1;
                Ok(Base::Perpendicular(self.axis()?))
            }
            _ => Err(self.err(
                "awal selector diharapkan (all, largest, >Z, +Z, |Z, #Z, idx:, of(...), '(')",
            )),
        }
    }

    fn int(&mut self) -> OpResult<usize> {
        self.ws();
        let start = self.pos;
        while self.chars.get(self.pos).is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        if start == self.pos {
            return Err(self.err("bilangan bulat diharapkan"));
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        s.parse()
            .map_err(|_| self.err_at(start, "bilangan bulat terlalu besar"))
    }

    fn filter(&mut self, ctx: SelCtx) -> OpResult<Filter> {
        self.ws();
        let key_pos = self.pos;
        let key = if self.keyword("kind") {
            Key::Kind
        } else if self.keyword("area") {
            Key::Area
        } else if self.keyword("len") {
            Key::Len
        } else if self.keyword("r") {
            Key::R
        } else if self.keyword("x") {
            Key::X
        } else if self.keyword("y") {
            Key::Y
        } else if self.keyword("z") {
            Key::Z
        } else {
            return Err(self.err("kunci filter diharapkan (kind, area, len, r, x, y, z)"));
        };
        match (key, ctx) {
            (Key::Area, SelCtx::Edge) => {
                return Err(self.err_at(key_pos, "'area' hanya untuk face"))
            }
            (Key::Len, SelCtx::Face) => return Err(self.err_at(key_pos, "'len' hanya untuk tepi")),
            _ => {}
        }
        self.ws();
        let cmp = if self.eat('<') {
            if self.eat('=') {
                Cmp::Le
            } else {
                Cmp::Lt
            }
        } else if self.eat('>') {
            if self.eat('=') {
                Cmp::Ge
            } else {
                Cmp::Gt
            }
        } else if self.eat('=') {
            Cmp::Eq
        } else {
            return Err(self.err("pembanding diharapkan (=, <, >, <=, >=)"));
        };
        self.ws();
        let val_pos = self.pos;
        let start = self.pos;
        while self
            .chars
            .get(self.pos)
            .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_'))
        {
            self.pos += 1;
        }
        let raw: String = self.chars[start..self.pos].iter().collect();
        if raw.is_empty() {
            return Err(self.err("nilai filter diharapkan"));
        }
        let value = if key == Key::Kind {
            let v = raw.to_ascii_lowercase();
            let allowed: &[&str] = if ctx == SelCtx::Face {
                &FACE_KINDS
            } else {
                &EDGE_KINDS
            };
            if cmp != Cmp::Eq || !allowed.contains(&v.as_str()) {
                return Err(self.err_at(
                    val_pos,
                    &format!("kind harus '=' salah satu dari {allowed:?}"),
                ));
            }
            Value::Ident(v)
        } else {
            match raw.parse::<f64>() {
                Ok(n) if n.is_finite() => Value::Num(n),
                _ => return Err(self.err_at(val_pos, "nilai numerik diharapkan")),
            }
        };
        if !self.eat(']') {
            return Err(self.err("']' diharapkan"));
        }
        Ok(Filter { key, cmp, value })
    }
}
