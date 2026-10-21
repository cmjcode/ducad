//! Parser input panjang dengan satuan dan aritmetika (lihat
//! [`crate::LengthUnit::parse_input_mm`]).
//!
//! Tata bahasa:
//! ```text
//! expr   := term (("+"|"-") term)*
//! term   := unary (("*"|"/") unary)*
//! unary  := "-" unary | "+" unary | atom
//! atom   := NUMBER UNIT? | "(" expr ")"
//! UNIT   := mm | cm | m | in | inch | " | ft
//! ```
//! Angka tanpa satuan adalah skalar; skalar yang berdiri sebagai panjang
//! (hasil akhir, atau dijumlah dengan panjang) dibaca dalam satuan aktif.

#[derive(Debug, Clone, Copy, PartialEq)]
enum Q {
    /// Angka tanpa satuan.
    Scalar(f64),
    /// Panjang dalam mm.
    Len(f64),
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
    /// Faktor satuan aktif → mm.
    active: f64,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }

    fn as_len(&self, q: Q) -> f64 {
        match q {
            Q::Scalar(v) => v * self.active,
            Q::Len(v) => v,
        }
    }

    fn expr(&mut self) -> Result<Q, String> {
        let mut acc = self.term()?;
        while let Some(op @ (b'+' | b'-')) = self.peek() {
            self.i += 1;
            let rhs = self.term()?;
            let sign = if op == b'+' { 1.0 } else { -1.0 };
            acc = match (acc, rhs) {
                (Q::Scalar(a), Q::Scalar(b)) => Q::Scalar(a + sign * b),
                (a, b) => Q::Len(self.as_len(a) + sign * self.as_len(b)),
            };
        }
        Ok(acc)
    }

    fn term(&mut self) -> Result<Q, String> {
        let mut acc = self.unary()?;
        while let Some(op @ (b'*' | b'/')) = self.peek() {
            self.i += 1;
            let rhs = self.unary()?;
            acc = if op == b'*' {
                match (acc, rhs) {
                    (Q::Scalar(a), Q::Scalar(b)) => Q::Scalar(a * b),
                    (Q::Scalar(a), Q::Len(b)) | (Q::Len(b), Q::Scalar(a)) => Q::Len(a * b),
                    (Q::Len(_), Q::Len(_)) => {
                        return Err("perkalian dua panjang bukan panjang".into())
                    }
                }
            } else {
                let divisor = match rhs {
                    Q::Scalar(b) | Q::Len(b) => b,
                };
                if divisor == 0.0 {
                    return Err("pembagian dengan nol".into());
                }
                match (acc, rhs) {
                    (Q::Scalar(a), Q::Scalar(b)) => Q::Scalar(a / b),
                    (Q::Len(a), Q::Scalar(b)) => Q::Len(a / b),
                    (Q::Len(a), Q::Len(b)) => Q::Scalar(a / b),
                    (Q::Scalar(_), Q::Len(_)) => {
                        return Err("angka dibagi panjang bukan panjang".into())
                    }
                }
            };
        }
        Ok(acc)
    }

    fn unary(&mut self) -> Result<Q, String> {
        match self.peek() {
            Some(b'-') => {
                self.i += 1;
                Ok(match self.unary()? {
                    Q::Scalar(v) => Q::Scalar(-v),
                    Q::Len(v) => Q::Len(-v),
                })
            }
            Some(b'+') => {
                self.i += 1;
                self.unary()
            }
            _ => self.atom(),
        }
    }

    fn atom(&mut self) -> Result<Q, String> {
        if self.peek() == Some(b'(') {
            self.i += 1;
            let v = self.expr()?;
            if self.peek() != Some(b')') {
                return Err("kurung tutup ')' hilang".into());
            }
            self.i += 1;
            return Ok(v);
        }
        self.ws();
        let start = self.i;
        while self.i < self.s.len()
            && (self.s[self.i].is_ascii_digit() || matches!(self.s[self.i], b'.' | b','))
        {
            self.i += 1;
        }
        if start == self.i {
            return Err(match self.s.get(self.i) {
                Some(c) => format!("karakter '{}' tidak diharapkan", *c as char),
                None => "angka diharapkan".into(),
            });
        }
        let raw = String::from_utf8_lossy(&self.s[start..self.i]).replace(',', ".");
        let v: f64 = raw
            .parse()
            .map_err(|_| format!("'{raw}' bukan angka yang valid"))?;
        self.ws();
        if self.s.get(self.i) == Some(&b'"') {
            self.i += 1;
            return Ok(Q::Len(v * 25.4));
        }
        let ustart = self.i;
        while self.i < self.s.len() && self.s[self.i].is_ascii_alphabetic() {
            self.i += 1;
        }
        let unit = String::from_utf8_lossy(&self.s[ustart..self.i]).to_ascii_lowercase();
        Ok(match unit.as_str() {
            "" => Q::Scalar(v),
            "mm" => Q::Len(v),
            "cm" => Q::Len(v * 10.0),
            "m" => Q::Len(v * 1000.0),
            "in" | "inch" => Q::Len(v * 25.4),
            "ft" => Q::Len(v * 304.8),
            other => {
                return Err(format!(
                    "satuan '{other}' tidak dikenal (mm, cm, m, in, ft)"
                ))
            }
        })
    }
}

