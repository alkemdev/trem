use trem::event::TimedEvent;

#[derive(Debug)]
pub enum Command {
    Play,
    Stop,
    SetBpm(f64),
    LoadEvents(Vec<TimedEvent>),
    NoteOn {
        frequency: f64,
        velocity: f64,
        voice: u32,
    },
    NoteOff {
        voice: u32,
    },
    SetParam {
        node: u32,
        param_id: u32,
        value: f64,
    },
}

#[derive(Debug)]
pub enum Notification {
    Position { beat: f64 },
    ScopeData(Vec<f32>),
    Meter { peak_l: f32, peak_r: f32 },
    Stopped,
}

pub struct Bridge {
    pub cmd_tx: rtrb::Producer<Command>,
    pub notif_rx: rtrb::Consumer<Notification>,
}

impl Bridge {
    pub fn send(&mut self, cmd: Command) {
        let _ = self.cmd_tx.push(cmd);
    }

    pub fn try_recv(&mut self) -> Option<Notification> {
        self.notif_rx.pop().ok()
    }
}

pub struct AudioBridge {
    pub cmd_rx: rtrb::Consumer<Command>,
    pub notif_tx: rtrb::Producer<Notification>,
}

pub fn create_bridge(capacity: usize) -> (Bridge, AudioBridge) {
    let (cmd_tx, cmd_rx) = rtrb::RingBuffer::new(capacity);
    let (notif_tx, notif_rx) = rtrb::RingBuffer::new(capacity);
    (
        Bridge { cmd_tx, notif_rx },
        AudioBridge { cmd_rx, notif_tx },
    )
}
