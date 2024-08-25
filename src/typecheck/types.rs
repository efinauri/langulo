use rusttyc::{Arity, Constructable, ContextSensitiveVariant, Partial, Variant};
use crate::errors::err::LanguloErr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LanguloType {
    Int,
    Float,
    Bool,
    Str,
    Char,
    Option(Box<LanguloType>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LanguloVariant {
    //concrete types
    Int,
    Float,
    Bool,
    Str,
    // utility types for type inference
    Any,
    Addable, // int+int, flt+flt, str+str
    Multipliable,
    Char,
}

struct Variable(usize);


impl Variant for LanguloVariant {
    type Err = LanguloErr;

    fn top() -> Self { LanguloVariant::Any }

    fn meet(lhs: Partial<Self>, rhs: Partial<Self>) -> Result<Partial<Self>, Self::Err> {
        assert_eq!(lhs.least_arity, 0, "spurious child");
        assert_eq!(rhs.least_arity, 0, "spurious child");

        use LanguloVariant::*;
        let err = format!("Incompatible types {:?} and {:?}", &lhs.variant, &rhs.variant);
        let variant = match (lhs.variant, rhs.variant) {
            (Any, other) | (other, Any) => Ok(other),

            (Int, Int) => Ok(Int),
            (Float, Float) => Ok(Float),
            (Str, Str) => Ok(Str),
            (Char, Char) => Ok(Char),

            (Addable, Int) | (Int, Addable) => Ok(Int),
            (Addable, Float) | (Float, Addable) => Ok(Float),
            (Addable, Str) | (Str, Addable) => Ok(Str),
            (Addable, Addable) => Ok(Addable),

            (Multipliable, Int) | (Int, Multipliable) => Ok(Int),
            (Multipliable, Float) | (Float, Multipliable) => Ok(Float),
            (Multipliable, Multipliable) => Ok(Multipliable),

            _ => Err(LanguloErr::typecheck(err))
        }?;
        Ok(Partial { variant, least_arity: 0 })
    }

    fn arity(&self) -> Arity { Arity::Fixed(0) }
}

impl Constructable for LanguloVariant {
    type Type = LanguloType;

    fn construct(&self, children: &[Self::Type]) -> Result<Self::Type, <Self as ContextSensitiveVariant>::Err> {
        assert!(children.is_empty(), "spurious children");
        use LanguloVariant::*;
        match self {
            Int => Ok(LanguloType::Int),
            Float => Ok(LanguloType::Float),
            Bool => Ok(LanguloType::Bool),
            Str => Ok(LanguloType::Str),
            Char => Ok(LanguloType::Char),
            Any
            | Addable
            | Multipliable => Err(LanguloErr::typecheck("Could not identify type before construction".to_string())),
        }
    }
}