/// Parse `text` → mm; angka tanpa satuan memakai faktor `active_to_mm`.
pub(crate) fn parse(text: &str, active_to_mm: f64) -> Result<f64, String> {
    let mut p = P {
        s: text.trim().as_bytes(),
        i: 0,
        active: active_to_mm,
    };
    if p.s.is_empty() {
        return Err("nilai kosong".into());
    }
    let q = p.expr()?;
    if p.peek().is_some() {
        return Err(format!(
            "sisa teks tidak dikenali: '{}'",
            String::from_utf8_lossy(&p.s[p.i..])
        ));
    }
    Ok(p.as_len(q))
}

#[cfg(test)]
mod tests {
    use crate::LengthUnit;

    fn mm(u: LengthUnit, s: &str) -> f64 {
        u.parse_input_mm(s).unwrap_or_else(|e| panic!("{s}: {e}"))
    }

    #[test]
    fn plain_numbers_use_the_active_unit() {
        assert_eq!(mm(LengthUnit::Millimeters, "12.5"), 12.5);
        assert_eq!(mm(LengthUnit::Millimeters, " 12,5 "), 12.5);
        assert_eq!(mm(LengthUnit::Centimeters, "3"), 30.0);
        assert!((mm(LengthUnit::Inches, "1") - 25.4).abs() < 1e-12);
    }

    #[test]
    fn explicit_units_override_the_active_unit() {
        assert_eq!(mm(LengthUnit::Centimeters, "5 mm"), 5.0);
        assert!((mm(LengthUnit::Millimeters, "1in") - 25.4).abs() < 1e-12);
        assert!((mm(LengthUnit::Millimeters, "2\"") - 50.8).abs() < 1e-12);
        assert_eq!(mm(LengthUnit::Millimeters, "0.5 m"), 500.0);
        assert!((mm(LengthUnit::Millimeters, "1 ft") - 304.8).abs() < 1e-9);
    }

    #[test]
    fn arithmetic_mixes_scalars_and_lengths() {
        assert_eq!(mm(LengthUnit::Millimeters, "25.4/2"), 12.7);
        assert!((mm(LengthUnit::Millimeters, "1 in + 3 mm") - 28.4).abs() < 1e-12);
        assert_eq!(mm(LengthUnit::Millimeters, "2*(10+5)"), 30.0);
        assert_eq!(mm(LengthUnit::Millimeters, "2 * 10mm"), 20.0);
        assert_eq!(
            mm(LengthUnit::Centimeters, "1 + 5mm"),
            15.0,
            "skalar + panjang"
        );
        assert_eq!(mm(LengthUnit::Millimeters, "-3"), -3.0);
        assert_eq!(
            mm(LengthUnit::Millimeters, "40mm / 4mm"),
            10.0,
            "rasio -> skalar -> satuan aktif"
        );
    }

    #[test]
    fn invalid_input_gives_a_reason_not_a_default() {
        for bad in [
            "",
            "abc",
            "5 km",
            "2 * 3mm * 4mm",
            "10 /0",
            "(3+4",
            "3 4",
            "1..2",
        ] {
            let r = LengthUnit::Millimeters.parse_input_mm(bad);
            assert!(r.is_err(), "'{bad}' seharusnya ditolak: {r:?}");
        }
    }
}
