/// A symbol-rooted place supported by native parallel `PSETF`.
#[derive(Clone, Debug, PartialEq)]
pub enum PsetfPlace {
    /// A variable place and its escaped-name flag.
    Symbol(String, bool),
    /// A list accessor chain, variable name, and escaped-name flag.
    List(Vec<String>, String, bool),
}
