/// A symbol or nested list place used by mixed ROTATEF/SHIFTF instructions.
#[derive(Clone, Debug, PartialEq)]
pub enum RotateShiftPlace {
    /// A variable name and its escaping mode.
    Symbol(String, bool),
    /// List accessors, root variable name, and its escaping mode.
    NestedList(Vec<String>, String, bool),
}
