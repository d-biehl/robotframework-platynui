//! Minimal EI (libei) client for the compositor's EIS tests, built on `reis`.
//!
//! Connects to the compositor's EIS socket, completes the handshake, binds
//! every capability a seat offers, and records the devices that resume. Input
//! is sent as one emulation sequence per event (start, event, frame, stop), the
//! same shape the platform's EIS backend and `platynui-eis-test-client` use.
//! Every wait has a deadline.
#![allow(dead_code)]

use std::fs::File;
use std::os::unix::fs::FileExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use reis::PendingRequestResult;
use reis::ei;
use reis::enumflags2::BitFlags;
use reis::event::{Device, DeviceCapability, EiEvent, EiEventConverter};
use rustix::event::{PollFd, PollFlags, Timespec, poll};

pub type BoxError = Box<dyn std::error::Error>;

/// An EI connection to the compositor.
pub struct EiClient {
    context: ei::Context,
    converter: EiEventConverter,
    /// Devices that have resumed (ready for input), in arrival order.
    pub devices: Vec<Device>,
    /// The compositor disconnected us (or closed the socket).
    pub disconnected: bool,
    sequence: u32,
}

fn timestamp_us() -> u64 {
    use std::sync::LazyLock;
    static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);
    u64::try_from(EPOCH.elapsed().as_micros()).unwrap_or(u64::MAX)
}

impl EiClient {
    /// Connect and complete the handshake as `context_type`.
    pub fn connect(socket: &Path, name: &str, context_type: ei::handshake::ContextType) -> Result<Self, BoxError> {
        let stream = UnixStream::connect(socket)?;
        let context = ei::Context::new(stream)?;
        let response = reis::handshake::ei_handshake_blocking(&context, name, context_type)?;
        let converter = EiEventConverter::new(&context, response);
        let mut client = Self { context, converter, devices: Vec::new(), disconnected: false, sequence: 0 };
        client.dispatch_buffered()?;
        Ok(client)
    }

    /// Connect as a sender and wait until a device with every capability in
    /// `wanted` has resumed.
    pub fn sender(socket: &Path, wanted: BitFlags<DeviceCapability>) -> Result<Self, BoxError> {
        let mut client = Self::connect(socket, "platynui-compositor-tests", ei::handshake::ContextType::Sender)?;
        client.wait_until("resumed devices", |c| wanted.iter().all(|cap| c.device_with(cap).is_some()))?;
        Ok(client)
    }

    fn dispatch_buffered(&mut self) -> Result<(), BoxError> {
        while let Some(result) = self.context.pending_event() {
            match result {
                PendingRequestResult::Request(event) => self.converter.handle_event(event)?,
                PendingRequestResult::ParseError(err) => return Err(format!("EI parse error: {err}").into()),
                PendingRequestResult::InvalidObject(_) => {}
            }
        }
        while let Some(event) = self.converter.next_event() {
            match event {
                EiEvent::SeatAdded(seat) => {
                    seat.seat.bind_capabilities(BitFlags::all());
                    self.converter.connection().flush()?;
                }
                EiEvent::DeviceResumed(resumed) => self.devices.push(resumed.device.clone()),
                EiEvent::DeviceRemoved(removed) => self.devices.retain(|d| d.device() != removed.device.device()),
                EiEvent::Disconnected(_) => self.disconnected = true,
                _ => {}
            }
        }
        Ok(())
    }

    /// Read and dispatch whatever arrives before `deadline`.
    fn pump(&mut self, deadline: Instant) -> Result<(), BoxError> {
        self.dispatch_buffered()?;
        let remaining = deadline.saturating_duration_since(Instant::now()).min(Duration::from_millis(100));
        let timeout =
            Timespec { tv_sec: remaining.as_secs().try_into().unwrap_or(0), tv_nsec: remaining.subsec_nanos().into() };
        let readable = {
            let mut fds = [PollFd::new(&self.context, PollFlags::IN)];
            poll(&mut fds, Some(&timeout))? > 0
        };
        if readable {
            match self.context.read() {
                Ok(0) => self.disconnected = true,
                Ok(_) => {}
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => self.disconnected = true,
            }
            self.dispatch_buffered()?;
        }
        Ok(())
    }

