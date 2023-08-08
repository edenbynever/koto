//! The core value type used in the Koto runtime

use {
    crate::{prelude::*, ExternalFunction, Result},
    koto_bytecode::Chunk,
    std::fmt::Write,
};

/// The core Value type for Koto
#[derive(Clone, Debug, Default)]
pub enum Value {
    /// The default type representing the absence of a value
    #[default]
    Null,

    /// A boolean, can be either true or false
    Bool(bool),

    /// A number, represented as either a signed 64 bit integer or float
    Number(ValueNumber),

    /// A range with start/end boundaries
    Range(IntRange),

    /// The list type used in Koto
    List(ValueList),

    /// The tuple type used in Koto
    Tuple(ValueTuple),

    /// The hash map type used in Koto
    Map(ValueMap),

    /// The string type used in Koto
    Str(ValueString),

    /// A callable function with simple properties
    SimpleFunction(SimpleFunctionInfo),

    /// A callable function with less simple properties, e.g. captures, variadic arguments, etc.
    Function(FunctionInfo),

    /// A function that's defined outside of the Koto runtime
    ExternalFunction(ExternalFunction),

    /// A function that produces an Iterator when called
    ///
    /// A [Vm](crate::Vm) gets spawned for the function to run in, which pauses each time a yield
    /// instruction is encountered. See Vm::call_generator and Iterable::Generator.
    Generator(FunctionInfo),

    /// The iterator type used in Koto
    Iterator(ValueIterator),

    /// An object with behaviour defined via the [KotoObject] trait
    Object(Object),

    /// A tuple of values that are packed into a contiguous series of registers
    ///
    /// Used as an optimization when multiple values are passed around without being assigned to a
    /// single Tuple value.
    ///
    /// Note: this is intended for internal use only.
    TemporaryTuple(RegisterSlice),

    /// The builder used while building lists or tuples
    ///
    /// Note: this is intended for internal use only.
    SequenceBuilder(Vec<Value>),

    /// The string builder used during string interpolation
    ///
    /// Note: this is intended for internal use only.
    StringBuilder(StringBuilder),
}

impl Value {
    /// Returns a recursive 'deep copy' of a Value
    ///
    /// This is used by koto.deep_copy.
    pub fn deep_copy(&self) -> RuntimeResult {
        use Value::*;

        let result = match &self {
            List(l) => {
                let result = l
                    .data()
                    .iter()
                    .map(|v| v.deep_copy())
                    .collect::<Result<_>>()?;
                List(ValueList::with_data(result))
            }
            Tuple(t) => {
                let result = t
                    .iter()
                    .map(|v| v.deep_copy())
                    .collect::<Result<Vec<_>>>()?;
                Tuple(result.into())
            }
            Map(m) => {
                let data = m
                    .data()
                    .iter()
                    .map(|(k, v)| v.deep_copy().map(|v| (k.clone(), v)))
                    .collect::<Result<_>>()?;
                let meta = m.meta_map().map(|meta| meta.borrow().clone());
                Map(ValueMap::with_contents(data, meta))
            }
            Iterator(i) => Iterator(i.make_copy()?),
            Object(o) => Object(o.try_borrow()?.copy()),
            _ => self.clone(),
        };

        Ok(result)
    }

    /// Returns true if the value has function-like callable behaviour
    pub fn is_callable(&self) -> bool {
        use Value::*;
        match self {
            SimpleFunction(_) | Function(_) | ExternalFunction(_) => true,
            Map(m) => m.contains_meta_key(&MetaKey::Call),
            _ => false,
        }
    }

    /// Returns true if the value is hashable
    ///
    /// Only hashable values are acceptable as map keys.
    pub fn is_hashable(&self) -> bool {
        use Value::*;
        match self {
            Null | Bool(_) | Number(_) | Range(_) | Str(_) => true,
            Tuple(t) => t.is_hashable(),
            _ => false,
        }
    }

    /// Returns true if a `ValueIterator` can be made from the value
    pub fn is_iterable(&self) -> bool {
        use Value::*;
        match self {
            Range(_) | List(_) | Tuple(_) | Map(_) | Str(_) | Iterator(_) => true,
            Object(o) => o.try_borrow().map_or(false, |o| {
                !matches!(o.is_iterable(), IsIterable::NotIterable)
            }),
            _ => false,
        }
    }

