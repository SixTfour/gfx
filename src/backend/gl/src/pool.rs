use core::{self, pool};
use command::{self, Command, RawCommandBuffer, SubpassCommandBuffer};
use native as n;
use {Backend, CommandQueue, Share};
use gl;
use std::collections::HashMap;
use std::rc::Rc;

fn create_fbo_internal(gl: &gl::Gl) -> gl::types::GLuint {
    let mut name = 0 as n::FrameBuffer;
    unsafe {
        gl.GenFramebuffers(1, &mut name);
    }
    info!("\tCreated frame buffer {}", name);
    name
}

// Storage of command buffer memory.
// Depends on the reset model chosen when creating the command pool.
pub enum BufferMemory {
    // Storing all recorded commands and data in the pool in a linear
    // piece of memory shared by all associated command buffers.
    //
    // # Safety!
    //
    // This implementation heavily relays on the fact that the user **must**
    // ensure that only **one** associated command buffer from each pool
    // is recorded at the same time. Additionally, we only allow to reset the
    // whole command pool. This allows us to avoid fragmentation of the memory
    // and saves us additional bookkeeping overhead for keeping track of all
    // allocated buffers.
    //
    // Reseting the pool will free all data and commands recorded. Therefore it's
    // crucial that all submits have been finished **before** calling `reset`.
    Linear {
        commands: Vec<Command>,
        data: Vec<u8>,
    },
    // Storing the memory for each command buffer separately to allow individual
    // command buffer resets.
    Individual {
        storage: HashMap<u64, (Vec<Command>, Vec<u8>)>,
        next_buffer_id: u64,
    },
}

pub struct RawCommandPool {
    fbo: n::FrameBuffer,
    limits: command::Limits,
    memory: BufferMemory,
}

impl core::RawCommandPool<Backend> for RawCommandPool {
    fn reset(&mut self) {
        match self.memory {
            BufferMemory::Linear { ref mut commands, ref mut data } => {
                commands.clear();
                data.clear();
            }
            BufferMemory::Individual { ref mut storage, .. } => {
                for (_, &mut (ref mut commands, ref mut data)) in storage {
                    commands.clear();
                    data.clear();
                }
            }
        }
    }

    unsafe fn from_queue(mut queue: &CommandQueue, flags: pool::CommandPoolCreateFlags) -> Self {
        let fbo = create_fbo_internal(&queue.share.context);
        let limits = queue.share.limits.into();
        let memory = if flags.contains(pool::RESET_INDIVIDUAL) {
            BufferMemory::Individual {
                storage: HashMap::new(),
                next_buffer_id: 0,
            }
        } else {
            BufferMemory::Linear {
                commands: Vec::new(),
                data: Vec::new(),
            }
        };

        // Ignoring `TRANSIENT` hint, unsure how to make use of this.

        RawCommandPool {
            fbo,
            limits,
            memory,
        }
    }

    fn allocate(&mut self, num: usize) -> Vec<RawCommandBuffer> {
        (0..num).map(|_|
                    RawCommandBuffer::new(
                        self.fbo,
                        self.limits,
                        &mut self.memory))
                .collect()
    }

    unsafe fn free(&mut self, buffers: Vec<RawCommandBuffer>) {
        if let BufferMemory::Individual { ref mut storage, .. } = self.memory {
            // Expecting that the buffers actually are allocated from this pool.
            for buffer in buffers {
                storage.remove(&buffer.id);
            }
        }
        // Linear: Freeing doesn't really matter here as everything is backed by
        //         only one Vec.
    }
}

pub struct SubpassCommandPool {
    command_buffers: Vec<SubpassCommandBuffer>,
}

impl pool::SubpassCommandPool<Backend> for SubpassCommandPool { }