    /// Dispatch until `cond` holds, failing after 10 s.
    pub fn wait_until(&mut self, what: &str, cond: impl Fn(&Self) -> bool) -> Result<(), BoxError> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !cond(self) {
            if Instant::now() >= deadline {
                return Err(format!("timed out waiting for {what} (devices: {:?})", self.capability_names()).into());
            }
            self.pump(deadline)?;
        }
        Ok(())
    }

    /// Dispatch for `duration`, collecting late devices or a disconnect.
    pub fn settle(&mut self, duration: Duration) -> Result<(), BoxError> {
        let deadline = Instant::now() + duration;
        while Instant::now() < deadline && !self.disconnected {
            self.pump(deadline)?;
        }
        Ok(())
    }

    /// The first resumed device with `capability`.
    pub fn device_with(&self, capability: DeviceCapability) -> Option<Device> {
        self.devices.iter().find(|d| d.has_capability(capability)).cloned()
    }

    /// The union of the resumed devices' capabilities.
    pub fn capabilities(&self) -> BitFlags<DeviceCapability> {
        let mut all = BitFlags::empty();
        for device in &self.devices {
            for cap in BitFlags::<DeviceCapability>::all() {
                if device.has_capability(cap) {
                    all |= cap;
                }
            }
        }
        all
    }

    fn capability_names(&self) -> Vec<String> {
        self.capabilities().iter().map(|c| format!("{c:?}")).collect()
    }

    /// The keymap text of the first keyboard device.
    pub fn keymap(&self) -> Option<String> {
        let keymap = self.device_with(DeviceCapability::Keyboard)?.keymap().map(|k| (k.fd.try_clone(), k.size))?;
        let (Ok(fd), size) = keymap else { return None };
        let mut buf = vec![0u8; size as usize];
        File::from(fd).read_exact_at(&mut buf, 0).ok()?;
        Some(String::from_utf8_lossy(&buf).trim_end_matches('\0').to_string())
    }

    /// Send one emulated event on the device with `capability`.
    fn emulate(&mut self, capability: DeviceCapability, send: impl FnOnce(&Device)) -> Result<(), BoxError> {
        let device = self.device_with(capability).ok_or_else(|| format!("no {capability:?} device"))?;
        let serial = self.converter.connection().serial();
        self.sequence += 1;
        device.device().start_emulating(serial, self.sequence);
        send(&device);
        device.device().frame(serial, timestamp_us());
        device.device().stop_emulating(serial);
        self.converter.connection().flush()?;
        Ok(())
    }

    /// Move the absolute pointer to a global logical position.
    #[allow(clippy::cast_possible_truncation)] // EI coordinates are f32 on the wire
    pub fn move_to(&mut self, x: f64, y: f64) -> Result<(), BoxError> {
        let (x, y) = (x as f32, y as f32);
        self.emulate(DeviceCapability::PointerAbsolute, |d| {
            d.interface::<ei::PointerAbsolute>().expect("absolute pointer").motion_absolute(x, y);
        })
    }

    /// Press or release a button (evdev code).
    pub fn button(&mut self, button: u32, pressed: bool) -> Result<(), BoxError> {
        let state = if pressed { ei::button::ButtonState::Press } else { ei::button::ButtonState::Released };
        self.emulate(DeviceCapability::Button, |d| {
            d.interface::<ei::Button>().expect("button").button(button, state);
        })
    }

    /// Press or release a key (evdev code).
    pub fn key(&mut self, key: u32, pressed: bool) -> Result<(), BoxError> {
        let state = if pressed { ei::keyboard::KeyState::Press } else { ei::keyboard::KeyState::Released };
        self.emulate(DeviceCapability::Keyboard, |d| {
            d.interface::<ei::Keyboard>().expect("keyboard").key(key, state);
        })
    }
}
