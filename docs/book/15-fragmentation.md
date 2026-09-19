# Chapter 15: Message Fragmentation

Per RFC 6455 §5.4, a single WebSocket message can be split across
multiple frames. This chapter explains how Runact's reader loop handles
fragmentation.

---

## 15.1 Fragmentation Overview

```
Message: "Hello, world!"

Can be split into:

Frame 1: FIN=false, Opcode=Text,     Payload="Hello, "
Frame 2: FIN=false, Opcode=Continua, Payload="world"
Frame 3: FIN=true,  Opcode=Continua, Payload="!"
```

### Rules (RFC 6455 §5.4)

1. The **first** frame of a message has `FIN=false` and a data opcode
   (`Text` or `Binary`).
2. **Subsequent** frames have `FIN=false` and opcode `Continuation`.
3. The **final** frame has `FIN=true` and opcode `Continuation`.
4. Control frames (`Ping`, `Pong`, `Close`) can be interleaved between
   any data frames.
5. A new `Text` or `Binary` frame while already in a fragmented message
   is a **protocol error**.

## 15.2 Runact's Reassembly

The reader loop in `AsyncWebSocket` maintains reassembly state:

```rust
// Simplified reader loop
let mut fragmented_opcode: Option<OpCode> = None;
let mut fragmented_payload: Vec<u8> = Vec::new();

while let Ok(frame) = Frame::parse(&buffer) {
    match frame.opcode {
        OpCode::Ping => { /* deliver immediately, don't affect reassembly */ }
        OpCode::Pong => { /* deliver immediately */ }
        OpCode::Close => { /* deliver immediately, break */ }
        OpCode::Text | OpCode::Binary => {
            if fragmented_opcode.is_some() {
                // Protocol error: new message started before old one finished
                return Err(ProtocolError);
            }
            if frame.fin {
                // Single-frame message — deliver immediately
                callback(Message::Frame(frame));
            } else {
                // Start of fragmented message
                fragmented_opcode = Some(frame.opcode);
                fragmented_payload = frame.payload;
            }
        }
        OpCode::Continuation => {
            if let Some(opcode) = fragmented_opcode {
                fragmented_payload.extend_from_slice(&frame.payload);
                if frame.fin {
                    // End of fragmented message — deliver reassembled
                    callback(Message::Frame(Frame {
                        fin: true,
                        opcode,  // Original opcode (Text or Binary)
                        payload: std::mem::take(&mut fragmented_payload),
                        ..
                    }));
                    fragmented_opcode = None;
                }
                // else: more continuation frames expected
            } else {
                // Protocol error: continuation without a started message
                return Err(ProtocolError);
            }
        }
    }
}
```

### Key Design Points

1. **Control frames are delivered immediately.** They are never buffered
   as part of a fragmented message. This means a Ping arriving in the
   middle of a fragmented Text message is delivered as its own `Message`
   before the reassembled message.

2. **Reassembled frames preserve original metadata.** The delivered frame
   has `fin=true`, the original data opcode, and `masked=false` (server-side).

3. **Protocol errors break the connection.** If the rules above are
   violated, the reader loop delivers a `Message::Error` and stops.

## 15.3 Multi-Frame read() Processing

The reader also handles the case where multiple WebSocket frames arrive
in a single TCP `read()` call:

```
TCP segment: [Frame1][Frame2][Frame3]
read() → [all bytes]
```

The reader loop processes frames sequentially, advancing an offset pointer
through the buffer:

```rust
let mut offset = 0;
while offset < data.len() {
    let frame = Frame::parse(&data[offset..])?;
    offset += frame_encoded_size(&frame);
    // ... process frame ...
}
```

This ensures no frames are lost when the OS coalesces multiple WebSocket
frames into one TCP segment.

### frame_encoded_size

```rust
fn frame_encoded_size(frame: &Frame) -> usize {
    let mut size = 2; // header bytes
    let len = frame.payload.len();
    if len >= 126 {
        size += if len <= u16::MAX as usize { 2 } else { 8 };
    }
    if frame.masked { size += 4; } // mask key
    size += len;
    size
}
```

## 15.4 Testing Fragmentation

The `ws_fragmentation.rs` integration test covers:

1. **Basic reassembly**: Client sends a 3-frame fragmented message.
   Server receives one reassembled frame with the full payload.

2. **Interleaved control frames**: Client sends a Ping frame in the middle
   of a fragmented Text message. Server receives the Ping (and auto-Pong
   response) immediately, then the reassembled Text message.

Both tests verify compliance with RFC 6455 §5.4.
