/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

use conquer_once::spin::OnceCell;
use core::pin::Pin;
use core::task::{Context, Poll};
use crossbeam_queue::ArrayQueue;
use futures_util::stream::Stream;
use futures_util::task::AtomicWaker;
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};
use spinning_top::Spinlock;

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

static KEYBOARD_DECODER: OnceCell<Spinlock<Keyboard<layouts::Us104Key, ScancodeSet1>>> =
    OnceCell::uninit();
static KEY_BUFFER: Spinlock<KeyBuffer> = Spinlock::new(KeyBuffer::new());

pub struct KeyBuffer {
    buffer: [u8; 256],
    head: usize,
    tail: usize,
}

impl KeyBuffer {
    pub const fn new() -> Self {
        KeyBuffer {
            buffer: [0; 256],
            head: 0,
            tail: 0,
        }
    }

    pub fn push(&mut self, byte: u8) {
        let next = (self.tail + 1) % self.buffer.len();
        if next == self.head {
            return;
        }
        self.buffer[self.tail] = byte;
        self.tail = next;
    }

    pub fn pop(&mut self) -> Option<u8> {
        if self.head == self.tail {
            return None;
        }
        let byte = self.buffer[self.head];
        self.head = (self.head + 1) % self.buffer.len();
        Some(byte)
    }

    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }
}

pub fn drain_scancodes() {
    let _ = SCANCODE_QUEUE.try_init_once(|| ArrayQueue::new(100));
    let queue = match SCANCODE_QUEUE.try_get() {
        Ok(q) => q,
        Err(_) => return,
    };
    let _ = KEYBOARD_DECODER.try_init_once(|| {
        Spinlock::new(Keyboard::new(
            ScancodeSet1::new(),
            layouts::Us104Key,
            HandleControl::Ignore,
        ))
    });
    let decoder = match KEYBOARD_DECODER.try_get() {
        Ok(d) => d,
        Err(_) => return,
    };
    let mut d = decoder.lock();
    let mut buf = KEY_BUFFER.lock();

    while let Some(scancode) = queue.pop() {
        if let Ok(Some(key_event)) = d.add_byte(scancode) {
            if let Some(key) = d.process_keyevent(key_event) {
                if let DecodedKey::Unicode(c) = key {
                    if c.is_ascii() {
                        buf.push(c as u8);
                    }
                }
            }
        }
    }
}

pub fn read_keybuf(buf: &mut [u8]) -> usize {
    let mut copied = 0;
    let mut kb = KEY_BUFFER.lock();
    while copied < buf.len() {
        match kb.pop() {
            Some(b) => {
                buf[copied] = b;
                copied += 1;
                if b == b'\n' {
                    break;
                }
            }
            None => break,
        }
    }
    copied
}

pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            crate::println!("WARNING: scancode queue full; dropping keyboard input");
        } else {
            WAKER.wake();
        }
    } else {
        crate::println!("WARNING: scancode queue uninitialized");
    }
}

pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE
            .try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        KEYBOARD_DECODER
            .try_init_once(|| {
                Spinlock::new(Keyboard::new(
                    ScancodeSet1::new(),
                    layouts::Us104Key,
                    HandleControl::Ignore,
                ))
            })
            .expect("KEYBOARD_DECODER already initialized");
        ScancodeStream { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE
            .try_get()
            .expect("scancode queue not initialized");

        if let Some(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        WAKER.register(&cx.waker());
        match queue.pop() {
            Some(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            None => Poll::Pending,
        }
    }
}
