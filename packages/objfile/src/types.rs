/// Target-independent section identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SectionId(pub u32);

/// A named section and its bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Section {
    /// Stable identifier used by relocations.
    pub id: SectionId,
    /// Format-independent section name.
    pub name: String,
    /// Section payload.
    pub bytes: Vec<u8>,
}

/// A symbol reference used by relocations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SymbolRef {
    /// Index into the object symbol table.
    Local(u32),
    /// Name resolved by the linker or loader.
    External(String),
}

/// Relocation operation independent of an assembler crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelocKind {
    /// Absolute 64-bit address or pointer slot.
    Abs64,
    /// Signed 32-bit PC-relative value.
    PcRel32,
    /// x86-64 PC-relative call through the PLT.
    Plt32,
    /// AArch64 unconditional branch displacement.
    Branch26,
    /// AArch64 page-relative address materialization.
    Adrp21,
    /// AArch64 page offset materialization.
    Add12,
    /// AArch64 conditional branch displacement.
    CondBranch19,
    /// Address of an NCL code entry.
    CodeEntry,
    /// Address resolved by an external loader symbol.
    ExternalSymbol,
}

/// A relocation applied to a section at a byte offset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Relocation {
    /// Section containing the relocation site.
    pub section: SectionId,
    /// Byte offset of the relocation site.
    pub offset: u32,
    /// Target-independent relocation operation.
    pub kind: RelocKind,
    /// Local symbol index or external symbol name.
    pub symbol: SymbolRef,
    /// Addend encoded in the target relocation record.
    pub addend: i64,
}
