#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    Continuation,
    Text,
    Binary,
    Close,
    Ping,
    Pong,
}

impl OpCode {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x0 => Some(OpCode::Continuation),
            0x1 => Some(OpCode::Text),
            0x2 => Some(OpCode::Binary),
            0x8 => Some(OpCode::Close),
            0x9 => Some(OpCode::Ping),
            0xA => Some(OpCode::Pong),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub fin: bool,
    pub opcode: OpCode,
    pub masked: bool,
    pub mask_key: [u8; 4],
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub enum FrameError {
    InsufficientData,
    InvalidOpCode(u8),
    Io(std::io::Error),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::InsufficientData => write!(f, "insufficient data"),
            FrameError::InvalidOpCode(c) => write!(f, "invalid opcode: {c:#x}"),
            FrameError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for FrameError {}

impl From<std::io::Error> for FrameError {
    fn from(e: std::io::Error) -> Self {
        FrameError::Io(e)
    }
}

impl Frame {
    pub fn parse(data: &[u8]) -> Result<Self, FrameError> {
        if data.len() < 2 {
            return Err(FrameError::InsufficientData);
        }
        let first = data[0];
        let second = data[1];
        let fin = first & 0x80 != 0;
        let opcode_val = first & 0x0F;
        let opcode = OpCode::from_u8(opcode_val).ok_or(FrameError::InvalidOpCode(opcode_val))?;
        let masked = second & 0x80 != 0;
        let mut payload_len = (second & 0x7F) as usize;
        let mut offset = 2;

        if payload_len == 126 {
            if data.len() < 4 {
                return Err(FrameError::InsufficientData);
            }
            payload_len = u16::from_be_bytes([data[2], data[3]]) as usize;
            offset = 4;
        } else if payload_len == 127 {
            if data.len() < 10 {
                return Err(FrameError::InsufficientData);
            }
            payload_len = u64::from_be_bytes([
                data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9],
            ]) as usize;
            offset = 10;
        }

        let mask_key = if masked {
            if data.len() < offset + 4 {
                return Err(FrameError::InsufficientData);
            }
            let mut key = [0u8; 4];
            key.copy_from_slice(&data[offset..offset + 4]);
            offset += 4;
            key
        } else {
            [0; 4]
        };

        if data.len() < offset + payload_len {
            return Err(FrameError::InsufficientData);
        }

        let mut payload = data[offset..offset + payload_len].to_vec();
        if masked {
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask_key[i % 4];
            }
        }

        Ok(Frame {
            fin,
            opcode,
            masked,
            mask_key,
            payload,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut first = 0u8;
        if self.fin {
            first |= 0x80;
        }
        first |= match self.opcode {
            OpCode::Continuation => 0x0,
            OpCode::Text => 0x1,
            OpCode::Binary => 0x2,
            OpCode::Close => 0x8,
            OpCode::Ping => 0x9,
            OpCode::Pong => 0xA,
        };
        out.push(first);

        let len = self.payload.len();
        if len < 126 {
            out.push((len as u8) | if self.masked { 0x80 } else { 0 });
        } else if len <= u16::MAX as usize {
            out.push(126 | if self.masked { 0x80 } else { 0 });
            out.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            out.push(127 | if self.masked { 0x80 } else { 0 });
            out.extend_from_slice(&(len as u64).to_be_bytes());
        }

        if self.masked {
            out.extend_from_slice(&self.mask_key);
            for (i, byte) in self.payload.iter().enumerate() {
                out.push(byte ^ self.mask_key[i % 4]);
            }
        } else {
            out.extend_from_slice(&self.payload);
        }

        out
    }
}
