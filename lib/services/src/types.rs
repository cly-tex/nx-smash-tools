#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Header0Tag(u32);

#[rustfmt::skip]
impl Header0Tag {
    const TAG:           u32 = 0x0000FFFF;
    const PNTR_COUNT:    u32 = 0x000F0000;
    const SEND_COUNT:    u32 = 0x00F00000;
    const RECV_COUNT:    u32 = 0x0F000000;
    const EXCH_COUNT:    u32 = 0xF0000000;

    const TAG_SHIFT:  u32 = Self::TAG.trailing_zeros();
    const PNTR_SHIFT: u32 = Self::PNTR_COUNT.trailing_zeros();
    const SEND_SHIFT: u32 = Self::SEND_COUNT.trailing_zeros();
    const RECV_SHIFT: u32 = Self::RECV_COUNT.trailing_zeros();
    const EXCH_SHIFT: u32 = Self::EXCH_COUNT.trailing_zeros();

    pub const fn new(tag: u32, pointer_count: u32, send_count: u32, receive_count: u32, exchange_count: u32) -> Self {
        let mut inner = 0u32;
        inner |= (tag            << Self::TAG_SHIFT ) & Self::TAG;
        inner |= (pointer_count  << Self::PNTR_SHIFT) & Self::PNTR_COUNT;
        inner |= (send_count     << Self::SEND_SHIFT) & Self::SEND_COUNT;
        inner |= (receive_count  << Self::RECV_SHIFT) & Self::RECV_COUNT;
        inner |= (exchange_count << Self::EXCH_SHIFT) & Self::EXCH_COUNT;

        Self(inner)
    }
}

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Header1Tag(u32);

#[rustfmt::skip]
impl Header1Tag {
    const RAW_COUNT:   u32 = 0x000003FF;
    const RECV_COUNT:  u32 = 0x00003C00;
    const RECV_OFFSET: u32 = 0x7FF00000;
    const SPECIAL_COUNT:    u32 = 0x80000000;

    const RAW_SHIFT:         u32 = Self::RAW_COUNT.trailing_zeros();
    const RECV_COUNT_SHIFT:  u32 = Self::RECV_COUNT.trailing_zeros();
    const RECV_OFFSET_SHIFT: u32 = Self::RECV_OFFSET.trailing_zeros();
    const SPECIAL_SHIFT:     u32 = Self::SPECIAL_COUNT.trailing_zeros();

    pub const fn new(raw_count: u32, receive_list_count: u32, receive_list_offset: u32, has_special: bool) -> Self {
        let mut inner = 0u32;
        inner |= (raw_count            << Self::RAW_SHIFT        ) & Self::RAW_COUNT;
        inner |= (receive_list_count   << Self::RECV_COUNT_SHIFT ) & Self::RECV_COUNT;
        inner |= (receive_list_offset  << Self::RECV_OFFSET_SHIFT) & Self::RECV_OFFSET;
        inner |= ((has_special as u32) << Self::SPECIAL_SHIFT    ) & Self::SPECIAL_COUNT;
        Self(inner)
    }
}

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct SpecialTag(u32);

#[rustfmt::skip]
impl SpecialTag {
    const PID:        u32 = 0x00000001;
    const COPY_COUNT: u32 = 0x0000001E;
    const MOVE_COUNT: u32 = 0x000001E0;

    const PID_SHIFT: u32 = Self::PID.trailing_zeros();
    const COPY_SHIFT: u32 = Self::COPY_COUNT.trailing_zeros();
    const MOVE_SHIFT: u32 = Self::MOVE_COUNT.trailing_zeros();

    pub const fn new(send_pid: bool, copy_handle_count: u32, move_handle_count: u32) -> Self {
        let mut inner = 0u32;
        inner |= ((send_pid as u32) << Self::PID_SHIFT) & Self::PID;
        inner |= (copy_handle_count << Self::COPY_SHIFT) & Self::COPY_COUNT;
        inner |= (move_handle_count << Self::MOVE_SHIFT) & Self::MOVE_COUNT;

        Self(inner)
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CmifInHeader {
    signature: u32,
    version: u16,
    reserved: u16,
    method_id: u32,
    token: u32,
}

impl CmifInHeader {
    const SIGNATURE: u32 = u32::from_le_bytes(*b"SFCI");
    const VERSION: u16 = 1;

    pub const fn new(method_id: u32) -> Self {
        Self {
            signature: Self::SIGNATURE,
            version: Self::VERSION,
            reserved: 0,
            method_id,
            token: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CmifOutHeader {
    signature: u32,
    version: u16,
    reserved: u16,
    result: u32,
    interface_id: u32,
}

impl CmifOutHeader {
    const SIGNATURE: u32 = u32::from_le_bytes(*b"SFCO");
    const VERSION: u16 = 0;

    pub const fn new(result: u32, interface_id: u32) -> Self {
        Self {
            signature: Self::SIGNATURE,
            version: Self::VERSION,
            reserved: 0,
            result,
            interface_id,
        }
    }

    pub const fn result(&self) -> u32 {
        self.result
    }
}
