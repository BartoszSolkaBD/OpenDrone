//! Which crate a function in compiled code comes from, read from its symbol
//! name.
//!
//! A generic or inline function is compiled into the crate that uses it, so
//! the game's compiled code can hold a core library's function. Rust's symbol
//! names (the "v0" scheme, Rust's default) say whose function each one is:
//! the crate whose code it is, with a number that tells apart two crates of
//! the same name, such as two versions of glam. For a method in an `impl`
//! block, that's the crate the `impl` is written in; for a trait's own
//! default method, the trait's crate.
//!
//! The scheme is in Rust's documentation:
//! <https://doc.rust-lang.org/rustc/symbol-mangling/v0.html>.

/// The crate a function comes from.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Owner {
    /// The crate's name as Rust code writes it, such as `parry3d_f64`.
    pub name: String,
    /// The number that tells apart crates of the same name. Older-style
    /// symbol names don't carry one.
    pub disambiguator: Option<u64>,
}

/// The crate the function or data named by `symbol` comes from, if the name
/// is a Rust one this can read.
pub(crate) fn owner(symbol: &str) -> Option<Owner> {
    if let Some(rest) = symbol.strip_prefix("_R") {
        let mut parser = Parser {
            symbol: rest.as_bytes(),
            next: 0,
            depth: 0,
        };
        return parser.owner().ok();
    }
    // The older scheme writes the item's path, such as
    // `glamx::eigen::h0123…`, with no number to tell crates apart, and an
    // `impl`'s methods as `<Type as Trait>::method`, which doesn't say where
    // the `impl` is written.
    if symbol.starts_with("_ZN") {
        let path = format!("{:#}", rustc_demangle::demangle(symbol));
        let (name, _) = path.split_once("::")?;
        if name.starts_with('<') {
            return None;
        }
        return Some(Owner {
            name: name.to_owned(),
            disambiguator: None,
        });
    }
    None
}

/// Reads a v0 symbol name after its `_R`. Each `skip_*` reads past one part
/// of the name; `owner` follows the parts that lead to the owning crate.
struct Parser<'a> {
    symbol: &'a [u8],
    next: usize,
    depth: u32,
}

/// Something in the name this parser can't read.
struct Unreadable;

type Read<T = ()> = Result<T, Unreadable>;

