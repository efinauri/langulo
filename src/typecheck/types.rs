use rusttyc::{Arity, Constructable, ContextSensitiveVariant, Partial, Variant};
use crate::errors::err::LanguloErr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LanguloType {
    Int,
    Float,
    Bool,
    Str,
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
    Repeatable, // int*str
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

            (Addable, Int) | (Int, Addable) => Ok(Addable),
            (Addable, Float) | (Float, Addable) => Ok(Addable),
            (Addable, Str) | (Str, Addable) => Ok(Addable),
            (Addable, Addable) => Ok(Addable),

            (Int, Repeatable) | (Repeatable, Int) => Ok(Repeatable),
            (Int, Str) | (Str, Int) => Ok(Repeatable),

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
            Any
            |Addable
            | Repeatable => Err(LanguloErr::typecheck("Could not identify type before construction".to_string())),
        }
    }
}