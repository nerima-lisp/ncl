/// Target-independent section identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SectionId(pub u32);

/// A named section and its bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Section {
    pub id: SectionId,
    pub name: String,
    pub bytes: Vec<u8>,
}

/// A symbol reference used by relocations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SymbolRef {
    Local(u32),
    External(String),
}

/// Relocation operation independent of an assembler crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelocKind {
    Abs64,
    PcRel32,
    /// x86-64 PC-relative call through the PLT.
    Plt32,
    Branch26,
    Adrp21,
    Add12,
    CondBranch19,
    CodeEntry,
    ExternalSymbol,
}

/// A relocation applied to a section at a byte offset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Relocation {
    pub section: SectionId,
    pub offset: u32,
    pub kind: RelocKind,
    pub symbol: SymbolRef,
    pub addend: i64,
}
