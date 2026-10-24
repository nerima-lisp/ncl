#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum BuiltinPackage {
    CommonLisp,
    AsdfInterface,
    UiopDriver,
    NclThreads,
    NclFfi,
    NclMop,
    NclGray,
    NclGc,
    NclImage,
    NclUnicode,
    NclOs,
    NclExt,
    NclSys,
    NclTest,
    SbExt,
    SbThread,
    SbAlien,
    SbSys,
    SbMop,
    SbGray,
    SbUnicode,
    SbDebug,
    SbAlienInternals,
    SbC,
    SbImpl,
    SbInt,
    SbKernel,
    SbPcl,
    SbUnix,
    SbVm,
}

impl BuiltinPackage {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CommonLisp => "COMMON-LISP",
            Self::AsdfInterface => "ASDF/INTERFACE",
            Self::UiopDriver => "UIOP/DRIVER",
            Self::NclThreads => "NCL-THREADS",
            Self::NclFfi => "NCL-FFI",
            Self::NclMop => "NCL-MOP",
            Self::NclGray => "NCL-GRAY",
            Self::NclGc => "NCL-GC",
            Self::NclImage => "NCL-IMAGE",
            Self::NclUnicode => "NCL-UNICODE",
            Self::NclOs => "NCL-OS",
            Self::NclExt => "NCL-EXT",
            Self::NclSys => "NCL-SYS",
            Self::NclTest => "NCL-TEST",
            Self::SbExt => "SB-EXT",
            Self::SbThread => "SB-THREAD",
            Self::SbAlien => "SB-ALIEN",
            Self::SbSys => "SB-SYS",
            Self::SbMop => "SB-MOP",
            Self::SbGray => "SB-GRAY",
            Self::SbUnicode => "SB-UNICODE",
            Self::SbDebug => "SB-DEBUG",
            Self::SbAlienInternals => "SB-ALIEN-INTERNALS",
            Self::SbC => "SB-C",
            Self::SbImpl => "SB-IMPL",
            Self::SbInt => "SB-INT",
            Self::SbKernel => "SB-KERNEL",
            Self::SbPcl => "SB-PCL",
            Self::SbUnix => "SB-UNIX",
            Self::SbVm => "SB-VM",
        }
    }

    pub(crate) const fn is_ncl(self) -> bool {
        matches!(
            self,
            Self::NclThreads
                | Self::NclFfi
                | Self::NclMop
                | Self::NclGray
                | Self::NclGc
                | Self::NclImage
                | Self::NclUnicode
                | Self::NclOs
                | Self::NclExt
                | Self::NclSys
                | Self::NclTest
        )
    }

    pub(crate) const fn is_sb(self) -> bool {
        matches!(
            self,
            Self::SbExt
                | Self::SbThread
                | Self::SbAlien
                | Self::SbSys
                | Self::SbMop
                | Self::SbGray
                | Self::SbUnicode
                | Self::SbDebug
                | Self::SbAlienInternals
                | Self::SbC
                | Self::SbImpl
                | Self::SbInt
                | Self::SbKernel
                | Self::SbPcl
                | Self::SbUnix
                | Self::SbVm
        )
    }
}
