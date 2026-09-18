use std::io::{Read, Write};

use crate::websocket::frame::{Frame, FrameError, OpCode};

pub struct WebSocketConnection<R, W = Box<dyn Write>> {
    reader: R,
    writer: W,
}

impl<R: Read> WebSocketConnection<R, Vec<u8>> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            writer: Vec::new(),
        }
    }
}

impl<R: Read, W: Write> WebSocketConnection<R, W> {
    pub fn with_writer(reader: R, writer: W) -> Self {
        Self { reader, writer }
    }

    pub fn read_frame(&mut self) -> Result<Frame, FrameError> {
        let mut header_buf = [0u8; 2];
        self.reader
            .read_exact(&mut header_buf)
            .map_err(FrameError::Io)?;

        let second = header_buf[1];
        let masked = second & 0x80 != 0;
        let mut payload_len = (second & 0x7F) as usize;
        let mut mask_key = [0u8; 4];

        if payload_len == 126 {
            let mut ext = [0u8; 2];
            self.reader.read_exact(&mut ext).map_err(FrameError::Io)?;
            payload_len = u16::from_be_bytes(ext) as usize;
        } else if payload_len == 127 {
            let mut ext = [0u8; 8];
            self.reader.read_exact(&mut ext).map_err(FrameError::Io)?;
            payload_len = u64::from_be_bytes(ext) as usize;
        }

        if masked {
            self.reader
                .read_exact(&mut mask_key)
                .map_err(FrameError::Io)?;
        }

        let mut payload = vec![0u8; payload_len];
        self.reader
            .read_exact(&mut payload)
            .map_err(FrameError::Io)?;

        if masked {
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask_key[i % 4];
            }
        }

        let opcode_val = header_buf[0] & 0x0F;
        let opcode = OpCode::from_u8(opcode_val).ok_or(FrameError::InvalidOpCode(opcode_val))?;
        let fin = header_buf[0] & 0x80 != 0;

        Ok(Frame {
            fin,
            opcode,
            masked,
            mask_key,
            payload,
        })
    }

    pub fn send_frame(&mut self, frame: &Frame) -> Result<(), FrameError> {
        let encoded = frame.encode();
        self.writer.write_all(&encoded).map_err(FrameError::Io)?;
        self.writer.flush().map_err(FrameError::Io)?;
        Ok(())
    }

    pub fn into_writer(self) -> W {
        self.writer
    }

    pub fn send_pong(&mut self, payload: &[u8]) -> Result<(), FrameError> {
        self.send_frame(&Frame {
            fin: true,
            opcode: OpCode::Pong,
            masked: false,
            mask_key: [0; 4],
            payload: payload.to_vec(),
        })
    }

    pub fn send_close(&mut self, status: u16) -> Result<(), FrameError> {
        self.send_frame(&Frame {
            fin: true,
            opcode: OpCode::Close,
            masked: false,
            mask_key: [0; 4],
            payload: status.to_be_bytes().to_vec(),
        })
    }
}