    /// Returns the 'size' of the value
    ///
    /// A value's size is the number of elements that can used in unpacking expressions
    /// e.g.
    /// x = [1, 2, 3] # x has size 3
    /// a, b, c = x
    ///
    /// See:
    ///   - [Op::Size](koto_bytecode::Op::Size)
    ///   - [Op::CheckSizeEqual](koto_bytecode::Op::CheckSizeEqual).
    ///   - [Op::CheckSizeMin](koto_bytecode::Op::CheckSizeMin).
    pub fn size(&self) -> usize {
        use Value::*;

        match &self {
            List(l) => l.len(),
            Tuple(t) => t.len(),
            TemporaryTuple(RegisterSlice { count, .. }) => *count as usize,
            Map(m) => m.len(),
            _ => 1,
        }
    }

    /// Returns the value's type as a ValueString
    pub fn type_as_string(&self) -> ValueString {
        use Value::*;
        match &self {
            Null => TYPE_NULL.with(|x| x.clone()),
            Bool(_) => TYPE_BOOL.with(|x| x.clone()),
            Number(ValueNumber::F64(_)) => TYPE_FLOAT.with(|x| x.clone()),
            Number(ValueNumber::I64(_)) => TYPE_INT.with(|x| x.clone()),
            List(_) => TYPE_LIST.with(|x| x.clone()),
            Range { .. } => TYPE_RANGE.with(|x| x.clone()),
            Map(m) if m.meta_map().is_some() => match m.get_meta_value(&MetaKey::Type) {
                Some(Str(s)) => s,
                Some(_) => "Error: expected string for overloaded type".into(),
                None => TYPE_OBJECT.with(|x| x.clone()),
            },
            Map(_) => TYPE_MAP.with(|x| x.clone()),
            Str(_) => TYPE_STRING.with(|x| x.clone()),
            Tuple(_) => TYPE_TUPLE.with(|x| x.clone()),
            SimpleFunction(_) | Function(_) => TYPE_FUNCTION.with(|x| x.clone()),
            Generator(_) => TYPE_GENERATOR.with(|x| x.clone()),
            ExternalFunction(_) => TYPE_EXTERNAL_FUNCTION.with(|x| x.clone()),
            Object(o) => o.try_borrow().map_or_else(
                |_| "Error: object already borrowed".into(),
                |o| o.object_type(),
            ),
            Iterator(_) => TYPE_ITERATOR.with(|x| x.clone()),
            TemporaryTuple { .. } => TYPE_TEMPORARY_TUPLE.with(|x| x.clone()),
            SequenceBuilder(_) => TYPE_SEQUENCE_BUILDER.with(|x| x.clone()),
            StringBuilder(_) => TYPE_STRING_BUILDER.with(|x| x.clone()),
        }
    }

    /// Returns true if the value is a Map or an External that contains the given meta key
    pub fn contains_meta_key(&self, key: &MetaKey) -> bool {
        use Value::*;
        match &self {
            Map(m) => m.contains_meta_key(key),
            _ => false,
        }
    }

    /// If the value is a Map or an External, returns a clone of the corresponding meta value
    pub fn get_meta_value(&self, key: &MetaKey) -> Option<Value> {
        use Value::*;
        match &self {
            Map(m) => m.get_meta_value(key),
            _ => None,
        }
    }
}

impl KotoDisplay for Value {
    fn display(
        &self,
        out: &mut StringBuilder,
        vm: &mut Vm,
        options: KotoDisplayOptions,
    ) -> Result<()> {
        use Value::*;
        let result = match self {
            Null => out.write_str("null"),
            Bool(b) => write!(out, "{b}"),
            Number(n) => write!(out, "{n}"),
            Range(r) => write!(out, "{r}"),
            SimpleFunction(_) | Function(_) => write!(out, "||"),
            Generator(_) => out.write_str("Generator"),
            Iterator(_) => out.write_str("Iterator"),
            ExternalFunction(_) => out.write_str("||"),
            TemporaryTuple(RegisterSlice { start, count }) => {
                write!(out, "TemporaryTuple [{start}..{}]", start + count)
            }
            SequenceBuilder(_) => out.write_str("SequenceBuilder"),
            StringBuilder(_) => write!(out, "StringBuilder"),
            Str(s) => return s.display(out, vm, options),
            List(l) => return l.display(out, vm, options),
            Tuple(t) => return t.display(out, vm, options),
            Map(m) => return m.display(out, vm, options),
            Object(o) => return o.try_borrow()?.display(out, vm, options),
        };
        if result.is_ok() {
            Ok(())
        } else {
            runtime_error!("Failed to write to string")
        }
    }
}