impl Parser<'_> {
    fn owner(&mut self) -> Read<Owner> {
        self.deeper()?;
        let owner = match self.byte()? {
            // The crate itself.
            b'C' => {
                let disambiguator = self.disambiguator()?;
                let name = self.identifier()?;
                Owner {
                    name: String::from_utf8_lossy(name).into_owned(),
                    disambiguator: Some(disambiguator),
                }
            }
            // `path::name`, or a closure or other item inside `path`.
            b'N' => {
                self.byte()?;
                self.owner()?
            }
            // An `impl` block's item: the crate the block is written in.
            b'M' | b'X' => {
                self.disambiguator()?;
                self.owner()?
            }
            // A trait's own item, `<Type as Trait>::item`: the trait's crate.
            b'Y' => {
                self.skip_type()?;
                self.owner()?
            }
            // `path::<generic arguments>`.
            b'I' => self.owner()?,
            // A part already written earlier in the name.
            b'B' => self.back_reference()?.owner()?,
            _ => return Err(Unreadable),
        };
        self.depth -= 1;
        Ok(owner)
    }

    fn skip_path(&mut self) -> Read {
        self.deeper()?;
        match self.byte()? {
            b'C' => {
                self.disambiguator()?;
                self.identifier()?;
            }
            b'N' => {
                self.byte()?;
                self.skip_path()?;
                self.disambiguator()?;
                self.identifier()?;
            }
            b'M' => {
                self.disambiguator()?;
                self.skip_path()?;
                self.skip_type()?;
            }
            b'X' => {
                self.disambiguator()?;
                self.skip_path()?;
                self.skip_type()?;
                self.skip_path()?;
            }
            b'Y' => {
                self.skip_type()?;
                self.skip_path()?;
            }
            b'I' => {
                self.skip_path()?;
                while !self.eat(b'E') {
                    self.skip_generic_argument()?;
                }
            }
            b'B' => {
                self.base_62()?;
            }
            _ => return Err(Unreadable),
        }
        self.depth -= 1;
        Ok(())
    }

    fn skip_generic_argument(&mut self) -> Read {
        if self.eat(b'L') {
            self.base_62().map(drop)
        } else if self.eat(b'K') {
            self.skip_const()
        } else {
            self.skip_type()
        }
    }

    fn skip_type(&mut self) -> Read {
        self.deeper()?;
        self.eat(b'w');
        match self.byte()? {
            // Built-in types, such as `f64` (`d`) and `usize` (`j`).
            b'a' | b'b' | b'c' | b'd' | b'e' | b'f' | b'h' | b'i' | b'j' | b'l' | b'm' | b'n'
            | b'o' | b'p' | b's' | b't' | b'u' | b'v' | b'x' | b'y' | b'z' => {}
            // References.
            b'R' | b'Q' => {
                if self.eat(b'L') {
                    self.base_62()?;
                }
                self.skip_type()?;
            }
            // Pointers and slices.
            b'P' | b'O' | b'S' => self.skip_type()?,
            // Arrays.
            b'A' => {
                self.skip_type()?;
                self.skip_const()?;
            }
            // Tuples.
            b'T' => {
                while !self.eat(b'E') {
                    self.skip_type()?;
                }
            }
            // Function pointers.
            b'F' => {
                if self.eat(b'G') {
                    self.base_62()?;
                }
                self.eat(b'U');
                if self.eat(b'K') && !self.eat(b'C') {
                    self.identifier()?;
                }
                while !self.eat(b'E') {
                    self.skip_type()?;
                }
                self.skip_type()?;
            }
            // Trait objects.
            b'D' => {
                if self.eat(b'G') {
                    self.base_62()?;
                }
                while !self.eat(b'E') {
                    self.skip_dyn_trait()?;
                }
                if !self.eat(b'L') {
                    return Err(Unreadable);
                }
                self.base_62()?;
            }
            b'B' => {
                self.base_62()?;
            }
            // Pattern types.
            b'W' => {
                self.skip_type()?;
                self.skip_pattern()?;
            }
            // A named type.
            _ => {
                self.next -= 1;
                self.skip_path()?;
            }
        }
        self.depth -= 1;
        Ok(())
    }

    fn skip_dyn_trait(&mut self) -> Read {
        if self.eat(b'B') {
            self.base_62()?;
        } else if self.eat(b'I') {
            self.skip_path()?;
            while !self.eat(b'E') {
                self.skip_generic_argument()?;
            }
        } else {
            self.skip_path()?;
        }
        while self.eat(b'p') {
            self.identifier()?;
            if self.eat(b'K') {
                self.skip_const()?;
            } else {
                self.skip_type()?;
            }
        }
        Ok(())
    }

    fn skip_const(&mut self) -> Read {
        self.deeper()?;
        let tag = self.byte()?;
        match tag {
            b'p' => {}
            b'h' | b't' | b'm' | b'y' | b'o' | b'j' | b'b' | b'c' | b'e' => self.skip_hex()?,
            b'a' | b's' | b'l' | b'x' | b'n' | b'i' => {
                self.eat(b'n');
                self.skip_hex()?;
            }
            b'R' | b'Q' => {
                if tag == b'R' && self.eat(b'e') {
                    self.skip_hex()?;
                } else {
                    self.skip_const()?;
                }
            }
            b'A' | b'T' => {
                while !self.eat(b'E') {
                    self.skip_const()?;
                }
            }
            b'V' => {
                self.skip_path()?;
                match self.byte()? {
                    b'U' => {}
                    b'T' => {
                        while !self.eat(b'E') {
                            self.skip_const()?;
                        }
                    }
                    b'S' => {
                        while !self.eat(b'E') {
                            self.disambiguator()?;
                            self.identifier()?;
                            self.skip_const()?;
                        }
                    }
                    _ => return Err(Unreadable),
                }
            }
            b'B' => {
                self.base_62()?;
            }
            _ => return Err(Unreadable),
        }
        self.depth -= 1;
        Ok(())
    }

    fn skip_pattern(&mut self) -> Read {
        self.deeper()?;
        match self.byte()? {
            b'R' => {
                self.skip_const()?;
                self.skip_const()?;
            }
            b'O' => {
                self.skip_pattern()?;
                while !self.eat(b'E') {
                    self.skip_pattern()?;
                }
            }
            b'N' => {}
            _ => return Err(Unreadable),
        }
        self.depth -= 1;
        Ok(())
    }

    /// A constant's value: hex digits ended by `_`.
    fn skip_hex(&mut self) -> Read {
        loop {
            match self.byte()? {
                b'0'..=b'9' | b'a'..=b'f' => {}
                b'_' => return Ok(()),
                _ => return Err(Unreadable),
            }
        }
    }

    /// Guards against a name nested deep enough to exhaust the stack.
    fn deeper(&mut self) -> Read {
        self.depth += 1;
        if self.depth > 500 {
            return Err(Unreadable);
        }
        Ok(())
    }

    fn byte(&mut self) -> Read<u8> {
        let byte = *self.symbol.get(self.next).ok_or(Unreadable)?;
        self.next += 1;
        Ok(byte)
    }

    fn eat(&mut self, byte: u8) -> bool {
        let found = self.symbol.get(self.next) == Some(&byte);
        if found {
            self.next += 1;
        }
        found
    }

    /// A number written in base 62 and ended by `_`, where `_` alone is 0
    /// and the digits stand for the number plus one.
    fn base_62(&mut self) -> Read<u64> {
        if self.eat(b'_') {
            return Ok(0);
        }
        let mut number: u64 = 0;
        while !self.eat(b'_') {
            let digit = match self.byte()? {
                digit @ b'0'..=b'9' => digit - b'0',
                digit @ b'a'..=b'z' => 10 + digit - b'a',
                digit @ b'A'..=b'Z' => 36 + digit - b'A',
                _ => return Err(Unreadable),
            };
            number = number
                .checked_mul(62)
                .and_then(|number| number.checked_add(u64::from(digit)))
                .ok_or(Unreadable)?;
        }
        number.checked_add(1).ok_or(Unreadable)
    }

    fn disambiguator(&mut self) -> Read<u64> {
        if !self.eat(b's') {
            return Ok(0);
        }
        self.base_62()?.checked_add(1).ok_or(Unreadable)
    }

    /// A name: its length in decimal (0 alone for an empty name, such as a
    /// closure's), an optional `_`, then the name.
    fn identifier(&mut self) -> Read<&[u8]> {
        self.eat(b'u');
        let mut length = match self.byte()? {
            digit @ b'0'..=b'9' => usize::from(digit - b'0'),
            _ => return Err(Unreadable),
        };
        if length != 0 {
            while let Some(digit @ b'0'..=b'9') = self.symbol.get(self.next).copied() {
                self.next += 1;
                length = length
                    .checked_mul(10)
                    .and_then(|length| length.checked_add(usize::from(digit - b'0')))
                    .ok_or(Unreadable)?;
            }
        }
        self.eat(b'_');
        let end = self.next.checked_add(length).ok_or(Unreadable)?;
        let name = self.symbol.get(self.next..end).ok_or(Unreadable)?;
        self.next = end;
        Ok(name)
    }

    /// A parser at an earlier part of the name, which this one repeats.
    fn back_reference(&mut self) -> Read<Parser<'_>> {
        let here = self.next - 1;
        let at = usize::try_from(self.base_62()?).map_err(|_| Unreadable)?;
        if at >= here {
            return Err(Unreadable);
        }
        Ok(Parser {
            symbol: self.symbol,
            next: at,
            depth: self.depth,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::owner;

    fn name(symbol: &str) -> Option<String> {
        owner(symbol).map(|owner| owner.name)
    }

    #[test]
    fn a_generic_function_compiled_into_the_game_belongs_to_the_crate_that_wrote_it() {
        // glamx::f::<f64>, compiled into the game (opendrone).
        let symbol = "_RINvCsj7HfDmZ2x31_5glamx1fdECsiuLsfcr6GEd_9opendrone";
        assert_eq!(name(symbol).as_deref(), Some("glamx"));
    }

    #[test]
    fn std_s_float_method_compiled_into_the_game_belongs_to_std() {
        // <f64>::acos, compiled into the game.
        let symbol = "_RNvMNtCsgHYyB3hU8Ec_3std3f64d4acosCsiuLsfcr6GEd_9opendrone";
        assert_eq!(name(symbol).as_deref(), Some("std"));
    }

    #[test]
    fn a_trait_impl_s_method_belongs_to_the_crate_the_impl_is_written_in() {
        // <f64 as core::convert::Into<f64>>::into: core writes that impl.
        let symbol =
            "_RNvXs1_NtCsevLNFiNqfJP_4core7convertdINtB5_4IntodE4intoCsiuLsfcr6GEd_9opendrone";
        assert_eq!(name(symbol).as_deref(), Some("core"));
    }

    #[test]
    fn two_crates_of_the_same_name_are_told_apart() {
        let one = owner("_RNvCs6XYssbys4uA_4glam4wave").expect("readable");
        let other = owner("_RNvCsj7HfDmZ2x31_4glam4wave").expect("readable");
        assert_eq!(one.name, other.name);
        assert_ne!(one.disambiguator, other.disambiguator);
    }

    #[test]
    fn a_trait_s_own_method_belongs_to_the_trait_s_crate() {
        // <glamx::Shape as parry::Query>::cast, a default method parry writes:
        // Y, the type (a path in glamx), then the trait (a path in parry).
        let symbol = "_RNvYNtCs1_5glamx5ShapeNtCs2_5parry5Query4cast";
        assert_eq!(name(symbol).as_deref(), Some("parry"));
    }

    #[test]
    fn an_older_style_name_gives_the_crate_without_its_number() {
        let found = owner("_ZN5glamx5eigen17h0123456789abcdefE").expect("readable");
        assert_eq!(found.name, "glamx");
        assert_eq!(found.disambiguator, None);
    }

    #[test]
    fn a_c_name_has_no_crate() {
        assert_eq!(owner("acos"), None);
        assert_eq!(owner("_RZZ"), None);
    }
}
