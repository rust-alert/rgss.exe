//! Ruby Marshal 4.8 只读子集：覆盖 XP `Scripts.rxdata` 的 Array / String / Fixnum / IVAR。

use std::collections::HashMap;

/// Marshal 解码错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarshalError {
    /// 不是 4.8。
    BadHeader,
    /// 未支持的类型字节。
    UnsupportedType {
        /// 类型码。
        tag: u8,
        /// 偏移。
        offset: usize,
    },
    /// 截断。
    Truncated {
        /// 偏移。
        offset: usize,
    },
    /// 符号 / 对象链接越界。
    BadLink,
}

impl std::fmt::Display for MarshalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadHeader => f.write_str("rgss.marshal.bad_header"),
            Self::UnsupportedType { .. } => f.write_str("rgss.marshal.unsupported_type"),
            Self::Truncated { .. } => f.write_str("rgss.marshal.truncated"),
            Self::BadLink => f.write_str("rgss.marshal.bad_link"),
        }
    }
}

impl std::error::Error for MarshalError {}

/// Marshal 值。
#[derive(Debug, Clone)]
pub enum MarshalValue {
    Nil,
    Bool(bool),
    Fixnum(i64),
    String(Vec<u8>),
    Symbol(String),
    Array(Vec<MarshalValue>),
    Hash(HashMap<String, MarshalValue>),
}

impl MarshalValue {
    /// 当作字节串。
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::String(b) => Some(b),
            _ => None,
        }
    }

    /// 当作数组。
    pub fn as_array(&self) -> Option<&[MarshalValue]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// 当作整数。
    pub fn as_fixnum(&self) -> Option<i64> {
        match self {
            Self::Fixnum(n) => Some(*n),
            _ => None,
        }
    }
}

/// 解码 Marshal 4.8 文档。
pub fn load(bytes: &[u8]) -> Result<MarshalValue, MarshalError> {
    if bytes.len() < 2 || bytes[0] != 4 || bytes[1] != 8 {
        return Err(MarshalError::BadHeader);
    }
    let mut r = Reader {
        data: bytes,
        i: 2,
        symbols: Vec::new(),
        objects: Vec::new(),
    };
    r.read_value()
}

struct Reader<'a> {
    data: &'a [u8],
    i: usize,
    symbols: Vec<String>,
    objects: Vec<MarshalValue>,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Result<u8, MarshalError> {
        let b = *self
            .data
            .get(self.i)
            .ok_or(MarshalError::Truncated { offset: self.i })?;
        self.i += 1;
        Ok(b)
    }

    fn s8(&mut self) -> Result<i8, MarshalError> {
        Ok(self.u8()? as i8)
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8], MarshalError> {
        let end = self
            .i
            .checked_add(n)
            .ok_or(MarshalError::Truncated { offset: self.i })?;
        let slice = self
            .data
            .get(self.i..end)
            .ok_or(MarshalError::Truncated { offset: self.i })?;
        self.i = end;
        Ok(slice)
    }

    fn long(&mut self) -> Result<i64, MarshalError> {
        let n = i32::from(self.s8()?);
        if n == 0 {
            return Ok(0);
        }
        if (5..=127).contains(&n) {
            return Ok(i64::from(n - 5));
        }
        if (-128..=-5).contains(&n) {
            return Ok(i64::from(n + 5));
        }
        let size = n.unsigned_abs() as usize;
        let mut x: u32 = 0;
        for k in 0..size {
            x |= u32::from(self.u8()?) << (8 * k);
        }
        if n > 0 {
            Ok(i64::from(x))
        } else if size < 4 {
            let sign_bit = 1u32 << (8 * size - 1);
            let full = 1u32 << (8 * size);
            if x & sign_bit != 0 {
                Ok(i64::from(x) - i64::from(full))
            } else {
                Ok(i64::from(x))
            }
        } else {
            Ok(i64::from(x as i32))
        }
    }

    fn read_value(&mut self) -> Result<MarshalValue, MarshalError> {
        let tag = self.u8()?;
        match tag {
            b'0' => Ok(MarshalValue::Nil),
            b'T' => Ok(MarshalValue::Bool(true)),
            b'F' => Ok(MarshalValue::Bool(false)),
            b'i' => Ok(MarshalValue::Fixnum(self.long()?)),
            b':' => {
                let n = self.long()? as usize;
                let s = String::from_utf8_lossy(self.bytes(n)?).into_owned();
                self.symbols.push(s.clone());
                Ok(MarshalValue::Symbol(s))
            }
            b';' => {
                let idx = self.long()? as usize;
                self.symbols
                    .get(idx)
                    .cloned()
                    .map(MarshalValue::Symbol)
                    .ok_or(MarshalError::BadLink)
            }
            b'"' => {
                let n = self.long()? as usize;
                let s = self.bytes(n)?.to_vec();
                let v = MarshalValue::String(s);
                self.objects.push(v.clone());
                Ok(v)
            }
            b'[' => {
                let n = self.long()? as usize;
                let idx = self.objects.len();
                self.objects.push(MarshalValue::Array(Vec::new()));
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(self.read_value()?);
                }
                let v = MarshalValue::Array(items);
                self.objects[idx] = v.clone();
                Ok(v)
            }
            b'{' => {
                let n = self.long()? as usize;
                let idx = self.objects.len();
                self.objects.push(MarshalValue::Hash(HashMap::new()));
                let mut map = HashMap::new();
                for _ in 0..n {
                    let key = self.read_key()?;
                    let val = self.read_value()?;
                    map.insert(key, val);
                }
                let v = MarshalValue::Hash(map);
                self.objects[idx] = v.clone();
                Ok(v)
            }
            b'I' => {
                let obj = self.read_value()?;
                let n = self.long()? as usize;
                for _ in 0..n {
                    let _ = self.read_value()?;
                    let _ = self.read_value()?;
                }
                Ok(obj)
            }
            b'@' => {
                let idx = self.long()? as usize;
                self.objects.get(idx).cloned().ok_or(MarshalError::BadLink)
            }
            _ => Err(MarshalError::UnsupportedType {
                tag,
                offset: self.i.saturating_sub(1),
            }),
        }
    }

    fn read_key(&mut self) -> Result<String, MarshalError> {
        match self.read_value()? {
            MarshalValue::Symbol(s) => Ok(s),
            MarshalValue::String(b) => Ok(String::from_utf8_lossy(&b).into_owned()),
            MarshalValue::Fixnum(n) => Ok(n.to_string()),
            other => Ok(format!("{other:?}")),
        }
    }
}