thread_local! {
    static TYPE_NULL: ValueString = "Null".into();
    static TYPE_BOOL: ValueString = "Bool".into();
    static TYPE_FLOAT: ValueString = "Float".into();
    static TYPE_INT: ValueString = "Int".into();
    static TYPE_LIST: ValueString = "List".into();
    static TYPE_RANGE: ValueString = "Range".into();
    static TYPE_MAP: ValueString = "Map".into();
    static TYPE_OBJECT: ValueString = "Object".into();
    static TYPE_STRING: ValueString = "String".into();
    static TYPE_TUPLE: ValueString = "Tuple".into();
    static TYPE_FUNCTION: ValueString = "Function".into();
    static TYPE_GENERATOR: ValueString = "Generator".into();
    static TYPE_EXTERNAL_FUNCTION: ValueString = "ExternalFunction".into();
    static TYPE_ITERATOR: ValueString = "Iterator".into();
    static TYPE_TEMPORARY_TUPLE: ValueString = "TemporaryTuple".into();
    static TYPE_SEQUENCE_BUILDER: ValueString = "SequenceBuilder".into();
    static TYPE_STRING_BUILDER: ValueString = "StringBuilder".into();
}

impl From<()> for Value {
    fn from(_: ()) -> Self {
        Self::Null
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<ValueNumber> for Value {
    fn from(value: ValueNumber) -> Self {
        Self::Number(value)
    }
}

impl From<IntRange> for Value {
    fn from(value: IntRange) -> Self {
        Self::Range(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Str(value.into())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Str(value.into())
    }
}

impl From<ValueString> for Value {
    fn from(value: ValueString) -> Self {
        Self::Str(value)
    }
}

impl From<ValueList> for Value {
    fn from(value: ValueList) -> Self {
        Self::List(value)
    }
}

impl From<ValueTuple> for Value {
    fn from(value: ValueTuple) -> Self {
        Self::Tuple(value)
    }
}

impl From<ValueMap> for Value {
    fn from(value: ValueMap) -> Self {
        Self::Map(value)
    }
}

impl From<Object> for Value {
    fn from(value: Object) -> Self {
        Self::Object(value)
    }
}

impl From<ValueIterator> for Value {
    fn from(value: ValueIterator) -> Self {
        Self::Iterator(value)
    }
}

/// A plain and simple function
///
/// See also:
/// * [Value::SimpleFunction]
/// * [FunctionInfo]
#[derive(Clone, Debug)]
pub struct SimpleFunctionInfo {
    /// The [Chunk] in which the function can be found.
    pub chunk: Ptr<Chunk>,
    /// The start ip of the function.
    pub ip: usize,
    /// The expected number of arguments for the function
    pub arg_count: u8,
}

/// A fully-featured function with all the bells and whistles
///
/// See also:
/// * [Value::Function]
/// * [SimpleFunctionInfo]
#[derive(Clone, Debug)]
pub struct FunctionInfo {
    /// The [Chunk] in which the function can be found.
    pub chunk: Ptr<Chunk>,
    /// The start ip of the function.
    pub ip: usize,
    /// The expected number of arguments for the function.
    pub arg_count: u8,
    /// If the function is variadic, then extra args will be captured in a tuple.
    pub variadic: bool,
    /// If the function has a single arg, and that arg is an unpacked tuple
    ///
    /// This is used to optimize external calls where the caller has a series of args that might be
    /// unpacked by the function, and it would be wasteful to create a Tuple when it's going to be
    /// immediately unpacked and discarded.
    pub arg_is_unpacked_tuple: bool,
    /// The optional list of captures that should be copied into scope when the function is called.
    //
    // Q. Why use a ValueList?
    // A. Because capturing values currently works by assigning by index, after the function
    //    itself has been created.
    // Q. Why not use a SequenceBuilder?
    // A. Recursive functions need to capture themselves into the list, and the captured function
    //    and the assigned function need to share the same captures list. Currently the only way
    //    for this to work is to allow mutation of the shared list after the creation of the
    //    function, so a ValueList is a reasonable choice.
    // Q. What about using Ptr<[Value]> for non-recursive functions, or Option<Value> for
    //    non-recursive functions with a single capture?
    // A. These could be potential optimizations to investigate at some point, but would involve
    //    placing FunctionInfo behind a Ptr due to its increased size, so it's not clear if there
    //    would be an overall performance win.
    pub captures: Option<ValueList>,
}

/// A slice of a VM's registers
///
/// See [Value::TemporaryTuple]
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegisterSlice {
    pub start: u8,
    pub count: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_mem_size() {
        // All Value variants should have a size of <= 32 bytes, and with the variant flag the
        // total size of Value should not be greater than 40 bytes.
        assert!(std::mem::size_of::<Value>() <= 40);
    }
}
