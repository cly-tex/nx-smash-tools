use core::num::NonZeroU32;

use crate::svc::BreakReason;

pub type NxResult<T> = Result<T, NxError>;

pub enum NxResultCode {
    Success,
    Error(NxError),
}

impl NxResultCode {
    pub fn assert(self) {
        match self {
            Self::Success => {}
            Self::Error(_) => crate::svc::break_now(BreakReason::ASSERT),
        }
    }

    pub fn then<T>(self, f: impl FnOnce() -> T) -> NxResult<T> {
        match self {
            Self::Success => Ok(f()),
            Self::Error(e) => Err(e),
        }
    }

    pub fn then_ok<T>(self, item: T) -> NxResult<T> {
        match self {
            Self::Success => Ok(item),
            Self::Error(e) => Err(e),
        }
    }
}

const _: () = {
    assert!(size_of::<NxResultCode>() == size_of::<NxError>());
};

impl NxResultCode {}

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct NxError(NonZeroU32);

impl core::fmt::Debug for NxError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let module = MODULES
            .get(self.module().get() as usize)
            .copied()
            .unwrap_or(ModuleDescriptor::new());
        let desc = module
            .descriptions
            .get(self.description() as usize)
            .unwrap_or(&"<unknown>");

        write!(
            f,
            "{:#x} {}:{} ({}-{})",
            self.0.get(),
            module.name,
            desc,
            self.module().get(),
            self.description()
        )
    }
}

#[rustfmt::skip]
impl NxError {
    const MODULE_MASK:       u32 = 0x000001FF;
    const DESCRIPTION_MASK:  u32 = 0x003FFE00;
    const MODULE_SHIFT:      u32 = Self::MODULE_MASK.trailing_zeros();
    const DESCRIPTION_SHIFT: u32 = Self::DESCRIPTION_MASK.trailing_zeros();

    const RESERVED_MASK:     u32 = 0xFFC00000;
}

impl NxError {
    /// Constructs a new error from raw parts
    ///
    /// # Notes
    /// - This method can still fail if `module` is not non-zero in the lower 9-bits
    pub const fn from_parts(module: NonZeroU32, desc: u32) -> Option<Self> {
        let mut inner = 0u32;
        inner |= (module.get() << Self::MODULE_SHIFT) & Self::MODULE_MASK;
        inner |= (desc << Self::DESCRIPTION_SHIFT) & Self::DESCRIPTION_MASK;
        Self::new_raw(inner)
    }

    /// Constructs a new error from the raw error value
    ///
    /// # Notes
    /// - The module component of the value is checked to ensure that it is non-zero
    /// - The reserved bits are masked off
    pub const fn new_raw(error: u32) -> Option<Self> {
        if (error & Self::MODULE_MASK) != 0 {
            // SAFETY: We know that the value is non-zero because the module is non-zero
            Some(unsafe { Self::new_raw_unchecked(error & !Self::RESERVED_MASK) })
        } else {
            None
        }
    }

    /// # Safety
    /// Caller ensures that `error` is a non-zero value
    pub const unsafe fn new_raw_unchecked(error: u32) -> Self {
        // SAFETY: Caller upholds invariants
        Self(unsafe { NonZeroU32::new_unchecked(error & !Self::RESERVED_MASK) })
    }

    /// Returns the module portion of this error
    pub const fn module(&self) -> NonZeroU32 {
        // SAFETY: It is an invariant that the inner error value have a non-zero value for the Module
        unsafe {
            NonZeroU32::new_unchecked((self.0.get() & Self::MODULE_MASK) >> Self::MODULE_SHIFT)
        }
    }

    /// Returns the description portion of this error
    pub const fn description(&self) -> u32 {
        (self.0.get() & Self::DESCRIPTION_MASK) >> Self::DESCRIPTION_SHIFT
    }
}

#[derive(Copy, Clone)]
struct ModuleDescriptor {
    name: &'static str,
    descriptions: &'static [&'static str],
}

impl ModuleDescriptor {
    const fn new() -> Self {
        Self {
            name: "<unknown>",
            descriptions: &[],
        }
    }
}

macro_rules! define_error_codes {
    ($($module_name:ident($value:expr) {
        $(
            $error_name:ident = $desc_value:expr;
        )*
    };)*) => {
        #[doc(hidden)]
        const NUM_MODULES: usize = const {
            let mut max = 0usize;
            $(
                if max < $value {
                    max = $value;
                }
            )*
            max + 1
        };

        const MODULES: [ModuleDescriptor; NUM_MODULES] = const {
            let mut modules = [const { ModuleDescriptor::new() }; NUM_MODULES];
            $(
                modules[$value] = $module_name::DESCRIPTOR;
            )*
            modules
        };

        $(
            pub mod $module_name {
                const NUM_DESCRIPTIONS: usize = const {
                    let mut max = 0usize;
                    $(
                        if max < $desc_value {
                            max = $desc_value;
                        }
                    )*

                    max + 1
                };
                const DESCRIPTION_NAMES: [&'static str; NUM_DESCRIPTIONS] = const {
                    let mut descs = ["<unknown>"; NUM_DESCRIPTIONS];
                    $(
                        descs[$desc_value] = stringify!($error_name);
                    )*
                    descs
                };

                pub(super) const DESCRIPTOR: super::ModuleDescriptor = super::ModuleDescriptor {
                    name: stringify!($module_name),
                    descriptions: &DESCRIPTION_NAMES
                };

                pub const MODULE_CODE: core::num::NonZeroU32 = const { core::num::NonZeroU32::new($value).unwrap() };

                $(
                    pub const $error_name: super::NxError = const { super::NxError::from_parts(MODULE_CODE, $desc_value).unwrap() };
                )*
            }
        )*
    }
}

define_error_codes! {
    svc(1) {
        OUT_OF_SESSIONS               = 7;

        INVALID_ARGUMENT              = 14;

        NOT_IMPLEMENTED               = 33;

        STOP_PROCESSING_EXCEPTION     = 54;

        NO_SYNCHRONIZATION_OBJECT     = 57;

        TERMINATION_REQUESTED         = 59;

        NO_EVENT                      = 70;

        INVALID_SIZE                  = 101;
        INVALID_ADDRESS               = 102;
        OUT_OF_RESOURCE               = 103;
        OUT_OF_MEMORY                 = 104;
        OUT_OF_HANDLES                = 105;
        INVALID_CURRENT_MEMORY        = 106;
        INVALID_NEW_MEMORY_PREMISSION = 108;
        INVALID_MEMORY_REGION         = 110;
        INVALID_PRIORITY              = 112;
        INVALID_CORE_ID               = 113;
        INVALID_HANDLE                = 114;
        INVALID_POINTER               = 115;
        INVALID_COMBINATION           = 116;
        TIMED_OUT                     = 117;
        CANCELLED                     = 118;
        OUT_OF_RANGE                  = 119;
        INVALID_ENUM_VALUE            = 120;
        NOT_FOUND                     = 121;
        BUSY                          = 122;
        SESSION_CLOSED                = 123;
        NOT_HANDLED                   = 124;
        INVALID_STATE                 = 125;
        RESERVED_USED                 = 126;
        NOT_SUPPORTED                 = 127;
        DEBUG                         = 128;
        NO_THREAD                     = 129;
        UNKNOWN_THREAD                = 130;
        PORT_CLOSED                   = 131;
        LIMIT_REACHED                 = 132;
        INVALID_MEMORY_POOL           = 133;

        RECEIVE_LIST_BROKEN           = 258;
        OUT_OF_ADDRESS_SPACE          = 259;
        MESSAGE_TOO_LARGE             = 260;

        INVALID_PROCESS_ID            = 517;
        INVALID_THREAD_ID             = 518;
        INVALID_ID                    = 519;
        PROCESS_TERMINATED            = 520;
    };
}
